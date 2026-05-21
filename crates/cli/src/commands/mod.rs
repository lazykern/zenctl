//! Command dispatch — routes parsed CLI arguments to the appropriate handler.
//!
//! Each top-level command maps to a module in this directory.
//! `dispatch()` is called from `main.rs` after the client connects.

pub mod target;

pub mod theme;

pub mod bookmarks;
pub mod boosts;
pub mod checkpoint;
pub mod compact;
pub mod containers;
pub mod cookies;
pub mod data;
pub mod downloads;
pub mod essentials;
pub mod ext;
pub mod find;
pub mod folders;
pub mod glance;
pub mod history;
pub mod live_folders;
pub mod media;
pub mod mods;
pub mod page;
pub mod prefs;
pub mod search;
pub mod session;
pub mod sessions;
pub mod share;
pub mod shortcuts;
pub mod snapshot;
pub mod split;
pub mod tabs;
pub mod urlbar;
pub mod wait;
pub mod watch;
pub mod windows;
pub mod workspace;

pub mod capabilities;
pub mod status;

use anyhow::Result;

use crate::client::Client;
use crate::install;

/// Global CLI options threaded through to every command.
#[derive(Clone, Copy)]
pub struct CliOpts {
    pub json: bool,
    pub dry_run: bool,
    pub force: bool,
    /// Default timeout in seconds for extension requests (overridable per-command).
    pub timeout: u64,
}

/// Helper: if dry_run is set, print a message and return Ok(()).
/// Otherwise, if force is not set, return an error asking for --force.
pub fn confirm(opts: &CliOpts, action: &str, detail: &str) -> Result<()> {
    if opts.dry_run {
        println!("Would {action}: {detail}");
        return Ok(());
    }
    if !opts.force {
        anyhow::bail!("Would {action}: {detail}\nUse --force to execute.");
    }
    Ok(())
}

// Import Cli and Command from main — we re-define the top-level Command here
// so the dispatch function can match on it.

/// Top-level command enum that mirrors all subcommands.
/// Clap derives this from the enum variants, so main.rs parses into this.
#[derive(clap::Subcommand)]
pub enum Command {
    /// Register the native messaging host and (re-)install the extension.
    /// Idempotent: safe to run repeatedly.
    Install {
        /// Manifest variant. Default: privileged (full feature set).
        #[arg(long, value_enum, default_value = "privileged")]
        variant: install::ManifestVariant,
        /// Override the extracted-extension directory (defaults to
        /// ~/.local/share/zenctl/extension).
        #[arg(long)]
        ext_dir: Option<std::path::PathBuf>,
        /// Symlink the extracted-extension dir to the repo's `extension/`
        /// instead of extracting a copy. Useful while developing.
        #[arg(long, conflicts_with = "ext_dir")]
        link: bool,
        /// Report current state without making changes.
        #[arg(long)]
        check: bool,
        /// Don't pkill the running native host. Old host keeps serving until
        /// it reconnects on its own.
        #[arg(long)]
        no_kill: bool,
    },
    /// Show host + extension status.
    Status,
    /// List supported methods and their tiers.
    Capabilities,
    /// Dump a JSON browser state snapshot for automation.
    Snapshot,
    /// Wait for tab/page conditions without shell sleeps.
    Wait {
        #[command(subcommand)]
        action: wait::Cmd,
    },
    /// Create/list lightweight browser session checkpoints.
    Checkpoint {
        #[command(subcommand)]
        action: checkpoint::Cmd,
    },
    /// Operate on Zen's compact mode.
    Compact {
        #[command(subcommand)]
        action: compact::Cmd,
    },
    /// Bookmark operations (via WebExtension).
    Bookmarks {
        #[command(subcommand)]
        action: bookmarks::Cmd,
    },
    /// Tab operations (via WebExtension).
    Tabs {
        #[command(subcommand)]
        action: tabs::Cmd,
    },
    /// Window operations (via WebExtension).
    Windows {
        #[command(subcommand)]
        action: windows::Cmd,
    },
    /// History search (via WebExtension).
    History {
        #[command(subcommand)]
        action: history::Cmd,
    },
    /// Read / write Zen / Gecko preferences (via WebExt experiment).
    Prefs {
        #[command(subcommand)]
        action: prefs::Cmd,
    },
    /// Operate on Zen workspaces.
    Workspace {
        #[command(subcommand)]
        action: workspace::Cmd,
    },
    /// Inspect and interact with the active web page.
    Page {
        #[command(subcommand)]
        action: page::Cmd,
    },
    /// Control media in the active/selected web page.
    Media {
        #[command(subcommand)]
        action: media::Cmd,
    },
    /// Download operations (via WebExtension).
    Downloads {
        #[command(subcommand)]
        action: downloads::Cmd,
    },
    /// Cookie operations (via WebExtension).
    Cookies {
        #[command(subcommand)]
        action: cookies::Cmd,
    },
    /// Recently closed tabs/windows (via WebExtension).
    Sessions {
        #[command(subcommand)]
        action: sessions::Cmd,
    },
    /// Clear browsing data (via WebExtension).
    Data {
        #[command(subcommand)]
        action: data::Cmd,
    },
    /// Container (contextual identity) operations.
    Containers {
        #[command(subcommand)]
        action: containers::Cmd,
    },
    /// Find text in a page.
    Find {
        #[command(subcommand)]
        action: find::Cmd,
    },
    /// Search engine operations.
    Search {
        #[command(subcommand)]
        action: search::Cmd,
    },
    /// Extension-side maintenance.
    Ext {
        #[command(subcommand)]
        action: ext::Cmd,
    },
    /// Session operations (profile-file tier).
    Session {
        #[command(subcommand)]
        action: session::Cmd,
    },
    /// Keyboard shortcuts operations (profile-file tier).
    Shortcuts {
        #[command(subcommand)]
        action: shortcuts::Cmd,
    },
    /// Glance operations (via WebExt experiment).
    Glance {
        #[command(subcommand)]
        action: glance::Cmd,
    },
    /// Split view operations (via WebExt experiment).
    Split {
        #[command(subcommand)]
        action: split::Cmd,
    },
    /// Address-bar operations (via WebExt experiment).
    Urlbar {
        #[command(subcommand)]
        action: urlbar::Cmd,
    },
    /// Zen essential (pinned) tab operations (via WebExt experiment).
    Essentials {
        #[command(subcommand)]
        action: essentials::Cmd,
    },
    /// Zen mod (theme) operations (via WebExt experiment).
    Mods {
        #[command(subcommand)]
        action: mods::Cmd,
    },
    /// Zen Boost operations (via WebExt experiment).
    Boosts {
        #[command(subcommand)]
        action: boosts::Cmd,
    },
    /// Theme operations (sugar over zen.theme.* preferences).
    Theme {
        #[command(subcommand)]
        action: theme::Cmd,
    },
    /// Zen live folders (RSS/GitHub) operations.
    LiveFolders {
        #[command(subcommand)]
        action: live_folders::Cmd,
    },
    /// Pinned-tab folders (Zen's `zen-folder` group elements).
    Folders {
        #[command(subcommand)]
        action: folders::Cmd,
    },
    /// Open Zen's native share dialog for the current tab or a given URL.
    /// (Windows + macOS only; use --check to test platform support.)
    Share {
        /// URL to share. Omit to share the active tab's URL.
        url: Option<String>,
        /// Title for the share payload (Windows only; macOS ignores it).
        #[arg(long)]
        title: Option<String>,
        /// Additional text for the share payload (Windows only).
        #[arg(long)]
        text: Option<String>,
        /// Just report whether native share is supported and exit.
        #[arg(long)]
        check: bool,
    },
    /// Stream live browser events (tab and window changes).
    Watch {
        /// Topics to watch: tabs, windows, all. Omit for all.
        topics: Vec<String>,
    },
    /// Generate shell completions for bash, zsh, or fish.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// Dispatch a parsed command to the appropriate handler.
///
/// `json` is the global `--json` flag; each command decides how to use it.
pub async fn dispatch(client: &mut Client, opts: &CliOpts, command: Command) -> Result<()> {
    match command {
        Command::Install { .. } => unreachable!("handled before daemon connect"),

        Command::Status => status::run(client, opts).await?,
        Command::Capabilities => capabilities::run(client, opts).await?,
        Command::Snapshot => snapshot::run(client, opts).await?,
        Command::Wait { action } => wait::run(client, opts, &action).await?,
        Command::Checkpoint { action } => checkpoint::run(client, opts, &action).await?,
        Command::Compact { action } => compact::run(client, opts, &action).await?,
        Command::Bookmarks { action } => bookmarks::run(client, opts, &action).await?,
        Command::Tabs { action } => tabs::run(client, opts, &action).await?,
        Command::Windows { action } => windows::run(client, opts, &action).await?,
        Command::History { action } => history::run(client, opts, &action).await?,
        Command::Downloads { action } => downloads::run(client, opts, &action).await?,
        Command::Cookies { action } => cookies::run(client, opts, &action).await?,
        Command::Sessions { action } => sessions::run(client, opts, &action).await?,
        Command::Data { action } => data::run(client, opts, &action).await?,
        Command::Containers { action } => containers::run(client, opts, &action).await?,
        Command::Find { action } => find::run(client, opts, &action).await?,
        Command::Search { action } => search::run(client, opts, &action).await?,
        Command::Prefs { action } => prefs::run(client, opts, &action).await?,
        Command::Workspace { action } => workspace::run(client, opts, &action).await?,
        Command::Page { action } => page::run(client, opts, &action).await?,
        Command::Media { action } => media::run(client, opts, &action).await?,
        Command::Ext { action } => ext::run(client, opts, &action).await?,
        Command::Session { action } => session::run(client, opts, &action).await?,
        Command::Shortcuts { action } => shortcuts::run(client, opts, &action).await?,
        Command::Glance { action } => glance::run(client, opts, &action).await?,
        Command::Split { action } => split::run(client, opts, &action).await?,
        Command::Urlbar { action } => urlbar::run(client, opts, &action).await?,
        Command::Essentials { action } => essentials::run(client, opts, &action).await?,
        Command::Mods { action } => mods::run(client, opts, &action).await?,
        Command::Boosts { action } => boosts::run(client, opts, &action).await?,
        Command::Theme { action } => theme::run(client, opts, &action).await?,
        Command::LiveFolders { action } => live_folders::run(client, opts, &action).await?,
        Command::Folders { action } => folders::run(client, opts, &action).await?,
        Command::Share {
            url,
            title,
            text,
            check,
        } => {
            share::run(
                client,
                opts,
                url.as_deref(),
                title.as_deref(),
                text.as_deref(),
                check,
            )
            .await?
        }
        Command::Watch { topics } => watch::run(client, opts, &topics).await?,
        Command::Completions { .. } => unreachable!("handled before daemon connect"),
    }
    Ok(())
}
