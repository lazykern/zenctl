//! `zenctl shortcuts` — read/write keyboard shortcuts file.

use super::CliOpts;
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::{json, Value};
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Read the shortcuts file as JSON.
    Read,
    /// Write shortcuts JSON to the profile file.
    Write {
        /// JSON string (full shortcuts object).
        data: String,
    },
    /// Reset keyboard shortcuts to Zen browser defaults.
    Reset,
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Read => {
            let value = client.call_raw(Method::ShortcutsRead, json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Cmd::Write { data } => {
            let parsed: Value = serde_json::from_str(data)?;
            let r = client
                .call_raw(Method::ShortcutsWrite, json!({ "shortcuts": parsed }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                let path = r.get("written").and_then(Value::as_str).unwrap_or("?");
                println!("written: {path}");
            }
        }
        Cmd::Reset => {
            let r = client.call_raw(Method::ShortcutsReset, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("shortcuts: reset to Zen defaults");
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
    async fn reset() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Reset;

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::ShortcutsReset);
        assert_eq!(params, serde_json::json!({}));

        test_helpers::write_response(&mut server_side, serde_json::json!({"reset": true})).await;
        handle.await.unwrap();
    }
}
