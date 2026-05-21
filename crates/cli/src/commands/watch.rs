//! `zenctl watch` — stream live browser events.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;

/// Run the watch loop. `topics` filters event topics by prefix (e.g. `tabs`,
/// `windows`); `all` or an empty list streams everything.
pub async fn run(client: &mut Client, opts: &CliOpts, topics: &[String]) -> Result<()> {
    let filter: Vec<String> = topics
        .iter()
        .filter(|t| t.as_str() != "all")
        .cloned()
        .collect();

    client.start_watch(&filter).await?;

    if !opts.json {
        let what = if filter.is_empty() {
            "all events".to_string()
        } else {
            filter.join(", ")
        };
        eprintln!("watching {what} — press Ctrl-C to stop");
    }

    loop {
        match client.recv_event().await {
            Ok(ev) => {
                if opts.json {
                    let line = serde_json::json!({
                        "topic": ev.topic,
                        "payload": ev.payload,
                    });
                    println!("{}", serde_json::to_string(&line)?);
                } else {
                    println!("{}  {}", ev.topic, ev.payload);
                }
            }
            Err(e) => {
                if !opts.json {
                    eprintln!("watch ended: {e}");
                }
                break;
            }
        }
    }
    Ok(())
}
