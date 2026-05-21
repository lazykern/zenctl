mod client;
mod host;
mod install;
mod profile;
mod session;
mod shortcuts;
#[cfg(target_os = "macos")]
mod zen_macos;

mod commands;
mod output;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use commands::Command;

// ---------------------------------------------------------------------------
// Entry point — detects whether we're running as a CLI or native host
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    use std::io::IsTerminal;
    let is_native_host = !std::io::stdin().is_terminal() && {
        let mut args = std::env::args_os();
        args.next(); // skip binary name
        match args.next() {
            None => true,
            Some(a) => a.to_string_lossy().ends_with(".json"),
        }
    };
    if is_native_host {
        tracing_subscriber::fmt()
            .with_env_filter("warn")
            .with_writer(std::io::stderr)
            .init();
        return host::run().await;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let cli = Cli::parse();

    // Socket path resolution
    if let Some(socket) = cli.socket.as_ref() {
        std::env::set_var("ZENCTL_SOCKET", socket);
    } else if let Some(profile) = cli.profile.as_ref() {
        let safe: String = profile
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        std::env::set_var(
            "ZENCTL_SOCKET",
            std::env::temp_dir().join(format!("zenctl-{safe}.sock")),
        );
    }

    // Commands that don't need a running daemon — handle first.
    if let Command::Install {
        variant,
        ext_dir,
        link,
        check,
        no_kill,
    } = cli.command
    {
        return install::run(install::InstallOpts {
            variant,
            ext_dir,
            link,
            check,
            no_kill,
        })
        .await;
    }
    if let Command::Ext {
        action: commands::ext::Cmd::Use { variant, ref dir },
    } = cli.command
    {
        return commands::ext::run_use(variant, dir.as_deref()).await;
    }
    if let Command::Completions { shell } = cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "zenctl", &mut std::io::stdout());
        return Ok(());
    }

    let mut client = client::Client::connect().await?;
    let opts = commands::CliOpts {
        json: cli.json,
        dry_run: cli.dry_run,
        force: cli.force,
        timeout: cli.timeout,
    };
    commands::dispatch(&mut client, &opts, cli.command).await
}

// ---------------------------------------------------------------------------
// Top-level CLI argument struct
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "zenctl",
    version,
    about = "Control plane for Zen Browser",
    after_help = "EXAMPLES:\n  zenctl status\n  zenctl tabs list\n  zenctl prefs get zen.view.compact\n  zenctl session list\n  zenctl bookmarks list\n  zenctl page info --active\n\nGenerate shell completions:\n  zenctl completions bash > ~/.local/share/bash-completion/completions/zenctl\n  zenctl completions zsh  > /usr/local/share/zsh/site-functions/_zenctl\n  zenctl completions fish > ~/.config/fish/completions/zenctl.fish\n\nSee 'zenctl <command> --help' for per-command examples."
)]
pub struct Cli {
    /// Emit JSON output where supported.
    #[arg(long, global = true)]
    pub json: bool,

    /// Preview what would happen without executing.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Skip confirmation prompts. Required for destructive commands.
    #[arg(long, global = true)]
    pub force: bool,

    /// Connect to a specific zenctl host socket.
    #[arg(long, global = true, env = "ZENCTL_SOCKET", value_hint = clap::ValueHint::FilePath)]
    pub socket: Option<std::path::PathBuf>,

    /// Use named socket at /tmp/zenctl-<name>.sock (for multi-instance hosts).
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Default timeout in seconds for extension requests (default: 15).
    /// Per-command --timeout flags override this for individual calls.
    #[arg(long, global = true, default_value_t = 15)]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Command,
}
