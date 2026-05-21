//! Shared output formatters for zenctl CLI commands.
//!
//! All human-readable formatting lives here so a) it's testable and
//! b) commands don't need to inline println! logic.

use serde_json::Value;
use std::borrow::Cow;

fn print_table(headers: &[&str], rows: Vec<Vec<String>>) {
    let ncols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < ncols {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let fmt_row = |cells: Vec<&str>| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{}", fmt_row(headers.to_vec()));
    println!(
        "{}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        println!("{}", fmt_row(cells));
    }
}

pub fn short_summary(value: &Value, keys: &[&str]) -> String {
    let parts: Vec<_> = keys
        .iter()
        .filter_map(|k| value.get(*k).map(|v| format!("{k}={v}")))
        .collect();
    if parts.is_empty() {
        value.to_string()
    } else {
        parts.join("  ")
    }
}

pub fn bookmarks_tree(value: &Value, depth: usize) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    for node in arr {
        let pad = "  ".repeat(depth);
        let title = node.get("title").and_then(Value::as_str).unwrap_or("");
        match node.get("url").and_then(Value::as_str) {
            Some(u) => println!("{pad}- {title}  [{u}]"),
            None => println!("{pad}* {title}/"),
        }
        if let Some(children) = node.get("children") {
            bookmarks_tree(children, depth + 1);
        }
    }
}

pub fn tabs_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no tabs)");
        return;
    }
    let rows = arr
        .iter()
        .map(|tab| {
            let active = tab.get("active").and_then(Value::as_bool).unwrap_or(false);
            let id = tab
                .get("id")
                .and_then(Value::as_i64)
                .map(|i| i.to_string())
                .unwrap_or_else(|| "?".into());
            let win = tab
                .get("windowId")
                .and_then(Value::as_i64)
                .map(|i| i.to_string())
                .unwrap_or_else(|| "?".into());
            let title = tab
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let url = tab
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            // Workspace column: show workspace name if available, else UUID prefix.
            let ws = tab
                .get("workspace_name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| tab.get("workspace_id").and_then(Value::as_str))
                .unwrap_or("");
            let ws_display = if ws.len() > 10 {
                let (a, _) = ws.split_at(8);
                Cow::Owned(format!("{a}…"))
            } else {
                Cow::Borrowed(ws)
            };
            vec![
                if active { "*".into() } else { "".into() },
                id,
                win,
                ws_display.into_owned(),
                title,
                url,
            ]
        })
        .collect();
    print_table(&["", "ID", "Win", "Ws", "Title", "URL"], rows);
}

pub fn windows_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no windows)");
        return;
    }
    let rows = arr
        .iter()
        .map(|w| {
            let focused = w.get("focused").and_then(Value::as_bool).unwrap_or(false);
            let id = w
                .get("id")
                .and_then(Value::as_i64)
                .map(|i| i.to_string())
                .unwrap_or_else(|| "?".into());
            let state = w
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![if focused { "*".into() } else { "".into() }, id, state]
        })
        .collect();
    print_table(&["", "ID", "State"], rows);
}

pub fn downloads_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no downloads)");
        return;
    }
    let rows = arr
        .iter()
        .map(|d| {
            let id = d
                .get("id")
                .and_then(Value::as_i64)
                .map(|i| i.to_string())
                .unwrap_or_else(|| "?".into());
            let state = d
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let bytes = d
                .get("bytesReceived")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .to_string();
            let filename = d
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![id, state, bytes, filename]
        })
        .collect();
    print_table(&["ID", "State", "Bytes", "Filename"], rows);
}

pub fn history_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no history entries)");
        return;
    }
    let rows = arr
        .iter()
        .map(|h| {
            let visits = h
                .get("visitCount")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .to_string();
            let title = h
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let url = h
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![visits, title, url]
        })
        .collect();
    print_table(&["Visits", "Title", "URL"], rows);
}

pub fn pref(name: &str, value: &Value) {
    if value.is_null() {
        println!("{name}: (unset)");
        return;
    }
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("?");
    let v = value.get("value").cloned().unwrap_or(Value::Null);
    let user = value
        .get("has_user_value")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    println!("{} {name}  [{kind}] = {v}", if user { "*" } else { " " });
}

pub fn pref_list(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no prefs matched)");
        return;
    }
    let rows = arr
        .iter()
        .map(|entry| {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let kind = entry
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let v = entry
                .get("value")
                .cloned()
                .unwrap_or(Value::Null)
                .to_string();
            let user = entry
                .get("has_user_value")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            vec![if user { "*".into() } else { "".into() }, name, kind, v]
        })
        .collect();
    print_table(&["", "Name", "Type", "Value"], rows);
}

pub fn workspaces_table(value: &Value) {
    let active_uuid = value.get("active").and_then(Value::as_str);
    let Some(arr) = value.get("workspaces").and_then(Value::as_array) else {
        println!("{value:#}");
        return;
    };
    let rows = arr
        .iter()
        .map(|w| {
            let uuid = w
                .get("uuid")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let name = w
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let icon = w
                .get("icon")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let mark = if Some(uuid.as_str()) == active_uuid {
                "*".into()
            } else {
                "".into()
            };
            vec![mark, uuid, icon, name]
        })
        .collect();
    print_table(&["", "UUID", "Icon", "Name"], rows);
}

pub fn session_tabs_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no tabs in sessionstore)");
        return;
    }
    let rows = arr
        .iter()
        .map(|tab| {
            let active = tab.get("active").and_then(Value::as_bool).unwrap_or(false);
            let pinned = tab.get("pinned").and_then(Value::as_bool).unwrap_or(false);
            let url = tab
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let title = tab
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let wi = tab.get("window").and_then(|v| v.as_u64()).unwrap_or(0);
            let ti = tab.get("tab").and_then(|v| v.as_u64()).unwrap_or(0);
            vec![
                if active { "*".into() } else { "".into() },
                if pinned { "📌".into() } else { "".into() },
                format!("W{wi}"),
                format!("T{ti}"),
                title,
                url,
            ]
        })
        .collect();
    print_table(&["", "", "Win", "Tab", "Title", "URL"], rows);
}

pub fn sessions_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no recently closed sessions)");
        return;
    }
    let rows = arr
        .iter()
        .map(|entry| {
            if let Some(tab) = entry.get("tab") {
                let sid = tab
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let title = tab
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let url = tab
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                vec!["tab".into(), sid, title, url]
            } else if let Some(win) = entry.get("window") {
                let sid = win
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let n = win
                    .get("tabs")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                vec!["window".into(), sid, format!("{n} tabs"), String::new()]
            } else {
                vec!["?".into(), String::new(), String::new(), String::new()]
            }
        })
        .collect();
    print_table(&["Type", "SessionID", "Title", "URL"], rows);
}

pub fn containers_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no containers)");
        return;
    }
    let rows = arr
        .iter()
        .map(|c| {
            let id = c
                .get("cookieStoreId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let name = c
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let color = c
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let icon = c
                .get("icon")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![id, name, color, icon]
        })
        .collect();
    print_table(&["CookieStoreID", "Name", "Color", "Icon"], rows);
}

pub fn search_engines_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no search engines)");
        return;
    }
    let rows = arr
        .iter()
        .map(|e| {
            let default = e.get("isDefault").and_then(Value::as_bool).unwrap_or(false);
            let name = e
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let alias = e
                .get("alias")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![if default { "*".into() } else { "".into() }, name, alias]
        })
        .collect();
    print_table(&["", "Name", "Alias"], rows);
}

pub fn visits_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no visits)");
        return;
    }
    let rows = arr
        .iter()
        .map(|v| {
            let id = v
                .get("visitId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let time = v
                .get("visitTime")
                .and_then(Value::as_f64)
                .map(|t| (t as u64).to_string())
                .unwrap_or_default();
            let transition = v
                .get("transition")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![id, time, transition]
        })
        .collect();
    print_table(&["VisitID", "VisitTime", "Transition"], rows);
}

pub fn mods_table(value: &Value) {
    let Some(arr) = value.as_array() else {
        println!("{value:#}");
        return;
    };
    if arr.is_empty() {
        println!("(no mods installed)");
        return;
    }
    let rows = arr
        .iter()
        .map(|m| {
            let enabled = m.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            let id = m
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let name = m
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            vec![if enabled { "*".into() } else { "".into() }, id, name]
        })
        .collect();
    print_table(&["", "ID", "Name"], rows);
}

use zenctl_protocol::Capability;

pub fn capabilities_table(caps: &[Capability]) {
    let rows = caps
        .iter()
        .map(|c| {
            vec![
                if c.available {
                    "ok".into()
                } else {
                    "stub".into()
                },
                serde_json::to_value(&c.method)
                    .unwrap()
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                format!("{:?}", c.tier),
                c.reason.as_deref().unwrap_or("").to_string(),
            ]
        })
        .collect();
    print_table(&["Status", "Method", "Tier", "Reason"], rows);
}

pub fn decompress_html(value: &Value) -> String {
    let html = value.get("html").and_then(Value::as_str).unwrap_or("");
    let compressed = value
        .get("compressed")
        .and_then(Value::as_str)
        .unwrap_or("");
    if compressed != "gzip+base64" {
        return html.to_string();
    }
    use base64::Engine;
    let gz = match base64::engine::general_purpose::STANDARD.decode(html) {
        Ok(b) => b,
        Err(e) => return format!("[decompress: base64 error: {e}]"),
    };
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(&gz[..]);
    let mut out = String::new();
    match decoder.read_to_string(&mut out) {
        Ok(_) => out,
        Err(e) => format!("[decompress: gzip error: {e}]"),
    }
}
