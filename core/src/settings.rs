use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ScreenshotMode {
    Off,
    Region,
    Full,
    Window,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A window chosen by app name and title; titles change, so the app name is the fallback.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct WindowTarget {
    pub app: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub api_base_url: String,
    pub model: String,
    pub whisper_model: String,
    pub language: String,
    pub explanation_language: String,
    pub notes_dir: Option<PathBuf>,
    pub name_variants: Vec<String>,
    pub keywords: Vec<String>,
    pub general_questions: bool,
    pub name_bias: bool,
    pub hotkey: String,
    pub screenshot_mode: ScreenshotMode,
    pub monitor: Option<String>,
    pub region: Option<Region>,
    pub window: Option<WindowTarget>,
    pub audio_device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_base_url: "https://opencode.ai/inference/go/anthropic".into(),
            model: "deepseek-v4.1-flash".into(),
            whisper_model: "large-v3-turbo-q5_0".into(),
            language: "en".into(),
            explanation_language: String::new(),
            notes_dir: None,
            name_variants: Vec::new(),
            keywords: [
                "homework",
                "assignment",
                "exam",
                "quiz",
                "deadline",
                "attendance",
            ]
            .map(String::from)
            .to_vec(),
            general_questions: true,
            name_bias: true,
            hotkey: "CommandOrControl+Shift+Space".into(),
            screenshot_mode: ScreenshotMode::Off,
            monitor: None,
            region: None,
            window: None,
            audio_device: None,
        }
    }
}

/// Missing or unreadable file → defaults. Unparsable file → moved to `<path>.bak`, defaults.
pub fn load(path: &Path) -> Settings {
    let Ok(text) = fs::read_to_string(path) else {
        return Settings::default();
    };
    match serde_json::from_str(&text) {
        Ok(settings) => settings,
        Err(_) => {
            let bak = with_suffix(path, ".bak");
            // Windows rename does not replace an existing destination.
            let _ = fs::remove_file(&bak);
            let _ = fs::rename(path, bak);
            Settings::default()
        }
    }
}

pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    let tmp = with_suffix(path, ".tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)
}

/// Appends `suffix` to the file name (`settings.json` → `settings.json.bak`).
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[derive(Debug)]
pub enum KeyError {
    Unavailable,
    Failed,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => write!(f, "keychain unavailable"),
            Self::Failed => write!(f, "keychain error"),
        }
    }
}

impl std::error::Error for KeyError {}

const KEYRING_SERVICE: &str = "live-meeting-assistant";
const KEYRING_ACCOUNT: &str = "llm-api-key";

fn entry() -> Result<keyring::Entry, KeyError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|_| KeyError::Unavailable)
}

pub fn get_api_key() -> Result<Option<String>, KeyError> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(KeyError::Failed),
    }
}

/// An empty key deletes the stored entry.
pub fn set_api_key(key: &str) -> Result<(), KeyError> {
    let entry = entry()?;
    if key.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(KeyError::Failed),
        };
    }
    entry.set_password(key).map_err(|_| KeyError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn defaults_match_spec() {
        let d = Settings::default();
        assert_eq!(d.api_base_url, "https://opencode.ai/inference/go/anthropic");
        assert_eq!(d.model, "deepseek-v4.1-flash");
        assert_eq!(d.whisper_model, "large-v3-turbo-q5_0");
        assert_eq!(d.language, "en");
        assert_eq!(d.explanation_language, "");
        assert_eq!(d.notes_dir, None);
        assert!(d.name_variants.is_empty());
        assert_eq!(
            d.keywords,
            [
                "homework",
                "assignment",
                "exam",
                "quiz",
                "deadline",
                "attendance"
            ]
        );
        assert!(d.general_questions);
        assert!(d.name_bias);
        assert_eq!(d.hotkey, "CommandOrControl+Shift+Space");
        assert_eq!(d.screenshot_mode, ScreenshotMode::Off);
        assert_eq!(d.monitor, None);
        assert_eq!(d.region, None);
        assert_eq!(d.audio_device, None);
    }

    #[test]
    fn missing_file_gives_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path), Settings::default());
    }

    #[test]
    fn corrupt_file_backed_up_and_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();

        assert_eq!(load(&path), Settings::default());
        assert!(!path.exists());
        let bak = dir.path().join("settings.json.bak");
        assert_eq!(fs::read_to_string(bak).unwrap(), "{not json");
    }

    #[test]
    fn unknown_and_missing_fields_tolerated() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"model":"x","future":1}"#).unwrap();

        let loaded = load(&path);
        let expected = Settings {
            model: "x".into(),
            ..Settings::default()
        };
        assert_eq!(loaded, expected);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut s = Settings {
            name_variants: vec!["Jordan Reyes".into()],
            screenshot_mode: ScreenshotMode::Region,
            region: Some(Region {
                x: 0,
                y: 10,
                width: 800,
                height: 600,
            }),
            notes_dir: Some(dir.path().join("notes")),
            ..Settings::default()
        };

        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);

        s.language = "tr".into();
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
    }

    #[test]
    fn keychain_identifiers_match_spec() {
        assert_eq!(KEYRING_SERVICE, "live-meeting-assistant");
        assert_eq!(KEYRING_ACCOUNT, "llm-api-key");
    }

    #[test]
    fn api_key_never_serialized() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        let object = json.as_object().unwrap();
        assert!(!object.contains_key("api_key"));
        assert!(!object.contains_key("token"));
    }
}
