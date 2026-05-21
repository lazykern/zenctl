//! `zenctl cookies` — cookie operations.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Get a cookie by URL and name.
    Get { url: String, name: String },
    /// Set a cookie value.
    Set {
        url: String,
        name: String,
        value: String,
    },
    /// Remove a cookie.
    Remove { url: String, name: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Get { url, name } => {
            let value = client
                .call_raw(Method::CookiesGet, json!({ "url": url, "name": name }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&value, &["name", "value", "domain"])
                );
            }
        }
        Cmd::Set {
            url,
            name,
            value: cookie_value,
        } => {
            let value = client
                .call_raw(
                    Method::CookiesSet,
                    json!({ "url": url, "name": name, "value": cookie_value }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&value, &["name", "value", "domain"])
                );
            }
        }
        Cmd::Remove { url, name } => {
            let value = client
                .call_raw(Method::CookiesRemove, json!({ "url": url, "name": name }))
                .await?;
            println!("{}", output::short_summary(&value, &["name", "url"]));
        }
    }
    Ok(())
}
