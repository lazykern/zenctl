//! `zenctl snapshot` — one JSON state dump for automation agents.

use anyhow::Result;
use serde_json::{json, Value};
use zenctl_protocol::Method;

use super::CliOpts;
use crate::client::Client;

pub async fn run(client: &mut Client, _opts: &CliOpts) -> Result<()> {
    let mut out = json!({
        "schema": "zenctl.snapshot.v1",
        "ok": true,
        "errors": []
    });

    call_into(client, &mut out, "status", Method::Status, Value::Null).await;
    call_into(client, &mut out, "windows", Method::WindowsList, json!({})).await;
    call_into(client, &mut out, "tabs", Method::TabsList, json!({})).await;
    call_into(
        client,
        &mut out,
        "workspaces",
        Method::WorkspaceList,
        json!({}),
    )
    .await;
    call_into(client, &mut out, "splits", Method::SplitViewList, json!({})).await;
    call_into(client, &mut out, "glances", Method::GlanceList, json!({})).await;
    call_into(client, &mut out, "folders", Method::FoldersList, json!({})).await;
    call_into(
        client,
        &mut out,
        "live_folders",
        Method::LiveFoldersList,
        json!({}),
    )
    .await;

    if out["errors"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false)
    {
        out["ok"] = json!(false);
    }

    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

async fn call_into(client: &mut Client, out: &mut Value, key: &str, method: Method, params: Value) {
    match client.call_raw(method, params).await {
        Ok(v) => out[key] = v,
        Err(e) => {
            out[key] = Value::Null;
            if let Some(errors) = out["errors"].as_array_mut() {
                errors.push(json!({ "section": key, "error": e.to_string() }));
            }
        }
    }
}
