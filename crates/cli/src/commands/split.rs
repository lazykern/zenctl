//! `zenctl split` — split view operations.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Split the current context tabs.
    Create {
        /// Layout: grid, vsep, hsep, unsplit.
        #[arg(long, default_value = "grid")]
        layout: String,
        /// Tab IDs to split together (pass multiple times). Omit to use current context.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
    },
    /// Unsplit the current split view.
    Unsplit,
    /// List active split groups across all windows.
    List,
    /// Add tabs to the active/existing split group.
    AddTab {
        /// Tab IDs to add (pass multiple times).
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// Layout: grid, vsep, hsep.
        #[arg(long)]
        layout: Option<String>,
    },
    /// Change the active split layout: grid, vsep, hsep.
    SetLayout { layout: String },
    /// Resize children of a split layout node.
    Resize {
        /// Node path from `split list --json` layout tree. Empty path = root; e.g. 0 or 1.0.
        #[arg(long, default_value = "")]
        path: String,
        /// Comma-separated child sizes in percent. Must match child count and sum to 100.
        #[arg(long, value_delimiter = ',')]
        sizes: Vec<f64>,
    },
    /// Toggle drag-reorder mode for the active split view's panes.
    Rearrange {
        /// Enable or disable rearrange mode (default: enable).
        #[arg(long, default_value = "true")]
        enable: bool,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Create { layout, tab_ids } => {
            let params = json!({ "grid_type": layout, "tab_ids": tab_ids });
            let r = client.call_raw(Method::SplitViewCreate, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("split: created");
            }
        }
        Cmd::Unsplit => {
            let r = client.call_raw(Method::SplitUnsplit, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("split: unsplit");
            }
        }
        Cmd::List => {
            let r = client.call_raw(Method::SplitViewList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                print_split_list(&r);
            }
        }
        Cmd::AddTab { tab_ids, layout } => {
            let r = client
                .call_raw(
                    Method::SplitViewAddTab,
                    json!({ "tab_ids": tab_ids, "grid_type": layout }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("split: added tab(s)");
            }
        }
        Cmd::SetLayout { layout } => {
            let r = client
                .call_raw(Method::SplitViewSetLayout, json!({ "grid_type": layout }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("split: layout set");
            }
        }
        Cmd::Resize { path, sizes } => {
            if sizes.is_empty() {
                anyhow::bail!("provide --sizes, e.g. --sizes 30,70");
            }
            let r = client
                .call_raw(
                    Method::SplitViewResize,
                    json!({ "path": path, "sizes": sizes }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("split: resized");
            }
        }
        Cmd::Rearrange { enable } => {
            let r = client
                .call_raw(Method::SplitViewRearrange, json!({ "enable": enable }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                let state = r
                    .get("rearranging")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                println!(
                    "split: rearrange {} ({})",
                    if state { "on" } else { "off" },
                    if state {
                        "drag panes to reorder"
                    } else {
                        "reorder disabled"
                    }
                );
            }
        }
    }
    Ok(())
}

fn print_split_list(r: &serde_json::Value) {
    let groups = r.get("groups").and_then(|g| g.as_array());
    let count = groups.map(|g| g.len()).unwrap_or(0);
    if count == 0 {
        println!("no active splits");
        return;
    }
    let active = r
        .get("active_group_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    println!("{} split group(s):", count);
    for g in groups.unwrap() {
        let id = g.get("group_id").and_then(|v| v.as_str()).unwrap_or("?");
        let grid = g.get("grid_type").and_then(|v| v.as_str()).unwrap_or("?");
        let tabs = g.get("tabs").and_then(|v| v.as_array());
        let marker = if id == active { "*" } else { " " };
        println!(
            "  {} {} [{}] {} tab(s)",
            marker,
            id,
            grid,
            tabs.map(|t| t.len()).unwrap_or(0)
        );
        if let Some(tabs) = tabs {
            for t in tabs {
                let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
                println!("      - {}  {}", title, url);
            }
        }
    }
    println!("(* = active split)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_helpers;
    use tokio::net::UnixStream;
    use zenctl_protocol::Method;

    #[tokio::test]
    async fn rearrange_enable() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Rearrange { enable: true };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::SplitViewRearrange);
        assert_eq!(params.get("enable").and_then(|v| v.as_bool()), Some(true));

        test_helpers::write_response(&mut server_side, serde_json::json!({"rearranging": true}))
            .await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn rearrange_disable() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Rearrange { enable: false };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::SplitViewRearrange);
        assert_eq!(params.get("enable").and_then(|v| v.as_bool()), Some(false));

        test_helpers::write_response(&mut server_side, serde_json::json!({"rearranging": false}))
            .await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn resize_sends_path_and_sizes() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Resize {
            path: "0".into(),
            sizes: vec![30.0, 70.0],
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::SplitViewResize);
        assert_eq!(params.get("path").and_then(|v| v.as_str()), Some("0"));
        let sizes: Vec<f64> = params
            .get("sizes")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        assert_eq!(sizes, vec![30.0, 70.0]);

        test_helpers::write_response(&mut server_side, serde_json::json!({"resized": true})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn create_sends_layout_and_tab_ids() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Create {
            layout: "hsep".into(),
            tab_ids: vec![1, 2, 3],
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::SplitViewCreate);
        assert_eq!(
            params.get("grid_type").and_then(|v| v.as_str()),
            Some("hsep")
        );
        let ids: Vec<i64> = params
            .get("tab_ids")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);

        test_helpers::write_response(&mut server_side, serde_json::json!({"split": true})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn resize_empty_sizes_fails() {
        let (client_side, _server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Resize {
            path: "0".into(),
            sizes: vec![],
        };

        let result = run(&mut client, &opts, &cmd).await;
        assert!(result.is_err(), "resize with empty sizes should fail");
        assert!(
            result.unwrap_err().to_string().contains("sizes"),
            "error should mention sizes"
        );
    }
}
