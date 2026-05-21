//! `zenctl sessions` — recently closed tabs and windows.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl sessions closed\n  zenctl sessions closed --limit 5\n  zenctl sessions restore\n  zenctl sessions restore <session-id>\n  zenctl sessions restore-window\n  zenctl sessions restore-tab <session-id>"
)]
pub enum Cmd {
    /// List recently closed tabs and windows.
    Closed {
        /// Maximum number of entries to return.
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Restore a closed tab/window. Restores the most recent if no id is given.
    Restore {
        /// Session id from `sessions closed`.
        session_id: Option<String>,
    },
    /// Restore a recently closed window. Restores the most recent window if no id is given.
    RestoreWindow {
        /// Window session id from `sessions closed`.
        session_id: Option<String>,
    },
    /// Restore a recently closed tab. Restores the most recent tab if no id is given.
    RestoreTab {
        /// Tab session id from `sessions closed`.
        session_id: Option<String>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Closed { limit } => {
            let mut params = json!({});
            if let Some(n) = limit {
                params["max_results"] = json!(n);
            }
            let value = client.call_raw(Method::SessionsClosed, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::sessions_table(&value);
            }
        }
        Cmd::Restore { session_id } => {
            restore(client, opts, Method::SessionsRestore, session_id).await?;
        }
        Cmd::RestoreWindow { session_id } => {
            restore(client, opts, Method::SessionRestoreWindow, session_id).await?;
        }
        Cmd::RestoreTab { session_id } => {
            restore(client, opts, Method::SessionRestoreTab, session_id).await?;
        }
    }
    Ok(())
}

async fn restore(
    client: &mut Client,
    opts: &CliOpts,
    method: Method,
    session_id: &Option<String>,
) -> Result<()> {
    let mut params = json!({});
    if let Some(id) = session_id {
        params["session_id"] = json!(id);
    }
    let value = client.call_raw(method, params).await?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        match value.get("tab").or_else(|| value.get("window")) {
            Some(obj) => println!(
                "restored: {}",
                output::short_summary(obj, &["id", "title", "url"])
            ),
            None => println!("restored"),
        }
    }
    Ok(())
}
