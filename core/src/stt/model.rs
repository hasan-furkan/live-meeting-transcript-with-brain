use super::SttError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

const TREE_URL: &str = "https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/main";
const RESOLVE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub fn model_file_name(model: &str) -> String {
    format!("ggml-{model}.bin")
}

/// SHA-256 (hex) and size from the tree listing entry, both only present for LFS files.
fn sha256_and_size(tree: &Value, file_name: &str) -> Option<(String, Option<u64>)> {
    let entry = tree
        .as_array()?
        .iter()
        .find(|e| e.get("path").and_then(Value::as_str) == Some(file_name))?;
    let lfs = entry.get("lfs")?;
    let oid = lfs.get("oid")?.as_str()?.to_string();
    let size = lfs.get("size").and_then(Value::as_u64);
    Some((oid, size))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn download_model(
    model: &str,
    dir: &Path,
    progress: impl Fn(u64, u64),
) -> Result<PathBuf, SttError> {
    let name = model_file_name(model);
    let dest = dir.join(&name);
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(dir).map_err(|_| SttError::DownloadFailed)?;

    let http = reqwest::Client::new();
    let tree: Value = http
        .get(TREE_URL)
        .send()
        .await
        .map_err(|_| SttError::DownloadFailed)?
        .json()
        .await
        .map_err(|_| SttError::DownloadFailed)?;
    let (expected, lfs_size) = sha256_and_size(&tree, &name).ok_or(SttError::DownloadFailed)?;

    let mut response = http
        .get(format!("{RESOLVE_URL}/{name}"))
        .send()
        .await
        .map_err(|_| SttError::DownloadFailed)?;
    if !response.status().is_success() {
        return Err(SttError::DownloadFailed);
    }
    let total = response.content_length().or(lfs_size).unwrap_or(0);

    let part = dir.join(format!("{name}.part"));
    let mut file = std::fs::File::create(&part).map_err(|_| SttError::DownloadFailed)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| SttError::DownloadFailed)?
    {
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|_| SttError::DownloadFailed)?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    drop(file);

    if hex(&hasher.finalize()) != expected {
        let _ = std::fs::remove_file(&part);
        return Err(SttError::ChecksumMismatch);
    }
    std::fs::rename(&part, &dest).map_err(|_| SttError::DownloadFailed)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"type": "file", "oid": "aa", "size": 10, "path": "README.md", "lfs": null},
        {"type": "file", "oid": "bb", "size": 20, "path": "ggml-tiny.bin",
         "lfs": {"oid": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "size": 20}},
        {"type": "file", "oid": "cc", "size": 30, "path": "ggml-base.bin"}
    ]"#;

    #[test]
    fn model_file_name_has_ggml_prefix() {
        assert_eq!(model_file_name("tiny"), "ggml-tiny.bin");
        assert_eq!(model_file_name("large-v3-turbo"), "ggml-large-v3-turbo.bin");
    }

    #[test]
    fn finds_lfs_oid_and_size_of_matching_path() {
        let tree: Value = serde_json::from_str(SAMPLE).unwrap_or_default();
        assert_eq!(
            sha256_and_size(&tree, "ggml-tiny.bin"),
            Some((
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                Some(20)
            ))
        );
    }

    #[test]
    fn rejects_missing_or_non_lfs_entry() {
        let tree: Value = serde_json::from_str(SAMPLE).unwrap_or_default();
        assert_eq!(sha256_and_size(&tree, "ggml-small.bin"), None);
        assert_eq!(sha256_and_size(&tree, "README.md"), None);
        assert_eq!(sha256_and_size(&tree, "ggml-base.bin"), None);
    }
}
