use crate::config::Config;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const PREFIX: &str = "keyflow-backup-";

pub fn default_dir() -> PathBuf {
    crate::config::data_dir().join("backups")
}

pub fn resolved_dir(cfg: &Config) -> PathBuf {
    match cfg.settings.backup_dir.as_deref() {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => default_dir(),
    }
}

/// Local wall-clock stamp, so the file name matches what the user's clock says.
pub fn stamp() -> String {
    let mut st = crate::win::SYSTEMTIME {
        wYear: 0,
        wMonth: 0,
        wDayOfWeek: 0,
        wDay: 0,
        wHour: 0,
        wMinute: 0,
        wSecond: 0,
        wMilliseconds: 0,
    };
    unsafe { crate::win::GetLocalTime(&mut st) };
    if st.wYear == 0 {
        let secs = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        return format!("epoch-{}", secs);
    }
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

pub fn snapshot_stem() -> String {
    format!("{}{}", PREFIX, stamp())
}

fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let mut target = dir.join(format!("{}.json", stem));
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{}-{}.json", stem, n));
        n += 1;
    }
    target
}

/// Copy the on-disk configuration into `dir` and return the file that was written.
pub fn write_snapshot(dir: &Path, stem: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let text = fs::read_to_string(crate::config::config_path())?;
    let target = free_path(dir, stem);
    fs::write(&target, text)?;
    Ok(target)
}

/// Copy a configuration file somewhere the user chose.
pub fn copy_to(source: &Path, target: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(source)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, text)
}

#[derive(Serialize, Clone)]
pub struct BackupEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
    pub snippets: Option<usize>,
}

pub fn entry_for(path: &Path) -> Option<BackupEntry> {
    let meta = fs::metadata(path).ok()?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let snippets = fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Config>(&t).ok())
        .map(|c| c.snippets.len());
    Some(BackupEntry {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: path.display().to_string(),
        size: meta.len(),
        modified_ms,
        snippets,
    })
}

pub fn list(dir: &Path) -> Vec<BackupEntry> {
    let mut out: Vec<BackupEntry> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".json") {
            continue;
        }
        if let Some(e) = entry_for(&path) {
            out.push(e);
        }
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out
}

