//! `zenctl urlbar` — Zen address-bar operations.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl urlbar search \"rust async book\"\n  zenctl urlbar search https://example.com --submit\n  zenctl urlbar actions\n  zenctl urlbar run history\n  zenctl urlbar close"
)]
pub enum Cmd {
    /// Open the address bar populated with a query (results panel shown).
    /// With --submit, execute the query (search or navigate) headlessly.
    Search {
        /// Query text or URL to place in the address bar.
        query: String,
        /// Execute the query instead of just opening the address bar.
        #[arg(long)]
        submit: bool,
    },
    /// Revert and close the address bar (the Esc-key behavior).
    Close,
    /// List Zen URL-bar global actions.
    Actions,
    /// Run a Zen URL-bar global action by id or label substring.
    Run { action: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Search { query, submit } => {
            let r = client
                .call_raw(
                    Method::UrlbarSearch,
                    json!({ "query": query, "submit": submit }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&r, &["query", "opened", "submitted"])
                );
            }
        }
        Cmd::Close => {
            let r = client.call_raw(Method::UrlbarClose, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["closed"]));
            }
        }
        Cmd::Actions => {
            let r = client
                .call_raw(Method::UrlbarActionsList, json!({}))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["count", "actions"]));
            }
        }
        Cmd::Run { action } => {
            let r = client
                .call_raw(Method::UrlbarActionsRun, json!({ "action": action }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["ran", "id", "label"]));
            }
        }
    }
    Ok(())
}
