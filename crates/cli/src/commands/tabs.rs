//! `zenctl tabs` — tab operations.

use super::{confirm, CliOpts};
use crate::{
    client::Client,
    commands::target::{page_target, TargetArgs},
    output,
};
use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl tabs list\n  zenctl tabs list --current-window\n  zenctl tabs find --url-contains github\n  zenctl tabs open https://example.com\n  zenctl tabs open https://example.com --background\n  zenctl tabs close --active\n  zenctl tabs close 123\n  zenctl tabs activate --url-contains github\n  zenctl tabs move 123 --index 0\n  zenctl tabs reload --active --bypass-cache\n  zenctl tabs duplicate 123\n  zenctl tabs discard --url-contains youtube\n  zenctl tabs mute --active\n  zenctl tabs pin 123\n  zenctl tabs screenshot --active -o shot.png
  zenctl tabs screenshot --active --full-page -o full.png
  zenctl tabs group 123 456
  zenctl tabs ungroup 123 456"
)]
pub enum Cmd {
    /// List open tabs.
    List {
        #[arg(long)]
        current_window: bool,
        /// Filter by workspace name or UUID.
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Find tabs matching target criteria.
    Find {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Open a URL in a new tab.
    Open {
        url: String,
        #[arg(long)]
        background: bool,
    },
    /// Close a tab by id or target criteria.
    Close {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Activate (switch to) a tab.
    Activate {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Move a tab within or between windows.
    Move {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long)]
        index: i64,
        #[arg(long = "dest-window", id = "dest_window_id")]
        window_id: Option<i64>,
    },
    /// Reload a tab.
    Reload {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
        /// Bypass the cache (hard reload).
        #[arg(long)]
        bypass_cache: bool,
    },
    /// Duplicate a tab.
    Duplicate {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Discard a tab — unload it from memory, keep it in the tab strip.
    Discard {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Mute a tab.
    Mute {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Unmute a tab.
    Unmute {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Pin a tab.
    Pin {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Unpin a tab.
    Unpin {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Capture a screenshot of a tab.
    Screenshot {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
        /// Write the image to this file. Defaults to stdout.
        #[arg(long, short)]
        output: Option<std::path::PathBuf>,
        /// Capture as JPEG instead of PNG.
        #[arg(long)]
        jpeg: bool,
        /// JPEG quality (0-100). Ignored for PNG.
        #[arg(long, default_value = "92")]
        quality: u8,
        /// Capture the full page by scrolling and stitching tiles.
        #[arg(long)]
        full_page: bool,
    },
    /// Get or set the zoom factor of a tab.
    Zoom {
        /// Zoom factor to set (e.g. 1.5). Omit to read the current zoom.
        value: Option<f64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Toggle reader mode on a tab.
    Reader {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Navigate a tab back in its history.
    Back {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Navigate a tab forward in its history.
    Forward {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Detach a tab into its own window.
    Detach {
        tab_id: Option<i64>,
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Group one or more tabs using Firefox tab groups when available.
    Group {
        #[arg(required = true)]
        tab_ids: Vec<i64>,
        /// Existing group id to add tabs to.
        #[arg(long)]
        group_id: Option<i64>,
        /// Window id for creating a new group.
        #[arg(long)]
        window_id: Option<i64>,
    },
    /// Remove one or more tabs from their tab group.
    Ungroup {
        #[arg(required = true)]
        tab_ids: Vec<i64>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List {
            current_window,
            workspace,
        } => {
            let mut params = if *current_window {
                json!({ "current_window": true })
            } else {
                json!({})
            };
            if let Some(ws) = workspace {
                params["workspace"] = json!(ws);
            };
            let value = client.call_raw(Method::TabsList, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::tabs_table(&value);
            }
        }
        Cmd::Find { target } => {
            let value = client
                .call_raw(Method::TabsFind, page_target(target))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::tabs_table(&value);
            }
        }
        Cmd::Open { url, background } => {
            let value = client
                .call_raw(
                    Method::TabsOpen,
                    json!({ "url": url, "active": !background }),
                )
                .await?;
            println!("{}", output::short_summary(&value, &["id", "url"]));
        }
        Cmd::Close { tab_id, target } => {
            let detail = match tab_id {
                Some(id) => format!("tab {id}"),
                None => "tab (by target)".into(),
            };
            confirm(opts, "close tab", &detail)?;
            let params = match tab_id {
                Some(id) => json!({ "tab_id": id }),
                None => json!({ "tab_id": resolve_tab_id(client, target).await? }),
            };
            let value = client.call_raw(Method::TabsClose, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "url"]));
            }
        }
        Cmd::Activate { tab_id, target } => {
            let id = match tab_id {
                Some(id) => *id,
                None => resolve_tab_id(client, target).await?,
            };
            let value = client
                .call_raw(Method::TabsActivate, json!({ "tab_id": id }))
                .await?;
            println!("{}", output::short_summary(&value, &["id", "url"]));
        }
        Cmd::Move {
            tab_id,
            target,
            index,
            window_id,
        } => {
            let id = match tab_id {
                Some(id) => *id,
                None => resolve_tab_id(client, target).await?,
            };
            let mut params = json!({ "tab_id": id, "index": index });
            if let Some(w) = window_id {
                params["window_id"] = json!(w);
            }
            let value = client.call_raw(Method::TabsMove, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "index"]));
            }
        }
        Cmd::Reload {
            tab_id,
            target,
            bypass_cache,
        } => {
            let mut params = target_params(*tab_id, target);
            params["bypass_cache"] = json!(bypass_cache);
            let value = client.call_raw(Method::TabsReload, params).await?;
            emit(opts, &value, &["tab_id", "reloaded"])?;
        }
        Cmd::Duplicate { tab_id, target } => {
            let params = target_params(*tab_id, target);
            let value = client.call_raw(Method::TabsDuplicate, params).await?;
            emit(opts, &value, &["id", "url"])?;
        }
        Cmd::Discard { tab_id, target } => {
            let params = target_params(*tab_id, target);
            let value = client.call_raw(Method::TabsDiscard, params).await?;
            emit(opts, &value, &["tab_id", "discarded"])?;
        }
        Cmd::Mute { tab_id, target } => {
            let mut params = target_params(*tab_id, target);
            params["muted"] = json!(true);
            let value = client.call_raw(Method::TabsSetMuted, params).await?;
            emit(opts, &value, &["tab_id", "muted"])?;
        }
        Cmd::Unmute { tab_id, target } => {
            let mut params = target_params(*tab_id, target);
            params["muted"] = json!(false);
            let value = client.call_raw(Method::TabsSetMuted, params).await?;
            emit(opts, &value, &["tab_id", "muted"])?;
        }
        Cmd::Pin { tab_id, target } => {
            let mut params = target_params(*tab_id, target);
            params["pinned"] = json!(true);
            let value = client.call_raw(Method::TabsSetPinned, params).await?;
            emit(opts, &value, &["tab_id", "pinned"])?;
        }
        Cmd::Unpin { tab_id, target } => {
            let mut params = target_params(*tab_id, target);
            params["pinned"] = json!(false);
            let value = client.call_raw(Method::TabsSetPinned, params).await?;
            emit(opts, &value, &["tab_id", "pinned"])?;
        }
        Cmd::Screenshot {
            tab_id,
            target,
            output,
            jpeg,
            quality,
            full_page,
        } => {
            let mut params = target_params(*tab_id, target);
            if *jpeg {
                params["format"] = json!("jpeg");
                params["quality"] = json!(quality);
            }
            if *full_page {
                params["full_page"] = json!(true);
            }
            let value = client.call_raw(Method::TabsScreenshot, params).await?;
            let data_url = value
                .get("data_url")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("screenshot response missing data_url"))?;
            let b64 = data_url.split_once(',').map(|(_, d)| d).unwrap_or(data_url);
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| anyhow::anyhow!("decode screenshot: {e}"))?;
            match output {
                Some(path) => {
                    std::fs::write(path, &bytes)
                        .map_err(|e| anyhow::anyhow!("write {}: {e}", path.display()))?;
                    println!("wrote {} bytes -> {}", bytes.len(), path.display());
                }
                None => {
                    use std::io::Write;
                    std::io::stdout().write_all(&bytes)?;
                }
            }
        }
        Cmd::Zoom { value, target } => {
            let mut params = target_params(None, target);
            if let Some(v) = value {
                params["value"] = json!(v);
            }
            let resp = client.call_raw(Method::TabsZoom, params).await?;
            emit(opts, &resp, &["tab_id", "zoom"])?;
        }
        Cmd::Reader { tab_id, target } => {
            let params = target_params(*tab_id, target);
            let resp = client.call_raw(Method::TabsReader, params).await?;
            emit(opts, &resp, &["tab_id", "toggled"])?;
        }
        Cmd::Back { tab_id, target } => {
            let params = target_params(*tab_id, target);
            let resp = client.call_raw(Method::TabsGoBack, params).await?;
            emit(opts, &resp, &["tab_id", "navigated"])?;
        }
        Cmd::Forward { tab_id, target } => {
            let params = target_params(*tab_id, target);
            let resp = client.call_raw(Method::TabsGoForward, params).await?;
            emit(opts, &resp, &["tab_id", "navigated"])?;
        }
        Cmd::Detach { tab_id, target } => {
            let id = match tab_id {
                Some(id) => *id,
                None => resolve_tab_id(client, target).await?,
            };
            let value = client
                .call_raw(Method::TabDetach, json!({ "tab_id": id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let tab = value
                    .get("tab_id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into());
                let win = value
                    .get("window_id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into());
                println!("detached: tab {} -> window {}", tab, win);
            }
        }
        Cmd::Group {
            tab_ids,
            group_id,
            window_id,
        } => {
            let mut params = json!({ "tab_ids": tab_ids });
            if let Some(id) = group_id {
                params["group_id"] = json!(id);
            }
            if let Some(id) = window_id {
                params["create_properties"] = json!({ "windowId": id });
            }
            let value = client.call_raw(Method::TabGroup, params).await?;
            emit(opts, &value, &["group_id"])?;
        }
        Cmd::Ungroup { tab_ids } => {
            let value = client
                .call_raw(Method::TabUngroup, json!({ "tab_ids": tab_ids }))
                .await?;
            emit(opts, &value, &["ungrouped"])?;
        }
    }
    Ok(())
}

/// Build `{"target": {...}}` params, injecting an explicit tab id when given.
fn target_params(tab_id: Option<i64>, target: &TargetArgs) -> Value {
    let mut params = page_target(target);
    if let Some(id) = tab_id {
        params["target"]["tab_id"] = json!(id);
    }
    params
}

/// Print a mutation result: pretty JSON with --json, else a key=value summary.
fn emit(opts: &CliOpts, value: &Value, keys: &[&str]) -> Result<()> {
    if opts.json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", output::short_summary(value, keys));
    }
    Ok(())
}

/// Resolve a TargetArgs into a concrete tab id by calling TabsFind.
/// Uses the existing client (no second connection).
async fn resolve_tab_id(client: &mut Client, target: &TargetArgs) -> Result<i64> {
    let value = client
        .call_raw(Method::TabsFind, page_target(target))
        .await?;
    let Some(tab) = value.as_array().and_then(|arr| arr.first()) else {
        anyhow::bail!("no tab matched target selector");
    };
    tab.get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("matched tab has no id"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_helpers;
    use tokio::net::UnixStream;
    use zenctl_protocol::Method;

    #[tokio::test]
    async fn detach_sends_tab_id() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Detach {
            tab_id: Some(42),
            target: TargetArgs::default(),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::TabDetach);
        assert_eq!(params.get("tab_id").and_then(|v| v.as_i64()), Some(42));

        test_helpers::write_response(
            &mut server_side,
            serde_json::json!({"tab_id": 99, "window_id": 5, "url": "https://example.com"}),
        )
        .await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn group_sends_tab_ids() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: true,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Group {
            tab_ids: vec![1, 2],
            group_id: Some(7),
            window_id: Some(3),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::TabGroup);
        assert_eq!(params.get("tab_ids").unwrap(), &serde_json::json!([1, 2]));
        assert_eq!(params.get("group_id").and_then(|v| v.as_i64()), Some(7));
        assert_eq!(
            params
                .pointer("/create_properties/windowId")
                .and_then(|v| v.as_i64()),
            Some(3)
        );
        test_helpers::write_response(&mut server_side, serde_json::json!({"group_id": 7})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn ungroup_sends_tab_ids() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: true,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Ungroup {
            tab_ids: vec![1, 2],
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::TabUngroup);
        assert_eq!(params.get("tab_ids").unwrap(), &serde_json::json!([1, 2]));
        test_helpers::write_response(&mut server_side, serde_json::json!({"ungrouped": [1, 2]}))
            .await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn detach_error_tab_not_found() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Detach {
            tab_id: Some(999),
            target: TargetArgs::default(),
        };

        let handle = tokio::spawn(async move {
            let result = run(&mut client, &opts, &cmd).await;
            assert!(result.is_err(), "detach should fail when tab not found");
        });

        let (_id, _method, _params, _to) = test_helpers::read_request(&mut server_side).await;

        test_helpers::write_error(&mut server_side, "tab 999 not found").await;
        handle.await.unwrap();
    }
}
