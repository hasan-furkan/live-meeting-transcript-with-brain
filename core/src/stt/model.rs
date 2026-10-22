use super::SttError;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TREE_URL: &str = "https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/main";
const RESOLVE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_REDIRECTS: usize = 10;

pub fn model_file_name(model: &str) -> String {
    format!("ggml-{model}.bin")
}

/// Non-empty, at most 64 chars of `[A-Za-z0-9._-]`, never `..`.
pub fn valid_model_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// `ggml-<valid name>.bin`: the only file name ever joined under the models folder.
pub(crate) fn valid_model_file_name(file: &str) -> bool {
    file.strip_prefix("ggml-")
        .and_then(|rest| rest.strip_suffix(".bin"))
        .is_some_and(valid_model_name)
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
    if !valid_model_name(model) {
        return Err(SttError::InvalidModelName);
    }
    let name = model_file_name(model);
    let dest = dir.join(&name);
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(dir).map_err(|_| SttError::DownloadFailed)?;

    // Redirects stay on HTTPS and are capped, so a downgrade cannot strip TLS.
    let http = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS || attempt.url().scheme() != "https" {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| SttError::DownloadFailed)?;
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
    fn model_name_accepts_only_safe_characters() {
        assert!(valid_model_name("large-v3-turbo-q5_0"));
        assert!(valid_model_name("tiny.en"));
        assert!(valid_model_name("base"));
        assert!(!valid_model_name(""));
        assert!(!valid_model_name(".."));
        assert!(!valid_model_name("../evil"));
        assert!(!valid_model_name("a/b"));
        assert!(!valid_model_name("a\\b"));
        assert!(!valid_model_name("tiny bin"));
        assert!(!valid_model_name(&"a".repeat(65)));
        assert!(valid_model_name(&"a".repeat(64)));
    }

    #[test]
    fn model_file_name_must_be_ggml_bin() {
        assert!(valid_model_file_name("ggml-tiny.bin"));
        assert!(!valid_model_file_name("evil.bin"));
        assert!(!valid_model_file_name("ggml-../evil.bin"));
        assert!(!valid_model_file_name("ggml-tiny.txt"));
    }

    #[tokio::test]
    async fn download_rejects_invalid_name_before_network() {
        let dir = tempfile::tempdir().unwrap();
        let err = download_model("../evil", dir.path(), |_, _| {})
            .await
            .unwrap_err();
        assert_eq!(err, SttError::InvalidModelName);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
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
