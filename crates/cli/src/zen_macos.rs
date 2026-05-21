//! Read-only AppleScript probes against Zen.

use anyhow::{anyhow, Result};
use std::process::Command;

pub fn count_windows() -> Result<u32> {
    let out = osascript(r#"tell application "Zen" to count windows"#)?;
    out.trim()
        .parse::<u32>()
        .map_err(|e| anyhow!("unexpected window count output {out:?}: {e}"))
}

fn osascript(script: &str) -> Result<String> {
    let out = Command::new("osascript").args(["-e", script]).output()?;
    if !out.status.success() {
        return Err(anyhow!(
            "osascript failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
