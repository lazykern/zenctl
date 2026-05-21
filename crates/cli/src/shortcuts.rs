use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

fn shortcuts_file(profile: &std::path::Path) -> PathBuf {
    profile.join("zen-keyboard-shortcuts.json")
}

pub fn read_shortcuts() -> Result<Option<Value>> {
    let Some(profile) = crate::profile::detect_profile()? else {
        return Ok(None);
    };
    let path = shortcuts_file(&profile);
    if !path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(value))
}

pub fn write_shortcuts(data: &Value) -> Result<PathBuf> {
    let Some(profile) = crate::profile::detect_profile()? else {
        anyhow::bail!("no profile detected");
    };
    let path = shortcuts_file(&profile);
    let text = serde_json::to_string_pretty(data)?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}
