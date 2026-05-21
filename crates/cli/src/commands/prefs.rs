//! `zenctl prefs` — read/write Zen/Gecko preferences.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl prefs get zen.view.compact\n  zenctl prefs set zen.view.compact true\n  zenctl prefs set zen.urlbar.placeholder \"Search...\"\n  zenctl prefs clear zen.view.compact\n  zenctl prefs list\n  zenctl prefs list --prefix browser.tabs."
)]
pub enum Cmd {
    /// Read a single preference.
    Get { name: String },
    /// Set a preference value (bool, int, or string).
    Set { name: String, value: String },
    /// Clear a preference (reset to default).
    Clear { name: String },
    /// List preferences matching a prefix.
    List {
        #[arg(long, default_value = "zen.")]
        prefix: String,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Get { name } => {
            let value = client
                .call_raw(Method::PrefsGet, json!({ "name": name }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::pref(name, &value);
            }
        }
        Cmd::Set { name, value } => {
            let parsed: Value = serde_json::from_str(value).unwrap_or(Value::String(value.clone()));
            let r = client
                .call_raw(Method::PrefsSet, json!({ "name": name, "value": parsed }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                output::pref(name, &r);
            }
        }
        Cmd::Clear { name } => {
            let r = client
                .call_raw(Method::PrefsClear, json!({ "name": name }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                output::pref(name, &r);
                println!("cleared");
            }
        }
        Cmd::List { prefix } => {
            let value = client
                .call_raw(Method::PrefsList, json!({ "prefix": prefix }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::pref_list(&value);
            }
        }
    }
    Ok(())
}
