//! `zenctl media` — control media in the active/selected web page.

use super::CliOpts;
use crate::{
    client::Client,
    commands::target::{page_target, TargetArgs},
};
use anyhow::Result;
use clap::Subcommand;
use zenctl_protocol::Method;

#[derive(Subcommand)]
pub enum Cmd {
    /// Get media playback status.
    Status {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Start or resume playback.
    Play {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Pause playback.
    Pause {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Toggle play/pause.
    Toggle {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Skip to next track.
    Next {
        #[command(flatten)]
        target: TargetArgs,
    },
    /// Skip to previous track.
    Previous {
        #[command(flatten)]
        target: TargetArgs,
    },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    let (method, target) = match cmd {
        Cmd::Status { target } => (Method::MediaStatus, target),
        Cmd::Play { target } => (Method::MediaPlay, target),
        Cmd::Pause { target } => (Method::MediaPause, target),
        Cmd::Toggle { target } => (Method::MediaToggle, target),
        Cmd::Next { target } => (Method::MediaNext, target),
        Cmd::Previous { target } => (Method::MediaPrevious, target),
    };
    let value = client.call_raw(method, page_target(target)).await?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        // Media responses: show status/summary instead of raw JSON
        if let Some(state) = value.get("state").and_then(|v| v.as_str()) {
            let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let artist = value.get("artist").and_then(|v| v.as_str()).unwrap_or("");
            if title.is_empty() {
                println!("state: {state}");
            } else if artist.is_empty() {
                println!("{state}: {title}");
            } else {
                println!("{state}: {title} — {artist}");
            }
        } else {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
    }

    Ok(())
}
