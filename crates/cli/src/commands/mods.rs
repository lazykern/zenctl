//! `zenctl mods` — Zen mod (theme) operations.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl mods list\n  zenctl mods install <mod-id>\n  zenctl mods install --url https://example.com/theme.json\n  zenctl mods enable <mod-id>\n  zenctl mods disable <mod-id>\n  zenctl mods remove <mod-id>\n  zenctl mods preferences <mod-id>"
)]
pub enum Cmd {
    /// List installed mods.
    List,
    /// Install a mod by its theme-store id, or from an arbitrary theme.json URL.
    Install {
        /// Mod UUID in the Zen theme store. Omit when using --url.
        mod_id: Option<String>,
        /// Direct theme.json URL (e.g. a fork or unlisted mod). The `id`
        /// field inside the JSON is used to register the mod.
        #[arg(long)]
        url: Option<String>,
    },
    /// Remove an installed mod.
    Remove { mod_id: String },
    /// Enable a mod.
    Enable { mod_id: String },
    /// Disable a mod.
    Disable { mod_id: String },
    /// Show a mod's preferences.
    Preferences { mod_id: String },
    /// Set a mod's preference value.
    SetPreference {
        mod_id: String,
        /// Preference property name (e.g. "mod.some.feature.enabled")
        pref_name: String,
        /// New value (boolean or string). Use "true" / "false" for checkboxes.
        pref_value: String,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let value = client.call_raw(Method::ModsList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::mods_table(&value);
            }
        }
        Cmd::Install { mod_id, url } => {
            if mod_id.is_none() && url.is_none() {
                anyhow::bail!("provide either a mod id (positional) or --url <theme.json>");
            }
            let mut params = serde_json::Map::new();
            if let Some(id) = mod_id {
                params.insert("mod_id".into(), json!(id));
            }
            if let Some(u) = url {
                params.insert("url".into(), json!(u));
            }
            let value = client
                .call_raw(Method::ModsInstall, serde_json::Value::Object(params))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["installed", "name"]));
            }
        }
        Cmd::Remove { mod_id } => {
            confirm(opts, "remove mod", mod_id)?;
            let value = client
                .call_raw(Method::ModsRemove, json!({ "mod_id": mod_id }))
                .await?;
            println!("{}", output::short_summary(&value, &["removed"]));
        }
        Cmd::Enable { mod_id } => {
            let value = client
                .call_raw(Method::ModsEnable, json!({ "mod_id": mod_id }))
                .await?;
            println!("{}", output::short_summary(&value, &["enabled"]));
        }
        Cmd::Disable { mod_id } => {
            let value = client
                .call_raw(Method::ModsDisable, json!({ "mod_id": mod_id }))
                .await?;
            println!("{}", output::short_summary(&value, &["disabled"]));
        }
        Cmd::Preferences { mod_id } => {
            let value = client
                .call_raw(Method::ModsPreferences, json!({ "mod_id": mod_id }))
                .await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Cmd::SetPreference {
            mod_id,
            pref_name,
            pref_value,
        } => {
            let value = client
                .call_raw(
                    Method::ModsSetPreference,
                    json!({
                        "mod_id": mod_id,
                        "pref_name": pref_name,
                        "pref_value": pref_value
                    }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "Set {} = {} for {}: {}",
                    pref_name,
                    pref_value,
                    mod_id,
                    output::short_summary(&value, &["ok"])
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_helpers;
    use tokio::net::UnixStream;
    use zenctl_protocol::Method;

    #[tokio::test]
    async fn set_preference_sends_correct_params() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::SetPreference {
            mod_id: "my-mod".into(),
            pref_name: "my.pref.name".into(),
            pref_value: "true".into(),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::ModsSetPreference);
        assert_eq!(
            params.get("mod_id").and_then(|v| v.as_str()),
            Some("my-mod")
        );
        assert_eq!(
            params.get("pref_name").and_then(|v| v.as_str()),
            Some("my.pref.name")
        );
        assert_eq!(
            params.get("pref_value").and_then(|v| v.as_str()),
            Some("true")
        );

        test_helpers::write_response(&mut server_side, serde_json::json!({"ok": true})).await;
        handle.await.unwrap();
    }
}
