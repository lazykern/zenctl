//! `zenctl share` — open Zen's native share dialog (Windows / macOS only).

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use serde_json::json;
use zenctl_protocol::Method;

pub async fn run(
    client: &mut Client,
    opts: &CliOpts,
    url: Option<&str>,
    title: Option<&str>,
    text: Option<&str>,
    check: bool,
) -> Result<()> {
    if check {
        let r = client.call_raw(Method::ShareCan, json!({})).await?;
        if opts.json {
            println!("{}", serde_json::to_string_pretty(&r)?);
        } else {
            let can = r.get("can").and_then(|v| v.as_bool()).unwrap_or(false);
            println!(
                "native share: {}",
                if can { "supported" } else { "unsupported" }
            );
            if let Some(reason) = r.get("reason").and_then(|v| v.as_str()) {
                println!("reason: {reason}");
            }
        }
        return Ok(());
    }

    let mut params = serde_json::Map::new();
    if let Some(u) = url {
        params.insert("url".into(), json!(u));
    }
    if let Some(t) = title {
        params.insert("title".into(), json!(t));
    }
    if let Some(t) = text {
        params.insert("text".into(), json!(t));
    }
    let r = client
        .call_raw(Method::Share, serde_json::Value::Object(params))
        .await?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        let shared = r.get("url").and_then(|v| v.as_str()).unwrap_or("");
        println!("share: {shared}");
    }
    Ok(())
}
