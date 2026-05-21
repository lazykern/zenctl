//! `zenctl folders` — Zen pinned-tab folder operations.

use super::{confirm, CliOpts};
use crate::client::Client;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(after_help = "EXAMPLES:\n  \
    zenctl folders list\n  \
    zenctl folders create --label Research\n  \
    zenctl folders add-tab <id> --tab-id 42 --tab-id 43\n  \
    zenctl folders add-tab <id> --url https://example.com\n  \
    zenctl folders set-icon <id> 🧪\n  \
    zenctl folders subfolder <parent-id> --label Subtopic\n  \
    zenctl folders rename <id> 'Project X'\n  \
    zenctl folders collapse <id>\n  \
    zenctl folders expand <id>\n  \
    zenctl folders unpack <id>\n  \
    zenctl folders unload <id>\n  \
    zenctl folders move-to-workspace <id> <workspace-uuid>\n  \
    zenctl folders convert-to-workspace <id> --force\n  \
    zenctl folders delete <id> --force")]
pub enum Cmd {
    /// List all pinned-tab folders across all windows.
    List,
    /// Create an empty folder.
    Create {
        /// Folder label.
        #[arg(long)]
        label: Option<String>,
        /// Workspace UUID. Omit to use the active workspace.
        #[arg(long = "workspace")]
        workspace_id: Option<String>,
    },
    /// Add existing tabs into a folder (by tab id or by url).
    AddTab {
        folder_id: String,
        /// Tab id to add. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
    },
    /// Set a folder's icon.
    /// Accepts: an emoji (🧪), a bare built-in icon name (flask.svg, bookmark.svg,
    /// star.svg, …), a chrome:// / http(s):// / data: URL, or "" to clear.
    SetIcon { folder_id: String, icon: String },
    /// Create a subfolder under an existing folder.
    Subfolder {
        parent_id: String,
        /// Subfolder label.
        #[arg(long)]
        label: Option<String>,
    },
    /// Delete a folder and its tabs.
    Delete { folder_id: String },
    /// Rename a folder by id.
    Rename { folder_id: String, name: String },
    /// Collapse a folder.
    Collapse { folder_id: String },
    /// Expand a folder.
    Expand { folder_id: String },
    /// Ungroup the folder, keeping its tabs as loose pinned tabs.
    Unpack { folder_id: String },
    /// Unload (discard) every tab in the folder to free memory.
    Unload { folder_id: String },
    /// Move the folder into another workspace.
    MoveToWorkspace {
        folder_id: String,
        workspace_id: String,
    },
    /// Convert the folder into a new workspace and delete the folder shell.
    ConvertToWorkspace { folder_id: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let value = client.call_raw(Method::FoldersList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                print_folders(&value);
            }
        }
        Cmd::Create {
            label,
            workspace_id,
        } => {
            let mut params = serde_json::Map::new();
            if let Some(l) = label {
                params.insert("label".into(), json!(l));
            }
            if let Some(ws) = workspace_id {
                params.insert("workspace_id".into(), json!(ws));
            }
            let value = client
                .call_raw(Method::FoldersCreate, serde_json::Value::Object(params))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let lbl = value.get("label").and_then(|v| v.as_str()).unwrap_or("?");
                println!("folder created: {id} ({lbl})");
            }
        }
        Cmd::Delete { folder_id } => {
            confirm(opts, "delete folder", folder_id)?;
            let value = client
                .call_raw(Method::FoldersDelete, json!({ "folder_id": folder_id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder deleted: {folder_id}");
            }
        }
        Cmd::Rename { folder_id, name } => {
            let value = client
                .call_raw(
                    Method::FoldersRename,
                    json!({ "folder_id": folder_id, "name": name }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder renamed: {folder_id} → {name}");
            }
        }
        Cmd::Collapse { folder_id } => {
            let value = client
                .call_raw(
                    Method::FoldersCollapse,
                    json!({ "folder_id": folder_id, "collapsed": true }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder collapsed: {folder_id}");
            }
        }
        Cmd::Expand { folder_id } => {
            let value = client
                .call_raw(
                    Method::FoldersCollapse,
                    json!({ "folder_id": folder_id, "collapsed": false }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder expanded: {folder_id}");
            }
        }
        Cmd::AddTab {
            folder_id,
            tab_ids,
            url,
        } => {
            if tab_ids.is_empty() && url.is_empty() {
                anyhow::bail!("provide --tab-id and/or --url at least once");
            }
            let value = client
                .call_raw(
                    Method::FoldersAddTab,
                    json!({ "folder_id": folder_id, "tab_ids": tab_ids, "urls": url }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let added = value.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("folder {folder_id}: added {added} tab(s)");
            }
        }
        Cmd::SetIcon { folder_id, icon } => {
            let value = client
                .call_raw(
                    Method::FoldersSetIcon,
                    json!({ "folder_id": folder_id, "icon": icon }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder {folder_id}: icon set");
            }
        }
        Cmd::Subfolder { parent_id, label } => {
            let mut params = serde_json::Map::new();
            params.insert("parent_id".into(), json!(parent_id));
            if let Some(l) = label {
                params.insert("label".into(), json!(l));
            }
            let value = client
                .call_raw(
                    Method::FoldersCreateSubfolder,
                    serde_json::Value::Object(params),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let lbl = value.get("label").and_then(|v| v.as_str()).unwrap_or("?");
                println!("subfolder created under {parent_id}: {id} ({lbl})");
            }
        }
        Cmd::Unpack { folder_id } => {
            confirm(opts, "unpack folder", folder_id)?;
            let value = client
                .call_raw(Method::FoldersUnpack, json!({ "folder_id": folder_id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder unpacked: {folder_id}");
            }
        }
        Cmd::Unload { folder_id } => {
            let value = client
                .call_raw(Method::FoldersUnload, json!({ "folder_id": folder_id }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder tabs unloaded: {folder_id}");
            }
        }
        Cmd::MoveToWorkspace {
            folder_id,
            workspace_id,
        } => {
            let value = client
                .call_raw(
                    Method::FoldersMoveToWorkspace,
                    json!({ "folder_id": folder_id, "workspace_id": workspace_id }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("folder {folder_id} moved to workspace {workspace_id}");
            }
        }
        Cmd::ConvertToWorkspace { folder_id } => {
            confirm(opts, "convert folder to workspace", folder_id)?;
            let value = client
                .call_raw(
                    Method::FoldersConvertToWorkspace,
                    json!({ "folder_id": folder_id }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                let uuid = value
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                println!("folder {folder_id} converted to workspace {uuid} ({name})");
            }
        }
    }
    Ok(())
}

fn print_folders(value: &serde_json::Value) {
    let folders = value.get("folders").and_then(|v| v.as_array());
    let count = folders.map(|f| f.len()).unwrap_or(0);
    if count == 0 {
        println!("no folders");
        return;
    }
    println!("{count} folder(s):");
    for f in folders.unwrap() {
        let id = f.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let label = f.get("label").and_then(|v| v.as_str()).unwrap_or("?");
        let collapsed = f
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let parent = f.get("parent_id").and_then(|v| v.as_str()).unwrap_or("");
        let live = f
            .get("is_live_folder")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let tabs = f.get("tabs").and_then(|v| v.as_array());
        let marker = if collapsed { "▸" } else { "▾" };
        let extra = match (parent.is_empty(), live) {
            (false, _) => format!(" (in {parent})"),
            (_, true) => " (live)".into(),
            _ => String::new(),
        };
        println!(
            "  {marker} {id} — {label}  [{} tab(s)]{extra}",
            tabs.map(|t| t.len()).unwrap_or(0)
        );
        if let Some(tabs) = tabs {
            for t in tabs {
                let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let url = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
                println!("      - {title}  {url}");
            }
        }
    }
}
