//! `zenctl checkpoint` — small recovery helpers for automation.

use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

use super::CliOpts;
use crate::{client::Client, output};

#[derive(Subcommand)]
pub enum Cmd {
    /// Create a timestamped sessionstore backup.
    Create,
    /// List current session tabs from the profile sessionstore.
    List {
        /// Return full session JSON instead of the tab table.
        #[arg(long)]
        full: bool,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Create => {
            let value = client.call_raw(Method::SessionBackup, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let path = value
                    .get("backup")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("checkpoint: {path}");
            }
        }
        Cmd::List { full } => {
            let value = client
                .call_raw(Method::SessionList, json!({ "tab_list": !full }))
                .await?;
            if opts.json || *full {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::session_tabs_table(&value);
            }
        }
    }
    Ok(())
}
