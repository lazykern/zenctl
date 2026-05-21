//! Shared tab-targeting args used by Tabs, Page, and Media commands.

use clap::Args;
use serde_json::{json, Value};

#[derive(Args, Clone, Default)]
pub struct TargetArgs {
    /// Target a specific tab by its browser-assigned ID (from `zenctl tabs list`).
    #[arg(long)]
    pub tab_id: Option<i64>,
    /// Target the active tab in this window id.
    #[arg(long)]
    pub window_id: Option<i64>,
    /// Target tab at this index in the selected/current window.
    #[arg(long)]
    pub tab_index: Option<i64>,
    /// Target the first tab whose URL contains this string.
    #[arg(long)]
    pub url_contains: Option<String>,
    /// Target the first tab whose title contains this string.
    #[arg(long)]
    pub title_contains: Option<String>,
    /// Target the active tab. This is the default when no selector is given.
    #[arg(long)]
    pub active: bool,
    /// Filter by workspace name or UUID (cross-workspace tab discovery).
    #[arg(long)]
    pub workspace: Option<String>,
}

/// Convert `TargetArgs` into the `{"target": {...}}` params used by the protocol.
pub fn page_target(target: &TargetArgs) -> Value {
    let mut m = serde_json::Map::new();
    if let Some(ws) = &target.workspace {
        m.insert("workspace".into(), json!(ws));
    }
    json!({
        "target": {
            "tab_id": target.tab_id,
            "window_id": target.window_id,
            "tab_index": target.tab_index,
            "url_contains": target.url_contains,
            "title_contains": target.title_contains,
            "active": target.active,
        },
        "workspace": target.workspace,
    })
}
