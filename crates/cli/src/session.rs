//! Read Firefox sessionstore files (.jsonlz4 / mozLz4 format).
//!
//! The sessionstore contains open windows and tabs.  The file is `mozLz40\0`
//! header (8 bytes) followed by raw lz4-compressed JSON.  We decompress and
//! return structured data so the CLI can list tabs or back up the file.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Header that Firefox writes at the start of every .jsonlz4 file.
const MOZLZ4_HEADER: &[u8] = b"mozLz40\0";

/// Locate the best candidate sessionstore file relative to the profile dir.
fn find_sessionstore(profile: &Path) -> Option<PathBuf> {
    let candidates = [
        profile
            .join("sessionstore-backups")
            .join("recovery.jsonlz4"),
        profile.join("sessionstore-backups").join("recovery.baklz4"),
        profile.join("sessionstore.jsonlz4"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

/// Read and decompress a mozLz4 file, returning parsed session store JSON
/// as a generic serde_json::Value (Firefox sessionstore has hundreds of
/// fields — we only extract what we need).
pub fn read_sessionstore(path: &Path) -> Result<Value> {
    let raw = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;

    if raw.len() < 8 || &raw[..8] != MOZLZ4_HEADER {
        anyhow::bail!(
            "{} is not a valid mozLz4 file (missing header)",
            path.display()
        );
    }

    // Firefox writes: 8-byte header + 4-byte uncompressed size (LE) + lz4 data.
    let compressed = &raw[12..];
    let uncompressed_size = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;

    let mut decompressed = vec![0u8; uncompressed_size];
    let written = lz4_flex::block::decompress_into(compressed, &mut decompressed)
        .with_context(|| format!("lz4 decompress failed for {}", path.display()))?;

    if written != uncompressed_size {
        anyhow::bail!("lz4 size mismatch: expected {uncompressed_size}, got {written}");
    }

    let text =
        std::str::from_utf8(&decompressed[..written]).context("sessionstore is not valid UTF-8")?;

    match serde_json::from_str::<Value>(text) {
        Ok(v) => Ok(v),
        Err(e) => {
            // Include a snippet of the text around the error position
            let line = e.line();
            let col = e.column();
            let start = text
                .char_indices()
                .nth(col.saturating_sub(1))
                .map(|(i, _)| i)
                .unwrap_or(0);
            let snippet = if start + 120 < text.len() {
                &text[start..start + 120]
            } else {
                &text[start..]
            };
            Err(anyhow::anyhow!(
                "JSON parse error at line {line} col {col}: {e}. Context: ...{snippet}..."
            ))
            .context(format!(
                "parse sessionstore JSON from {} ({} bytes decompressed)",
                path.display(),
                written
            ))
        }
    }
}

/// Try to read the sessionstore for the detected profile.  Returns
/// JSON Value or `None` if no session file is found.
pub fn read_current_session() -> Result<Option<Value>> {
    let Some(profile) = crate::profile::detect_profile()? else {
        return Ok(None);
    };
    let Some(path) = find_sessionstore(&profile) else {
        return Ok(None);
    };
    Ok(Some(read_sessionstore(&path)?))
}

/// Flatten tabs into a simple list from a sessionstore Value.
/// Each entry includes the window index, tab index, URL, title, and pinned state.
pub fn tab_list(session: &Value) -> Vec<TabSummary> {
    let mut out = Vec::new();
    let Some(windows) = session.get("windows").and_then(|v| v.as_array()) else {
        return out;
    };
    for (wi, w) in windows.iter().enumerate() {
        let tabs = match w.get("tabs").and_then(|v| v.as_array()) {
            Some(t) => t,
            None => continue,
        };
        let selected = w.get("selected").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        for (ti, t) in tabs.iter().enumerate() {
            let entries = t.get("entries").and_then(|v| v.as_array());
            let index = t.get("index").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let entry = entries.and_then(|e| e.get(index.saturating_sub(1)));

            out.push(TabSummary {
                window: wi,
                tab: ti,
                active: selected.saturating_sub(1) == ti,
                pinned: t.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false),
                url: entry
                    .and_then(|e| e.get("url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                title: entry
                    .and_then(|e| e.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    out
}

#[derive(Debug, serde::Serialize)]
pub struct TabSummary {
    pub window: usize,
    pub tab: usize,
    pub active: bool,
    pub pinned: bool,
    pub url: String,
    pub title: String,
}

/// Copy the current sessionstore to a timestamped backup file and return
/// the backup path.  Destination: `<profile>/sessionstore-backups/zenctl-<name>-<ts>.jsonlz4`
pub fn backup_sessionstore() -> Result<Option<PathBuf>> {
    let Some(profile) = crate::profile::detect_profile()? else {
        anyhow::bail!("no profile detected");
    };
    let Some(src) = find_sessionstore(&profile) else {
        return Ok(None);
    };

    let backups = profile.join("sessionstore-backups");
    std::fs::create_dir_all(&backups).with_context(|| format!("create {}", backups.display()))?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = profile
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("session");
    let dest = backups.join(format!("zenctl-{name}-{ts}.jsonlz4"));

    std::fs::copy(&src, &dest)
        .with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;

    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mozlz4_header_constant() {
        assert_eq!(MOZLZ4_HEADER, b"mozLz40\0");
        assert_eq!(MOZLZ4_HEADER.len(), 8);
    }

    #[test]
    fn test_tab_list_empty() {
        let session = serde_json::json!({});
        assert!(tab_list(&session).is_empty());
    }

    #[test]
    fn test_tab_list_basic() {
        let session = serde_json::json!({
            "windows": [{
                "tabs": [{
                    "entries": [{
                        "url": "https://example.com",
                        "title": "Example"
                    }],
                    "index": 1,
                    "pinned": false
                }],
                "selected": 1
            }]
        });
        let list = tab_list(&session);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].url, "https://example.com");
        assert!(list[0].active);
    }
}
