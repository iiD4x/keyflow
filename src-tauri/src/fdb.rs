use crate::config::{Snippet, TriggerMode};

pub struct FdbRule {
    pub trigger: String,
    pub replacement: String,
    pub name: String,
}

pub fn parse(text: &str) -> Vec<FdbRule> {
    let mut out: Vec<FdbRule> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        let first = match line.chars().next() {
            Some(c) => c,
            None => continue,
        };
        if first != '0' && first != '1' {
            continue;
        }
        let fields: Vec<&str> = line.split('•').collect();
        if fields.len() < 16 {
            continue;
        }
        let trigger = fields[1].trim();
        if trigger.is_empty() {
            continue;
        }
        if fields[12].trim() != "Send" {
            continue;
        }
        let replacement = fields[2].replace('¶', "\n");
        let name = fields[11].trim().to_string();
        if let Some(existing) = out.iter_mut().find(|r| r.trigger == trigger) {
            existing.replacement = replacement;
            existing.name = name;
        } else {
            out.push(FdbRule {
                trigger: trigger.to_string(),
                replacement,
                name,
            });
        }
    }
    out
}

pub fn to_snippets(rules: &[FdbRule], prefix: &str) -> Vec<Snippet> {
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| Snippet {
            id: format!("{}{}", prefix, i),
            trigger: r.trigger.clone(),
            replacement: r.replacement.clone(),
            enabled: true,
            case_sensitive: false,
            sensitive: false,
            trigger_mode: TriggerMode::Auto,
            word_boundary: false,
            mode: None,
        })
        .collect()
}

pub fn detect_path() -> Option<String> {
    let appdata = std::env::var("APPDATA").ok()?;
    let ini = std::path::Path::new(&appdata)
        .join("FastKeys")
        .join("settings.ini");
    let text = std::fs::read_to_string(ini).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("DataFile=") {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}
