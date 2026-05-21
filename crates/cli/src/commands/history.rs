//! `zenctl history` — history search.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Search browsing history.
    Search {
        query: String,
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    /// Delete a history entry by URL.
    Delete { url: String },
    /// Add a URL to history.
    Add {
        url: String,
        #[arg(long)]
        title: Option<String>,
    },
    /// List recorded visits for a URL.
    Visits { url: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Search { query, limit } => {
            let value = client
                .call_raw(
                    Method::HistorySearch,
                    json!({ "query": query, "max_results": limit }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::history_table(&value);
            }
        }
        Cmd::Delete { url } => {
            let value = client
                .call_raw(Method::HistoryDelete, json!({ "url": url }))
                .await?;
            println!("{}", output::short_summary(&value, &["deleted"]));
        }
        Cmd::Add { url, title } => {
            let mut params = json!({ "url": url });
            if let Some(t) = title {
                params["title"] = json!(t);
            }
            let value = client.call_raw(Method::HistoryAdd, params).await?;
            println!("{}", output::short_summary(&value, &["added"]));
        }
        Cmd::Visits { url } => {
            let value = client
                .call_raw(Method::HistoryGetVisits, json!({ "url": url }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::visits_table(&value);
            }
        }
    }
    Ok(())
}
