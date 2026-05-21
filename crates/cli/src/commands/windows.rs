//! `zenctl windows` — window operations.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// List all open windows.
    List,
    /// Focus a window by id.
    Focus { window_id: i64 },
    /// Close a window by id.
    Close { window_id: i64 },
    /// Open a new window.
    Create {
        /// URL to open in the new window.
        #[arg(long)]
        url: Option<String>,
        /// Open as a private window.
        #[arg(long)]
        incognito: bool,
        /// Initial window state.
        #[arg(long, value_parser = ["normal", "minimized", "maximized", "fullscreen"])]
        state: Option<String>,
    },
    /// Change a window's state.
    State {
        window_id: i64,
        #[arg(value_parser = ["normal", "minimized", "maximized", "fullscreen"])]
        state: String,
    },
    /// Force cross-window workspace sync via gZenWorkspaces.propagateWorkspacesToAllWindows().
    Sync,
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let value = client.call_raw(Method::WindowsList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::windows_table(&value);
            }
        }
        Cmd::Focus { window_id } => {
            let value = client
                .call_raw(Method::WindowsFocus, json!({ "window_id": window_id }))
                .await?;
            println!("{}", output::short_summary(&value, &["id", "focused"]));
        }
        Cmd::Close { window_id } => {
            confirm(opts, "close window", &format!("window {window_id}"))?;
            let value = client
                .call_raw(Method::WindowsClose, json!({ "window_id": window_id }))
                .await?;
            println!(
                "{}",
                output::short_summary(&value, &["closed", "window_id"])
            );
        }
        Cmd::Create {
            url,
            incognito,
            state,
        } => {
            let mut params = json!({});
            if let Some(u) = url {
                params["url"] = json!(u);
            }
            if *incognito {
                params["incognito"] = json!(true);
            }
            if let Some(s) = state {
                params["state"] = json!(s);
            }
            let value = client.call_raw(Method::WindowsCreate, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "state"]));
            }
        }
        Cmd::State { window_id, state } => {
            let value = client
                .call_raw(
                    Method::WindowsUpdate,
                    json!({ "window_id": window_id, "state": state }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "state"]));
            }
        }
        Cmd::Sync => {
            let value = client.call_raw(Method::WindowSyncForce, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let synced = value
                    .get("synced")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                println!("workspace sync: {}", if synced { "done" } else { "failed" });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_helpers;
    use tokio::net::UnixStream;
    use zenctl_protocol::Method;

    #[tokio::test]
    async fn sync_sends_window_sync_force() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Sync;

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::WindowSyncForce);
        assert_eq!(params, serde_json::json!({}));

        test_helpers::write_response(&mut server_side, serde_json::json!({"synced": true})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn sync_error_path() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Sync;

        let handle = tokio::spawn(async move {
            let result = run(&mut client, &opts, &cmd).await;
            assert!(
                result.is_err(),
                "sync should fail when server returns error"
            );
        });

        let (_id, _method, _params, _to) = test_helpers::read_request(&mut server_side).await;

        // Simulate an extension error (e.g. experiment API unavailable).
        test_helpers::write_error(
            &mut server_side,
            "gZenWorkspaces.propagateWorkspacesToAllWindows unavailable",
        )
        .await;
        handle.await.unwrap();
    }
}
