//! `zenctl wait` — polling primitives for automation scripts.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use clap::Subcommand;
use serde_json::{json, Value};
use tokio::time::sleep;
use zenctl_protocol::Method;

use super::{
    target::{page_target, TargetArgs},
    CliOpts,
};
use crate::client::Client;

#[derive(Subcommand)]
pub enum Cmd {
    /// Wait until a targeted tab reports status=complete.
    TabLoaded {
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, default_value_t = 250)]
        interval_ms: u64,
    },
    /// Wait until a targeted tab's URL contains text.
    UrlContains {
        text: String,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, default_value_t = 250)]
        interval_ms: u64,
    },
    /// Wait until a targeted tab's title contains text.
    TitleContains {
        text: String,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, default_value_t = 250)]
        interval_ms: u64,
    },
    /// Wait until visible page text contains text.
    Text {
        text: String,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, default_value_t = 500)]
        interval_ms: u64,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::TabLoaded {
            target,
            timeout,
            interval_ms,
        } => {
            wait_tab(
                client,
                opts,
                target,
                *timeout,
                *interval_ms,
                |tab| tab.get("status").and_then(Value::as_str) == Some("complete"),
                "tab status=complete",
            )
            .await
        }
        Cmd::UrlContains {
            text,
            target,
            timeout,
            interval_ms,
        } => {
            wait_tab(
                client,
                opts,
                target,
                *timeout,
                *interval_ms,
                |tab| {
                    tab.get("url")
                        .and_then(Value::as_str)
                        .map(|s| s.contains(text))
                        .unwrap_or(false)
                },
                &format!("url contains {text:?}"),
            )
            .await
        }
        Cmd::TitleContains {
            text,
            target,
            timeout,
            interval_ms,
        } => {
            wait_tab(
                client,
                opts,
                target,
                *timeout,
                *interval_ms,
                |tab| {
                    tab.get("title")
                        .and_then(Value::as_str)
                        .map(|s| s.contains(text))
                        .unwrap_or(false)
                },
                &format!("title contains {text:?}"),
            )
            .await
        }
        Cmd::Text {
            text,
            target,
            timeout,
            interval_ms,
        } => wait_page_text(client, opts, target, text, *timeout, *interval_ms).await,
    }
}

async fn wait_tab<F>(
    client: &mut Client,
    opts: &CliOpts,
    target: &TargetArgs,
    timeout_secs: u64,
    interval_ms: u64,
    mut pred: F,
    label: &str,
) -> Result<()>
where
    F: FnMut(&Value) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut last = Value::Null;
    loop {
        let found = client
            .call_raw(Method::TabsFind, page_target(target))
            .await?;
        let tab = found
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        if !tab.is_null() && pred(&tab) {
            return print_ok(opts, label, tab);
        }
        last = tab;
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for {label}; last={}",
                compact(&last)
            ));
        }
        sleep(Duration::from_millis(interval_ms)).await;
    }
}

async fn wait_page_text(
    client: &mut Client,
    opts: &CliOpts,
    target: &TargetArgs,
    text: &str,
    timeout_secs: u64,
    interval_ms: u64,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut last_len = 0usize;
    loop {
        let value = client
            .call_raw(Method::PageText, page_target(target))
            .await?;
        let body = value.get("text").and_then(Value::as_str).unwrap_or("");
        last_len = body.len();
        if body.contains(text) {
            return print_ok(
                opts,
                &format!("text contains {text:?}"),
                json!({
                    "matched": true,
                    "text": text,
                    "length": body.len(),
                    "url": value.get("url").cloned().unwrap_or(Value::Null),
                    "title": value.get("title").cloned().unwrap_or(Value::Null),
                }),
            );
        }
        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for text {text:?}; last_text_length={last_len}"
            ));
        }
        sleep(Duration::from_millis(interval_ms)).await;
    }
}

fn print_ok(opts: &CliOpts, label: &str, value: Value) -> Result<()> {
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "waited_for": label,
                "value": value,
            }))?
        );
    } else {
        println!("ok: {label}");
    }
    Ok(())
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}
