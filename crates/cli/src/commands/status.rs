//! `zenctl status` — show host, extension, and browser status.

use super::CliOpts;
use crate::client::{self, Client};
use anyhow::Result;

pub async fn run(client: &mut Client, opts: &CliOpts) -> Result<()> {
    let s = client.status().await?;
    let sock = client::socket_path();
    let sock_override = std::env::var_os("ZENCTL_SOCKET").is_some();
    if opts.json {
        let mut v = serde_json::to_value(&s)?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "socket".into(),
                serde_json::json!(sock.display().to_string()),
            );
            obj.insert("socket_from_env".into(), serde_json::json!(sock_override));
        }
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        println!(
            "host:      {} (protocol v{})",
            s.daemon_version, s.protocol_version
        );
        println!(
            "extension: {}",
            if s.extension_connected {
                "connected"
            } else {
                "not connected"
            }
        );
        let zen_label = if s.zen_running {
            match s.zen_pid {
                Some(pid) => format!("running (pid {pid})"),
                None => "running".to_string(),
            }
        } else {
            "not running".to_string()
        };
        println!("zen:       {zen_label}");
        if let Some(c) = s.window_count {
            println!("windows:   {c}");
        }
        if let Some(p) = s.profile_path {
            println!("profile:   {p}");
        }
        let src = if sock_override {
            " (ZENCTL_SOCKET)"
        } else {
            ""
        };
        println!("socket:    {}{}", sock.display(), src);
        if s.stale_extension {
            let loaded = s.loaded_extension_hash.as_deref().unwrap_or("?");
            let bundled = s.bundled_extension_hash.as_deref().unwrap_or("?");
            eprintln!();
            eprintln!(
                "⚠  loaded extension is stale (loaded={}, bundled={}).",
                short_hash(loaded),
                short_hash(bundled)
            );
            eprintln!("   Run `zenctl install` and re-add the extension in about:debugging.");
        }
    }
    Ok(())
}

fn short_hash(s: &str) -> String {
    if s.len() > 12 {
        format!("{}…", &s[..12])
    } else {
        s.to_string()
    }
}
