//! `zenctl downloads` — download operations.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// List active and recent downloads.
    List {
        #[arg(long)]
        query: Option<String>,
    },
    /// Cancel a download by id.
    Cancel { download_id: i64 },
    /// Start a download from a URL.
    Start {
        url: String,
        /// Suggested filename for the download.
        #[arg(long)]
        filename: Option<String>,
        /// Prompt for a save location.
        #[arg(long)]
        save_as: bool,
    },
    /// Pause a download by id.
    Pause { download_id: i64 },
    /// Resume a paused download by id.
    Resume { download_id: i64 },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List { query } => {
            let params = match query {
                Some(q) => json!({ "query": q }),
                None => json!({}),
            };
            let value = client.call_raw(Method::DownloadsList, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::downloads_table(&value);
            }
        }
        Cmd::Cancel { download_id } => {
            let value = client
                .call_raw(
                    Method::DownloadsCancel,
                    json!({ "download_id": download_id }),
                )
                .await?;
            println!("{}", output::short_summary(&value, &["cancelled"]));
        }
        Cmd::Start {
            url,
            filename,
            save_as,
        } => {
            let mut params = json!({ "url": url });
            if let Some(f) = filename {
                params["filename"] = json!(f);
            }
            if *save_as {
                params["save_as"] = json!(true);
            }
            let value = client.call_raw(Method::DownloadsStart, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["download_id", "url"]));
            }
        }
        Cmd::Pause { download_id } => {
            let value = client
                .call_raw(
                    Method::DownloadsPause,
                    json!({ "download_id": download_id }),
                )
                .await?;
            println!("{}", output::short_summary(&value, &["paused"]));
        }
        Cmd::Resume { download_id } => {
            let value = client
                .call_raw(
                    Method::DownloadsResume,
                    json!({ "download_id": download_id }),
                )
                .await?;
            println!("{}", output::short_summary(&value, &["resumed"]));
        }
    }
    Ok(())
}
