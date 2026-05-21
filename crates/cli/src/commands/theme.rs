//! `zenctl theme` — sugar over `zenctl prefs` for Zen theme preferences.
//!
//! All theme settings are stored as `zen.theme.*` Gecko preferences.
//! This command wraps the existing `PrefsGet`/`PrefsSet`/`PrefsList`
//! protocol methods, accepting short key names and auto-prefixing them
//! with `zen.theme.`.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

/// Known theme preference short keys with types and descriptions.
const THEME_KEYS: &[(&str, &str, &str)] = &[
    (
        "accent-color",
        "color",
        "CSS color for the accent (e.g. #ff6600, blue)",
    ),
    (
        "border-radius",
        "number",
        "Corner radius for UI elements (-1 = platform native)",
    ),
    (
        "content-element-separation",
        "number",
        "Gap between sidebar elements (0–12)",
    ),
    (
        "dark-mode-bias",
        "float",
        "Bias generated workspace colors toward dark (0.0–1.0)",
    ),
    (
        "gradient.show-custom-colors",
        "bool",
        "Show custom workspace gradient colors",
    ),
    (
        "essentials-favicon-bg",
        "bool",
        "Colored background behind pinned essential favicons",
    ),
    (
        "acrylic-elements",
        "bool",
        "Acrylic/frosted-glass effects on sidebar elements",
    ),
    (
        "disable-lightweight",
        "bool",
        "Disable lightweight extension theme integration",
    ),
    (
        "use-system-colors",
        "bool",
        "Use system accent colors instead of custom",
    ),
    (
        "hide-tab-throbber",
        "bool",
        "Hide the loading spinner on tabs",
    ),
    (
        "styled-status-panel",
        "bool",
        "Style the status panel (always true on macOS)",
    ),
    (
        "hide-unified-extensions-button",
        "bool",
        "Hide the extensions puzzle-piece button in the toolbar",
    ),
];

fn full_name(key: &str) -> String {
    if key.starts_with("zen.theme.") {
        key.to_string()
    } else {
        format!("zen.theme.{key}")
    }
}

#[derive(Subcommand)]
#[command(after_help = "EXAMPLES:\n  \
    zenctl theme list\n  \
    zenctl theme get accent-color\n  \
    zenctl theme set accent-color '#ff6600'\n  \
    zenctl theme set border-radius 8\n  \
    zenctl theme set essentials-favicon-bg false\n  \
    \n  \
    Short keys (e.g. `accent-color`) are auto-prefixed to `zen.theme.accent-color`.\n  \
    Use the full dotted path (e.g. `zen.theme.accent-color`) if you prefer.")]
pub enum Cmd {
    /// List all known Zen theme preferences with their current values.
    List,
    /// Get the current value of a single theme preference.
    Get {
        /// Theme key (short name like `accent-color`, or full path `zen.theme.accent-color`).
        key: String,
    },
    /// Set a theme preference.
    Set {
        /// Theme key.
        key: String,
        /// New value (JSON literal: true, false, 8, 0.5, \"#ff6600\").
        value: String,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let value = client
                .call_raw(Method::PrefsList, json!({ "prefix": "zen.theme." }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                print_theme_list(&value);
            }
        }
        Cmd::Get { key } => {
            let name = full_name(key);
            let value = client
                .call_raw(Method::PrefsGet, json!({ "name": name }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                crate::output::pref(&name, &value);
            }
        }
        Cmd::Set { key, value } => {
            let name = full_name(key);
            let parsed: serde_json::Value =
                serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.clone()));
            let r = client
                .call_raw(Method::PrefsSet, json!({ "name": name, "value": parsed }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                crate::output::pref(&name, &r);
            }
        }
    }
    Ok(())
}

fn print_theme_list(raw: &serde_json::Value) {
    // Build a map of zen.theme.* pref name → value from the raw response.
    let entries: Vec<(String, serde_json::Value)> = raw
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let name = e.get("name")?.as_str()?;
            let value = e.get("value");
            Some((
                name.to_string(),
                value.cloned().unwrap_or(serde_json::Value::Null),
            ))
        })
        .collect();

    if entries.is_empty() {
        println!("no zen.theme.* preferences found");
        return;
    }

    fn short_name(full: &str) -> &str {
        full.strip_prefix("zen.theme.").unwrap_or(full)
    }
    let val_str = |v: &serde_json::Value| match v {
        serde_json::Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    };

    // Align columns
    let max_key = entries
        .iter()
        .map(|(n, _)| short_name(n).len())
        .max()
        .unwrap_or(8);
    let max_val = entries
        .iter()
        .map(|(_, v)| val_str(v).len())
        .max()
        .unwrap_or(6);

    println!(
        "{:<k$}  {:>v$}  type",
        "key",
        "value",
        k = max_key,
        v = max_val
    );
    println!("{:-<k$}  {:-<v$}  ----", "", "", k = max_key, v = max_val);

    for (name, value) in &entries {
        let key = short_name(name);
        let v = val_str(value);
        let type_hint = THEME_KEYS
            .iter()
            .find(|(k, _, _)| key == *k)
            .map(|(_, t, _)| *t)
            .unwrap_or("?");
        println!(
            "{:<k$}  {:>v$}  {type_hint}",
            key,
            v,
            k = max_key,
            v = max_val
        );
    }
}
