//! `zenctl ext` — WebExtension maintenance.

use super::CliOpts;
use crate::{
    client::Client,
    install::{self, ManifestVariant},
};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Reload the WebExtension (picks up edited experiment scripts).
    Reload,
    /// Switch the active extension manifest variant (basic or privileged).
    /// Creates extension/manifest.json as a symlink to the chosen variant.
    Use {
        /// Variant to activate.
        #[arg(value_enum)]
        variant: ManifestVariant,
        /// Path to the extension directory (default: walks up from CWD).
        #[arg(long)]
        dir: Option<std::path::PathBuf>,
    },
    /// Toggle verbose request/response logging in the extension console.
    /// With no argument, prints the current state.
    Debug {
        /// `on` or `off`. Omit to query current state.
        state: Option<DebugState>,
    },
    /// Print a detailed extension diagnostics report (variant in use,
    /// fingerprint match, experiment-API liveness, socket, …).
    Status,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum DebugState {
    On,
    Off,
}

/// `ext use` is handled before daemon connect (no client needed).
pub async fn run_use(variant: ManifestVariant, dir: Option<&std::path::Path>) -> Result<()> {
    let ext_dir = match dir {
        Some(p) => p.to_path_buf(),
        None => install::find_extension_dir()?,
    };
    install::link_manifest(variant, &ext_dir)?;
    println!("manifest.json → {}", variant.filename());
    println!("dir: {}", ext_dir.display());
    if matches!(variant, ManifestVariant::Privileged) {
        println!();
        println!("Privileged variant needs `extensions.experiments.enabled = true`");
        println!("in about:config");
    }

    // Best-effort reload — skip silently if Zen is not running.
    match Client::connect().await {
        Ok(mut c) => {
            let r = c.call_raw(Method::ExtReload, json!({})).await;
            match r {
                Ok(_) => println!("ext: reloading"),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("extension not connected") || msg.contains("timed out") {
                        println!("ext: reload requested (connection dropped as expected)");
                    } else {
                        println!("ext: reload failed: {e}");
                    }
                }
            }
        }
        Err(_) => {
            println!("ext: Zen not running — reload manually in about:debugging");
        }
    }
    Ok(())
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Reload => {
            let r = client.call_raw(Method::ExtReload, json!({})).await;
            match r {
                Ok(_) => println!("ext: reloading"),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("extension not connected") || msg.contains("timed out") {
                        println!("ext: reload requested (connection dropped as expected)");
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Cmd::Debug { state } => {
            let params = match state {
                Some(DebugState::On) => json!({ "enabled": true }),
                Some(DebugState::Off) => json!({ "enabled": false }),
                None => json!({}),
            };
            let resp = client.call_raw(Method::ExtDebug, params).await?;
            let enabled = resp
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if opts.json {
                println!("{}", serde_json::to_string(&resp)?);
            } else {
                println!("ext debug: {}", if enabled { "on" } else { "off" });
            }
        }
        Cmd::Status => {
            let s = client.status().await?;
            // CLI sockets are one-shot — the host reads one request, writes
            // one response, closes. Spawn a fresh client for capabilities.
            let caps = match Client::connect().await {
                Ok(mut c2) => c2.capabilities().await.unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let zen_chrome = caps
                .iter()
                .find(|c| matches!(c.method, Method::CompactToggle))
                .map(|c| c.available)
                .unwrap_or(false);
            let zen_prefs = caps
                .iter()
                .find(|c| matches!(c.method, Method::PrefsGet))
                .map(|c| c.available)
                .unwrap_or(false);
            let active_link = active_variant_link();
            let sock = crate::client::socket_path();
            if opts.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "host_version": s.daemon_version,
                        "protocol_version": s.protocol_version,
                        "extension_connected": s.extension_connected,
                        "stale_extension": s.stale_extension,
                        "bundled_hash": s.bundled_extension_hash,
                        "loaded_hash": s.loaded_extension_hash,
                        "zen_chrome_live": zen_chrome,
                        "zen_prefs_live": zen_prefs,
                        "active_variant": active_link,
                        "socket": sock.display().to_string(),
                    }))?
                );
            } else {
                println!(
                    "host:             {} (protocol v{})",
                    s.daemon_version, s.protocol_version
                );
                println!(
                    "extension:        {}",
                    if s.extension_connected {
                        "connected"
                    } else {
                        "not connected"
                    }
                );
                if let Some(b) = s.bundled_extension_hash.as_deref() {
                    let loaded = s.loaded_extension_hash.as_deref().unwrap_or("?");
                    let match_str = if s.stale_extension { "STALE" } else { "match" };
                    println!(
                        "fingerprint:      {} (loaded={}, bundled={}) [{}]",
                        if s.stale_extension { "⚠" } else { "✓" },
                        short_hash(loaded),
                        short_hash(b),
                        match_str
                    );
                }
                println!(
                    "experiments:      zenChrome={} zenPrefs={}",
                    if zen_chrome { "live" } else { "missing" },
                    if zen_prefs { "live" } else { "missing" }
                );
                if let Some(v) = active_link {
                    println!("active variant:   {v}");
                }
                println!("socket:           {}", sock.display());
                if s.stale_extension {
                    println!();
                    println!("⚠  The extension Zen has loaded is older than this binary's bundle.");
                    println!("   Run `zenctl install` and reload the extension (or `zenctl ext reload`).");
                }
            }
        }
        Cmd::Use { .. } => unreachable!("handled before daemon connect"),
    }
    Ok(())
}

fn active_variant_link() -> Option<String> {
    let ext = install::find_extension_dir().ok()?;
    let target = std::fs::read_link(ext.join("manifest.json")).ok()?;
    target.file_name().map(|n| n.to_string_lossy().to_string())
}

fn short_hash(s: &str) -> String {
    if s.len() > 12 {
        format!("{}…", &s[..12])
    } else {
        s.to_string()
    }
}
