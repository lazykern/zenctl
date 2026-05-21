//! `zenctl search` — search engine operations.

use super::CliOpts;
use crate::{
    client::Client,
    commands::target::{page_target, TargetArgs},
    output,
};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl search engines\n  zenctl search query \"rust async\"\n  zenctl search query \"weather\" --engine DuckDuckGo"
)]
pub enum Cmd {
    /// List installed search engines.
    Engines,
    /// Run a search query (loads results in a tab).
    Query {
        query: String,
        /// Search engine name (defaults to the browser default).
        #[arg(long)]
        engine: Option<String>,
        #[command(flatten)]
        target: TargetArgs,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Engines => {
            let value = client.call_raw(Method::SearchList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::search_engines_table(&value);
            }
        }
        Cmd::Query {
            query,
            engine,
            target,
        } => {
            let mut params = page_target(target);
            params["query"] = json!(query);
            if let Some(e) = engine {
                params["engine"] = json!(e);
            }
            let value = client.call_raw(Method::SearchQuery, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["query", "engine"]));
            }
        }
    }
    Ok(())
}
