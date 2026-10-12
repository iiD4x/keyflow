use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn def_true() -> bool {
    true
}

fn def_mode() -> TypingMode {
    TypingMode::Fast
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum TypingMode {
    Instant,
    #[default]
    Fast,
    Human,
    Slow,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum TriggerMode {
    #[default]
    Auto,
    Confirm,
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    #[serde(default)]
    pub replacement: String,
    #[serde(default = "def_true")]
    pub enabled: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub trigger_mode: TriggerMode,
    #[serde(default)]
    pub word_boundary: bool,
    #[serde(default)]
    pub mode: Option<TypingMode>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    #[serde(default = "def_mode")]
    pub default_mode: TypingMode,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub autostart: bool,
    /// Folder for configuration backups. Absent or empty means the default folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_mode: TypingMode::Fast,
            paused: false,
            autostart: false,
            backup_dir: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Config {
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub settings: Settings,
}

pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("KEYFLOW_CONFIG_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("KeyFlow")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn backup_path() -> PathBuf {
    data_dir().join("config.backup.json")
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    // Never overwrite content we could not read: park it and start empty.
                    let parked = data_dir().join("config.unreadable.json");
                    let _ = fs::rename(&path, &parked);
                    crate::exec::logline(&format!(
                        "config parse failed ({}); kept it as {}",
                        e,
                        parked.display()
                    ));
                    Config::default()
                }
            },
            Err(_) => {
                let cfg = Config::default();
                let _ = cfg.save();
                cfg
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = data_dir();
        fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(self).unwrap_or_default();
        let path = config_path();
        // Keep the previous revision so any bad write can be undone.
        if let Ok(previous) = fs::read(&path) {
            let _ = fs::write(backup_path(), previous);
        }
        // Write through a temp file and rename, so readers never see a half-written config.
        let tmp = dir.join("config.json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }
}

pub fn new_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("s{}", ms)
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    fn snippet(id: &str, trigger: &str) -> Snippet {
        Snippet {
            id: id.to_string(),
            trigger: trigger.to_string(),
            replacement: String::new(),
            enabled: true,
            case_sensitive: false,
            sensitive: false,
            trigger_mode: TriggerMode::Auto,
            word_boundary: false,
            mode: None,
        }
    }

    #[test]
    fn safe_save_and_unreadable_handling() {
        let dir = std::env::temp_dir().join(format!("keyflow-config-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("KEYFLOW_CONFIG_DIR", &dir);

        // The first save has nothing to back up yet.
        let mut first = Config::default();
        first.snippets.push(snippet("1", "aa"));
        first.save().unwrap();
        assert!(config_path().exists());
        assert!(!backup_path().exists());

        // The second save keeps the previous revision.
        let mut second = Config::default();
        second.snippets.push(snippet("2", "bb"));
        second.save().unwrap();
        let backup: Config = serde_json::from_str(&fs::read_to_string(backup_path()).unwrap()).unwrap();
        assert_eq!(backup.snippets.len(), 1);
        assert_eq!(backup.snippets[0].trigger, "aa");

        // Unreadable content is parked, never overwritten.
        fs::write(config_path(), "{ not json").unwrap();
        let loaded = Config::load();
        assert!(loaded.snippets.is_empty());
        let parked = dir.join("config.unreadable.json");
        assert_eq!(fs::read_to_string(&parked).unwrap(), "{ not json");

        std::env::remove_var("KEYFLOW_CONFIG_DIR");
        let _ = fs::remove_dir_all(&dir);
    }
}
