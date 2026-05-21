//! `zenctl find` — find text in a page.

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
    after_help = "EXAMPLES:\n  zenctl find text \"search term\"\n  zenctl find text hello --url-contains wikipedia --case-sensitive\n  zenctl find clear"
)]
pub enum Cmd {
    /// Find text in a page and highlight matches.
    Text {
        query: String,
        #[command(flatten)]
        target: TargetArgs,
        /// Match case exactly.
        #[arg(long)]
        case_sensitive: bool,
        /// Match whole words only.
        #[arg(long)]
        entire_word: bool,
    },
    /// Clear find highlights.
    Clear,
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Text {
            query,
            target,
            case_sensitive,
            entire_word,
        } => {
            let mut params = page_target(target);
            params["query"] = json!(query);
            params["case_sensitive"] = json!(case_sensitive);
            params["entire_word"] = json!(entire_word);
            let value = client.call_raw(Method::FindInPage, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["query", "count"]));
            }
        }
        Cmd::Clear => {
            let value = client.call_raw(Method::FindClear, json!({})).await?;
            println!("{}", output::short_summary(&value, &["cleared"]));
        }
    }
    Ok(())
}
