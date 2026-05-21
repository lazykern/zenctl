//! `zenctl compact` — toggle/set Zen's compact mode.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Toggle compact mode on/off.
    Toggle,
    /// Set compact mode explicitly.
    Set {
        /// true to enable, false to disable
        #[arg(action = clap::ArgAction::Set)]
        value: bool,
    },
    /// Hide part of the UI (compact mode must be on for this to show).
    Hide {
        /// What to hide.
        #[arg(value_parser = ["sidebar", "toolbar", "both"])]
        what: String,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Toggle => {
            let r = client.compact_toggle().await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                let state = match r.enabled {
                    Some(true) => "enabled",
                    Some(false) => "disabled",
                    None => "toggled",
                };
                println!("compact: {state}");
            }
        }
        Cmd::Set { value } => {
            let r = client
                .call_raw(Method::CompactSet, json!({ "value": value }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                let state = match r.get("enabled").and_then(Value::as_bool) {
                    Some(true) => "enabled",
                    Some(false) => "disabled",
                    None => "unknown",
                };
                println!("compact: {state}");
            }
        }
        Cmd::Hide { what } => {
            let r = client
                .call_raw(Method::CompactHide, json!({ "what": what }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("compact: hid {what}");
            }
        }
    }
    Ok(())
}
