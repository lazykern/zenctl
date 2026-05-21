//! `zenctl session` — read/backup Firefox sessionstore.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl session list\n  zenctl session list --full    # raw JSON\n  zenctl session backup"
)]
pub enum Cmd {
    /// List open tabs from the current sessionstore.
    List {
        /// Return full session JSON instead of tab list.
        #[arg(long)]
        full: bool,
    },
    /// Back up the current sessionstore to a timestamped copy.
    Backup,
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List { full } => {
            let params = json!({ "tab_list": !full });
            let value = client.call_raw(Method::SessionList, params).await?;
            if opts.json || *full {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::session_tabs_table(&value);
            }
        }
        Cmd::Backup => {
            let value = client.call_raw(Method::SessionBackup, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let path = value
                    .get("backup")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("backup: {path}");
            }
        }
    }
    Ok(())
}
