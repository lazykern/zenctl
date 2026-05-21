//! `zenctl data` — clear browsing data.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Map, Value};
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl data clear --cache --force\n  zenctl data clear --cookies --history --since 2h --force\n  zenctl data clear --all --since 7d --force\n  zenctl data clear --cache --dry-run"
)]
pub enum Cmd {
    /// Clear browsing data. Destructive — requires --force.
    Clear {
        /// Clear the disk and memory cache.
        #[arg(long)]
        cache: bool,
        /// Clear cookies.
        #[arg(long)]
        cookies: bool,
        /// Clear browsing history.
        #[arg(long)]
        history: bool,
        /// Clear the download history (not the files).
        #[arg(long)]
        downloads: bool,
        /// Clear saved form data.
        #[arg(long)]
        form_data: bool,
        /// Clear all of the data types above.
        #[arg(long)]
        all: bool,
        /// Only clear data newer than this (e.g. 30m, 2h, 7d). Default: everything.
        #[arg(long)]
        since: Option<String>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Clear {
            cache,
            cookies,
            history,
            downloads,
            form_data,
            all,
            since,
        } => {
            let mut types = Map::new();
            for (selected, key) in [
                (*cache, "cache"),
                (*cookies, "cookies"),
                (*history, "history"),
                (*downloads, "downloads"),
                (*form_data, "formData"),
            ] {
                if *all || selected {
                    types.insert(key.to_string(), json!(true));
                }
            }
            if types.is_empty() {
                anyhow::bail!(
                    "select at least one data type (--cache, --cookies, --history, \
                     --downloads, --form-data) or --all"
                );
            }

            let since_ms = match since {
                Some(s) => now_ms().saturating_sub(parse_duration_ms(s)?),
                None => 0,
            };

            let label = types.keys().cloned().collect::<Vec<_>>().join(", ");
            let scope = match since {
                Some(s) => format!("{label} (last {s})"),
                None => format!("{label} (all time)"),
            };
            confirm(opts, "clear browsing data", &scope)?;
            if opts.dry_run {
                return Ok(());
            }

            let value = client
                .call_raw(
                    Method::DataClear,
                    json!({ "since": since_ms, "types": Value::Object(types) }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["cleared", "since"]));
            }
        }
    }
    Ok(())
}

/// Current Unix time in milliseconds.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Parse a duration like `30m`, `2h`, `7d`, `45s`, or a bare millisecond count.
fn parse_duration_ms(s: &str) -> Result<u64> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration: {s}"))?;
    let mult = match unit {
        "" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        other => anyhow::bail!("unknown duration unit '{other}' (use s/m/h/d)"),
    };
    Ok(n.saturating_mul(mult))
}

#[cfg(test)]
mod tests {
    use super::parse_duration_ms;

    #[test]
    fn parses_units() {
        assert_eq!(parse_duration_ms("45s").unwrap(), 45_000);
        assert_eq!(parse_duration_ms("30m").unwrap(), 1_800_000);
        assert_eq!(parse_duration_ms("2h").unwrap(), 7_200_000);
        assert_eq!(parse_duration_ms("7d").unwrap(), 604_800_000);
        assert_eq!(parse_duration_ms("500").unwrap(), 500);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_duration_ms("abc").is_err());
        assert!(parse_duration_ms("10y").is_err());
    }
}
