//! `zenctl boosts` — Zen Boost operations.

use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

use super::CliOpts;
use crate::{client::Client, output};

#[derive(Subcommand)]
pub enum Cmd {
    /// List registered Boost domains and entries.
    List,
    /// Create a Boost for a domain (example: example.com).
    Create { domain: String },
    /// Delete a Boost by domain and id.
    Delete { domain: String, id: String },
    /// Make a Boost active for its domain.
    Activate { domain: String, id: String },
    /// Toggle a Boost active/inactive for its domain.
    Toggle { domain: String, id: String },
    /// Update a Boost's visual data (CSS, colors, etc). Pass partial boostData as JSON.
    Update {
        domain: String,
        id: String,
        /// JSON with boostData fields to update
        /// (e.g. '{"customCSS": "body { background: red }", "brightness": 0.8}')
        #[arg(long)]
        data: String,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    let r = match cmd {
        Cmd::List => client.call_raw(Method::BoostsList, json!({})).await?,
        Cmd::Create { domain } => {
            client
                .call_raw(Method::BoostsCreate, json!({ "domain": domain }))
                .await?
        }
        Cmd::Delete { domain, id } => {
            client
                .call_raw(Method::BoostsDelete, json!({ "domain": domain, "id": id }))
                .await?
        }
        Cmd::Activate { domain, id } => {
            client
                .call_raw(
                    Method::BoostsActivate,
                    json!({ "domain": domain, "id": id }),
                )
                .await?
        }
        Cmd::Toggle { domain, id } => {
            client
                .call_raw(Method::BoostsToggle, json!({ "domain": domain, "id": id }))
                .await?
        }
        Cmd::Update { domain, id, data } => {
            client
                .call_raw(
                    Method::BoostsUpdate,
                    json!({ "domain": domain, "id": id, "data_json": data }),
                )
                .await?
        }
    };
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        println!(
            "{}",
            output::short_summary(
                &r,
                &["count", "created", "deleted", "active", "toggled", "domain", "id", "updated"]
            )
        );
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
    async fn update_sends_correct_params() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Update {
            domain: "example.com".into(),
            id: "abc123".into(),
            data: r#"{"customCSS": "body { color: red }", "brightness": 0.8}"#.into(),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::BoostsUpdate);
        assert_eq!(
            params.get("domain").and_then(|v| v.as_str()),
            Some("example.com")
        );
        assert_eq!(params.get("id").and_then(|v| v.as_str()), Some("abc123"));
        assert!(params
            .get("data_json")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("customCSS"));

        test_helpers::write_response(&mut server_side, serde_json::json!({"updated": true})).await;
        handle.await.unwrap();
    }
}
