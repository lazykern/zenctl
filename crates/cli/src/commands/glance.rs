//! `zenctl glance` — glance overlay operations.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// List open glance overlays.
    List,
    /// Close all glance overlays.
    CloseAll,
    /// Close the current glance overlay.
    Close {
        /// Target a specific tab by id (activates it first).
        #[arg(long)]
        tab_id: Option<i64>,
    },
    /// Fully expand the current glance into a regular tab.
    Expand {
        /// Target a specific tab by id (activates it first).
        #[arg(long)]
        tab_id: Option<i64>,
    },
    /// Open a glance overlay on a URL.
    Open { url: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let r = client.call_raw(Method::GlanceList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&r)?);
            }
        }
        Cmd::CloseAll => {
            let r = client.call_raw(Method::GlanceCloseAll, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                let closed = r.get("closed").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("glance: closed {closed}");
            }
        }
        Cmd::Close { tab_id } => {
            let r = client
                .call_raw(Method::GlanceClose, json!({ "tab_id": tab_id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("glance: closed");
            }
        }
        Cmd::Expand { tab_id } => {
            let r = client
                .call_raw(Method::GlanceExpand, json!({ "tab_id": tab_id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("glance: expanded");
            }
        }
        Cmd::Open { url } => {
            let r = client
                .call_raw(Method::GlanceOpen, json!({ "url": url }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("glance: opened {url}");
            }
        }
    }
    Ok(())
}
