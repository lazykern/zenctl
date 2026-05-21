//! `zenctl workspace` — Zen workspace operations.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl workspace list\n  zenctl workspace switch <uuid>\n  zenctl workspace create --name Research --icon \u{1f50d}\n  zenctl workspace rename <uuid> \"New Name\"\n  zenctl workspace set-icon <uuid> \u{1f680}\n  zenctl workspace set-container <uuid> <cookie-store-id>\n  zenctl workspace reorder <uuid> 0\n  zenctl workspace move-tab <uuid> --tab-id 42\n  zenctl workspace move-tab <uuid> --url https://example.com\n  zenctl workspace unload <uuid>\n  zenctl workspace unload-all --except <uuid>\n  zenctl workspace remove <uuid>"
)]
pub enum Cmd {
    /// Switch to a workspace by UUID.
    Switch { uuid: String },
    /// List all workspaces.
    List,
    /// Create a new workspace.
    Create {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        icon: Option<String>,
    },
    /// Remove a workspace by UUID.
    Remove { uuid: String },
    /// Rename a workspace.
    Rename { uuid: String, name: String },
    /// Set a workspace's icon.
    SetIcon { uuid: String, icon: String },
    /// Bind a workspace to a Firefox container.
    SetContainer {
        uuid: String,
        cookie_store_id: String,
    },
    /// Move a workspace to a new position (0-based index).
    Reorder { uuid: String, index: i64 },
    /// Move existing tabs into a workspace (by tab id or by url).
    /// Tabs must live in the active window. zen-essential tabs are skipped.
    MoveTab {
        uuid: String,
        /// Tab id to move. Pass multiple times for several tabs.
        #[arg(long = "tab-id")]
        tab_ids: Vec<i64>,
        /// URL to match. Pass multiple times for several tabs.
        #[arg(long)]
        url: Vec<String>,
    },
    /// Unload all unloadable tabs in a workspace. Defaults to active workspace.
    Unload { uuid: Option<String> },
    /// Unload all unloadable tabs outside a workspace. Defaults to active workspace.
    UnloadAll {
        /// Workspace to keep loaded. Defaults to active workspace.
        #[arg(long = "except")]
        except_uuid: Option<String>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Switch { uuid } => {
            let r = client
                .call_raw(Method::WorkspaceSwitch, json!({ "uuid": uuid }))
                .await?;
            println!("{}", output::short_summary(&r, &["active"]));
        }
        Cmd::List => {
            let r = client.call_raw(Method::WorkspaceList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                output::workspaces_table(&r);
            }
        }
        Cmd::Create { name, icon } => {
            let mut params = json!({});
            if let Some(n) = name {
                params["name"] = json!(n);
            }
            if let Some(i) = icon {
                params["icon"] = json!(i);
            }
            let r = client.call_raw(Method::WorkspaceCreate, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&r, &["uuid", "name", "created"])
                );
            }
        }
        Cmd::Remove { uuid } => {
            confirm(opts, "remove workspace", uuid)?;
            let r = client
                .call_raw(Method::WorkspaceRemove, json!({ "uuid": uuid }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["removed"]));
            }
        }
        Cmd::Rename { uuid, name } => {
            let r = client
                .call_raw(
                    Method::WorkspaceRename,
                    json!({ "uuid": uuid, "name": name }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["uuid", "name"]));
            }
        }
        Cmd::SetIcon { uuid, icon } => {
            let r = client
                .call_raw(
                    Method::WorkspaceSetIcon,
                    json!({ "uuid": uuid, "icon": icon }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["uuid", "icon"]));
            }
        }
        Cmd::SetContainer {
            uuid,
            cookie_store_id,
        } => {
            let r = client
                .call_raw(
                    Method::WorkspaceSetContainer,
                    json!({ "uuid": uuid, "cookie_store_id": cookie_store_id }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["uuid", "containerTabId"]));
            }
        }
        Cmd::Reorder { uuid, index } => {
            let r = client
                .call_raw(
                    Method::WorkspaceReorder,
                    json!({ "uuid": uuid, "index": index }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("{}", output::short_summary(&r, &["uuid", "index"]));
            }
        }
        Cmd::MoveTab { uuid, tab_ids, url } => {
            if tab_ids.is_empty() && url.is_empty() {
                anyhow::bail!("provide --tab-id and/or --url at least once");
            }
            let r = client
                .call_raw(
                    Method::WorkspaceMoveTab,
                    json!({ "uuid": uuid, "tab_ids": tab_ids, "urls": url }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&r, &["workspace_id", "moved", "skipped"])
                );
            }
        }
        Cmd::Unload { uuid } => {
            let r = client
                .call_raw(Method::WorkspaceUnload, json!({ "uuid": uuid }))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&r, &["workspace_id", "unloaded"])
                );
            }
        }
        Cmd::UnloadAll { except_uuid } => {
            let r = client
                .call_raw(
                    Method::WorkspaceUnloadAll,
                    json!({ "except_uuid": except_uuid }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&r, &["workspace_id", "unloaded"])
                );
            }
        }
    }
    Ok(())
}
