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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_mode: TypingMode::Fast,
            paused: false,
            autostart: false,
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
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("KeyFlow")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(_) => {
                    let _ = fs::rename(&path, data_dir().join("config.broken.json"));
                    let cfg = Config::default();
                    let _ = cfg.save();
                    cfg
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
        fs::create_dir_all(data_dir())?;
        let text = serde_json::to_string_pretty(self).unwrap_or_default();
        fs::write(config_path(), text)
    }
}

pub fn new_id() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("s{}", ms)
}
