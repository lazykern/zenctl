//! `zenctl bookmarks` — bookmark CRUD operations.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl bookmarks list\n  zenctl bookmarks list --folder toolbar_____\n  zenctl bookmarks create \"My Site\" --url https://example.com\n  zenctl bookmarks create \"Folder\"    # no url = folder\n  zenctl bookmarks update <id> --title \"New Name\"\n  zenctl bookmarks remove <id>\n  zenctl bookmarks remove <id> --recursive"
)]
pub enum Cmd {
    /// List bookmarks (tree view).
    List {
        #[arg(long)]
        folder: Option<String>,
    },
    /// Create a bookmark or folder.
    Create {
        title: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        parent_id: Option<String>,
    },
    /// Update a bookmark's title or url.
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        url: Option<String>,
    },
    /// Remove a bookmark (or folder when --recursive).
    Remove {
        id: String,
        #[arg(long)]
        recursive: bool,
    },
    /// Move a bookmark to a new parent and/or index.
    Move {
        id: String,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        index: Option<i64>,
    },
    /// Search bookmarks by text.
    Search { query: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List { folder } => {
            let params = match folder {
                Some(id) => json!({ "folder_id": id }),
                None => json!({}),
            };
            let value = client
                .call_raw(zenctl_protocol::Method::BookmarksList, params)
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::bookmarks_tree(&value, 0);
            }
        }
        Cmd::Create {
            parent_id,
            title,
            url,
        } => {
            let mut params = json!({ "title": title });
            if let Some(p) = parent_id {
                params["parent_id"] = json!(p);
            }
            if let Some(u) = url {
                params["url"] = json!(u);
            }
            let value = client
                .call_raw(zenctl_protocol::Method::BookmarksCreate, params)
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "title", "url"]));
            }
        }
        Cmd::Update { id, title, url } => {
            let mut params = json!({ "id": id });
            if let Some(t) = title {
                params["title"] = json!(t);
            }
            if let Some(u) = url {
                params["url"] = json!(u);
            }
            let value = client
                .call_raw(zenctl_protocol::Method::BookmarksUpdate, params)
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "title", "url"]));
            }
        }
        Cmd::Remove { id, recursive } => {
            confirm(opts, "remove bookmark", id)?;
            let value = client
                .call_raw(
                    zenctl_protocol::Method::BookmarksRemove,
                    json!({ "id": id, "recursive": recursive }),
                )
                .await?;
            println!("{}", output::short_summary(&value, &["removed"]));
        }
        Cmd::Move {
            id,
            parent_id,
            index,
        } => {
            let mut params = json!({ "id": id });
            if let Some(p) = parent_id {
                params["parent_id"] = json!(p);
            }
            if let Some(i) = index {
                params["index"] = json!(i);
            }
            let value = client
                .call_raw(zenctl_protocol::Method::BookmarksMove, params)
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::short_summary(&value, &["id", "title", "url"]));
            }
        }
        Cmd::Search { query } => {
            let value = client
                .call_raw(
                    zenctl_protocol::Method::BookmarksSearch,
                    json!({ "query": query }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::bookmarks_tree(&value, 0);
            }
        }
    }
    Ok(())
}
