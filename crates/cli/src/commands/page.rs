//! `zenctl page` — inspect and interact with the active web page.

use super::CliOpts;
use crate::{
    client::Client,
    commands::target::{page_target, TargetArgs},
    output,
};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Return title, URL, active element, and ready state.
    Info {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Return visible page text.
    Text {
        #[command(flatten)]
        target: TargetArgs,
        /// Only return text from this frame index.
        #[arg(long)]
        frame_index: Option<u32>,
    },
    /// Return the full HTML source of the page (document.documentElement.outerHTML).
    Source {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Return a compact list of visible interactive elements.
    Snapshot {
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Only show elements from this frame index (0 = main, 1 = first iframe, ...).
        #[arg(long)]
        frame_index: Option<u32>,
    },
    /// List all frames (main + iframes) with their URLs and indices.
    ///
    /// Use the index values with --frame-index on other page commands.
    Frames {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Click an element by CSS selector or by a ref from `page snapshot`.
    ///
    /// EXAMPLES:
    ///   zenctl page click "button.submit"
    ///   zenctl page click --nth 3 "button.choice"   # click the 3rd match
    ///   zenctl page click --frame-index 1 "button"  # click in first iframe
    ///   zenctl page click --ref f0:e2
    Click {
        /// CSS selector. Optional when --ref is used.
        selector: Option<String>,
        #[command(flatten)]
        target: TargetArgs,
        /// Target a specific sub-frame by index.
        #[arg(long)]
        frame_index: Option<u32>,
        /// Click the Nth match of the selector (1-based, default 1).
        #[arg(long)]
        nth: Option<u32>,
        /// Element ref from `page snapshot` (for example f0:e2).
        #[arg(long)]
        r#ref: Option<String>,
    },
    /// Type text into an element by CSS selector.
    Type {
        selector: String,
        text: String,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long)]
        submit: bool,
        /// Target a specific sub-frame by index.
        #[arg(long)]
        frame_index: Option<u32>,
        /// Type into the Nth match of the selector (1-based, default 1).
        #[arg(long)]
        nth: Option<u32>,
    },
    /// Type text into an element ref from `page snapshot`.
    TypeRef {
        r#ref: String,
        text: String,
        #[command(flatten)]
        target: TargetArgs,
        #[arg(long)]
        submit: bool,
    },
    /// Send a keyboard key to the page (Enter, Space, Escape, ArrowDown, ...).
    Key {
        key: String,
        #[command(flatten)]
        target: TargetArgs,
        /// Target a specific sub-frame by index.
        #[arg(long)]
        frame_index: Option<u32>,
    },
    /// Wait until a CSS selector appears (or text appears on page).
    ///
    /// EXAMPLES:
    ///   zenctl page wait ".loaded"
    ///   zenctl page wait --text "Question 5"         # wait for text anywhere
    ///   zenctl page wait ".btn" --text "Submit"      # wait for selector with text
    ///   zenctl page wait --frame-index 1 --text "Q5" # wait in specific frame
    Wait {
        /// CSS selector to wait for (optional if --text is used alone).
        #[arg(default_value = "")]
        selector: String,
        #[command(flatten)]
        target: TargetArgs,
        /// Timeout in milliseconds (default: 5000).
        #[arg(long = "wait-timeout", default_value_t = 5000)]
        wait_timeout: u32,
        /// Target a specific sub-frame by index.
        #[arg(long)]
        frame_index: Option<u32>,
        /// Wait for this text to appear (in the element if selector given, or anywhere on page).
        #[arg(long)]
        text: Option<String>,
        /// Wait for the Nth match of the selector (1-based, default 1).
        #[arg(long)]
        nth: Option<u32>,
    },
    /// Run JavaScript in the page. Expert/debug command.
    /// Supports async/await, const/let, multi-statement code.
    /// The last expression is auto-returned (like browser devtools).
    ///
    /// EXAMPLES:
    ///   zenctl page eval 'document.title'
    ///   zenctl page eval 'const n = document.querySelectorAll("a").length; n'
    ///   zenctl page eval --timeout 60 --url-contains github '(async () => { ... })()'
    ///   zenctl page eval --frame-index 1 'document.querySelectorAll("button").length'
    Eval {
        code: String,
        #[command(flatten)]
        target: TargetArgs,
        /// Timeout in seconds. Overrides the global --timeout for this call.
        #[arg(long)]
        timeout: Option<u64>,
        /// Run in a specific sub-frame by index (0 = main frame, 1 = first iframe, ...).
        #[arg(long)]
        frame_index: Option<u32>,
    },
    /// Run a JavaScript file in the page. Reads the file and evaluates it with
    /// PageEval. Useful for multi-step automation scripts that exceed shell
    /// quoting limits.
    ///
    /// EXAMPLES:
    ///   zenctl page script automation/delete-files.js --url-contains chatgpt
    ///   zenctl page script cleanup.js --timeout 120 --active
    Script {
        /// Path to a .js file to evaluate in the page.
        file: std::path::PathBuf,
        #[command(flatten)]
        target: TargetArgs,
        /// Timeout in seconds (default: 60).
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// Run in a specific sub-frame by index.
        #[arg(long)]
        frame_index: Option<u32>,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::Info { target } => {
            let value = client
                .call_raw(Method::PageInfo, page_target(target))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "title: {}\nurl:   {}",
                    value.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    value.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                );
                if let Some(el) = value.get("activeElement").and_then(|v| v.as_str()) {
                    if !el.is_empty() {
                        println!("el:    {el}");
                    }
                }
            }
        }
        Cmd::Text {
            target,
            frame_index,
        } => {
            let mut params = page_target(target);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            let value = client.call_raw(Method::PageText, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    value.get("text").and_then(|v| v.as_str()).unwrap_or("")
                );
            }
        }
        Cmd::Source { target } => {
            let value = client
                .call_raw(Method::PageSource, page_target(target))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", output::decompress_html(&value));
            }
        }
        Cmd::Snapshot {
            target,
            limit,
            frame_index,
        } => {
            let mut params = page_target(target);
            params["limit"] = json!(limit);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            let value = client.call_raw(Method::PageSnapshot, params).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Cmd::Frames { target } => {
            let value = client
                .call_raw(Method::PageFrames, page_target(target))
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else if let Some(frames) = value.get("frames").and_then(|v| v.as_array()) {
                println!("{:<6} {:<8} {}", "Index", "FrameId", "URL");
                println!("{:<6} {:<8} {}", "-----", "-------", "---");
                for f in frames {
                    println!(
                        "{:<6} {:<8} {}",
                        f.get("index").and_then(|v| v.as_u64()).unwrap_or(0),
                        f.get("frameId").and_then(|v| v.as_u64()).unwrap_or(0),
                        f.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    );
                }
            }
        }
        Cmd::Click {
            selector,
            target,
            frame_index,
            nth,
            r#ref,
        } => {
            let mut params = page_target(target);
            if let Some(s) = selector {
                params["selector"] = json!(s);
            }
            if let Some(r) = r#ref {
                params["ref"] = json!(r);
            }
            if params.get("selector").is_none() && params.get("ref").is_none() {
                anyhow::bail!("selector or --ref required");
            }
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            if let Some(n) = nth {
                params["nth"] = json!(n);
            }
            let value = client.call_raw(Method::PageClick, params).await?;
            println!(
                "{}",
                output::short_summary(&value, &["clicked", "selector", "text"])
            );
        }
        Cmd::Type {
            selector,
            text,
            target,
            submit,
            frame_index,
            nth,
        } => {
            let mut params = page_target(target);
            params["selector"] = json!(selector);
            params["text"] = json!(text);
            params["submit"] = json!(submit);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            if let Some(n) = nth {
                params["nth"] = json!(n);
            }
            let value = client.call_raw(Method::PageType, params).await?;
            println!(
                "{}",
                output::short_summary(&value, &["typed", "selector", "submitted"])
            );
        }
        Cmd::TypeRef {
            r#ref,
            text,
            target,
            submit,
        } => {
            let mut params = page_target(target);
            params["ref"] = json!(r#ref);
            params["text"] = json!(text);
            params["submit"] = json!(submit);
            let value = client.call_raw(Method::PageType, params).await?;
            println!(
                "{}",
                output::short_summary(&value, &["typed", "selector", "submitted"])
            );
        }
        Cmd::Key {
            key,
            target,
            frame_index,
        } => {
            let mut params = page_target(target);
            params["key"] = json!(key);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            let value = client.call_raw(Method::PageKey, params).await?;
            println!("{}", output::short_summary(&value, &["key", "sent"]));
        }
        Cmd::Wait {
            selector,
            target,
            wait_timeout,
            frame_index,
            text,
            nth,
        } => {
            let mut params = page_target(target);
            if !selector.is_empty() {
                params["selector"] = json!(selector);
            }
            params["timeout"] = json!(wait_timeout);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            if let Some(t) = text {
                params["wait_text"] = json!(t);
            }
            if let Some(n) = nth {
                params["nth"] = json!(n);
            }
            let value = client.call_raw(Method::PageWait, params).await?;
            println!(
                "{}",
                output::short_summary(&value, &["found", "selector", "text", "elapsed_ms"])
            );
        }
        Cmd::Eval {
            code,
            target,
            timeout,
            frame_index,
        } => {
            let mut params = page_target(target);
            params["code"] = json!(code);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            let secs = timeout.unwrap_or(opts.timeout);
            let value = client
                .call_raw_timed(Method::PageEval, params, secs)
                .await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Cmd::Script {
            file,
            target,
            timeout,
            frame_index,
        } => {
            let code = std::fs::read_to_string(file)
                .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", file.display()))?;
            let mut params = page_target(target);
            params["code"] = json!(code);
            if let Some(fi) = frame_index {
                params["frame_index"] = json!(fi);
            }
            let value = client
                .call_raw_timed(Method::PageEval, params, *timeout)
                .await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
    }
    Ok(())
}
