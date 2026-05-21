//! `zenctl essentials` — Zen essential (pinned) tab operations.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(after_help = "EXAMPLES:\n  \
    zenctl essentials list\n  \
    zenctl essentials add --tab-id 42 --tab-id 43\n  \
    zenctl essentials add --url https://example.com\n  \
    zenctl essentials remove --tab-id 42\n  \
    zenctl essentials remove --url https://example.com --keep-pinned\n  \
    zenctl essentials reset --tab-id 42\n  \
    zenctl essentials replace-url --tab-id 42")]
pub enum Cmd {
    /// List all essential (zen-essential) tabs across every window.
    List,
    /// Promote existing tabs to essentials (by tab id or url).
    Add {
        /// Tab id to add. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
    },
    /// Remove tabs from essentials (by tab id or url).
    Remove {
        /// Tab id to remove. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
        /// Keep the tab pinned instead of unpinning it after removal.
        #[arg(long = "keep-pinned")]
        keep_pinned: bool,
    },
    /// Reset a pinned/essential tab to its stored URL (revert navigation).
    Reset {
        /// Tab id to reset. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
    },
    /// Commit a pinned tab's current URL as its new stored URL.
    ReplaceUrl {
        /// Tab id to update. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let r = client.call_raw(Method::EssentialsList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                print_essentials(&r);
            }
        }
        Cmd::Add { tab_ids, url } => {
            require_target(tab_ids, url)?;
            let r = client
                .call_raw(
                    Method::EssentialsAdd,
                    json!({ "tab_ids": tab_ids, "urls": url }),
                )
                .await?;
            emit(opts, &r, &["added", "skipped"])?;
        }
        Cmd::Remove {
            tab_ids,
            url,
            keep_pinned,
        } => {
            require_target(tab_ids, url)?;
            let r = client
                .call_raw(
                    Method::EssentialsRemove,
                    json!({ "tab_ids": tab_ids, "urls": url, "unpin": !keep_pinned }),
                )
                .await?;
            emit(opts, &r, &["removed", "skipped", "unpinned"])?;
        }
        Cmd::Reset { tab_ids, url } => {
            require_target(tab_ids, url)?;
            let r = client
                .call_raw(
                    Method::EssentialsReset,
                    json!({ "tab_ids": tab_ids, "urls": url }),
                )
                .await?;
            emit(opts, &r, &["reset", "skipped"])?;
        }
        Cmd::ReplaceUrl { tab_ids, url } => {
            require_target(tab_ids, url)?;
            let r = client
                .call_raw(
                    Method::EssentialsReplaceUrl,
                    json!({ "tab_ids": tab_ids, "urls": url }),
                )
                .await?;
            emit(opts, &r, &["replaced", "skipped"])?;
        }
    }
    Ok(())
}

fn require_target(tab_ids: &[i64], url: &[String]) -> Result<()> {
    if tab_ids.is_empty() && url.is_empty() {
        anyhow::bail!("provide --tab-id and/or --url at least once");
    }
    Ok(())
}

fn emit(opts: &CliOpts, r: &serde_json::Value, keys: &[&str]) -> Result<()> {
    if opts.json {
        println!("{}", serde_json::to_string_pretty(r)?);
    } else {
        println!("{}", output::short_summary(r, keys));
    }
    Ok(())
}

fn print_essentials(value: &serde_json::Value) {
    let essentials = value.get("essentials").and_then(|v| v.as_array());
    let count = essentials.map(|e| e.len()).unwrap_or(0);
    if count == 0 {
        println!("no essential tabs");
        return;
    }
    println!("{count} essential tab(s):");
    for e in essentials.unwrap() {
        let title = e.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let url = e.get("url").and_then(|v| v.as_str()).unwrap_or("");
        println!("  - {title}  {url}");
    }
}
