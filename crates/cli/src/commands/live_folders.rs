//! `zenctl live-folders` — Zen live folder operations.

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::json;
use zenctl_protocol::Method;

use super::CliOpts;
use crate::{client::Client, output};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Provider {
    Rss,
    GithubPullRequests,
    GithubIssues,
}

impl Provider {
    fn as_str(self) -> &'static str {
        match self {
            Provider::Rss => "rss",
            Provider::GithubPullRequests => "github:pull-requests",
            Provider::GithubIssues => "github:issues",
        }
    }
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List live folders.
    List,
    /// Create a live folder.
    Create {
        #[arg(value_enum)]
        provider: Provider,
        /// RSS/Atom feed URL. Required for provider `rss`.
        #[arg(long)]
        url: Option<String>,
        /// Override folder label.
        #[arg(long)]
        label: Option<String>,
    },
    /// Delete a live folder by id.
    Delete { folder_id: String },
    /// Refresh a live folder by id.
    Refresh { folder_id: String },
    /// Pause a live folder's auto-fetch timer.
    Pause { folder_id: String },
    /// Resume a live folder's auto-fetch timer.
    Resume { folder_id: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    let r = match cmd {
        Cmd::List => client.call_raw(Method::LiveFoldersList, json!({})).await?,
        Cmd::Create {
            provider,
            url,
            label,
        } => {
            client
                .call_raw(
                    Method::LiveFoldersCreate,
                    json!({
                        "provider": provider.as_str(),
                        "url": url,
                        "label": label,
                    }),
                )
                .await?
        }
        Cmd::Delete { folder_id } => {
            client
                .call_raw(Method::LiveFoldersDelete, json!({ "folder_id": folder_id }))
                .await?
        }
        Cmd::Refresh { folder_id } => {
            client
                .call_raw(
                    Method::LiveFoldersRefresh,
                    json!({ "folder_id": folder_id }),
                )
                .await?
        }
        Cmd::Pause { folder_id } => {
            client
                .call_raw(Method::LiveFoldersPause, json!({ "folder_id": folder_id }))
                .await?
        }
        Cmd::Resume { folder_id } => {
            client
                .call_raw(Method::LiveFoldersResume, json!({ "folder_id": folder_id }))
                .await?
        }
    };
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else {
        println!(
            "{}",
            output::short_summary(
                &r,
                &[
                    "count",
                    "created",
                    "deleted",
                    "refreshed",
                    "paused",
                    "resumed",
                    "id",
                    "label",
                ]
            )
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::test_helpers;
    use tokio::net::UnixStream;
    use zenctl_protocol::Method;

    #[tokio::test]
    async fn create_sends_provider_and_url() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Create {
            provider: Provider::Rss,
            url: Some("https://example.com/feed.xml".into()),
            label: None,
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::LiveFoldersCreate);
        assert_eq!(params.get("provider").and_then(|v| v.as_str()), Some("rss"));
        assert_eq!(
            params.get("url").and_then(|v| v.as_str()),
            Some("https://example.com/feed.xml")
        );

        test_helpers::write_response(&mut server_side, serde_json::json!({"created": true})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn pause_resume_send_folder_id() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Pause {
            folder_id: "abc123".into(),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::LiveFoldersPause);
        assert_eq!(
            params.get("folder_id").and_then(|v| v.as_str()),
            Some("abc123")
        );

        test_helpers::write_response(&mut server_side, serde_json::json!({"paused": true})).await;
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn github_provider_serializes_correctly() {
        let (client_side, mut server_side) = UnixStream::pair().unwrap();
        let mut client = Client::with_stream(client_side);
        let opts = CliOpts {
            json: false,
            dry_run: false,
            force: false,
            timeout: 15,
        };
        let cmd = Cmd::Create {
            provider: Provider::GithubPullRequests,
            url: None,
            label: Some("My PRs".into()),
        };

        let handle = tokio::spawn(async move {
            run(&mut client, &opts, &cmd).await.unwrap();
        });

        let (_id, method, params, _to) = test_helpers::read_request(&mut server_side).await;
        assert_eq!(method, Method::LiveFoldersCreate);
        assert_eq!(
            params.get("provider").and_then(|v| v.as_str()),
            Some("github:pull-requests")
        );
        assert_eq!(params.get("label").and_then(|v| v.as_str()), Some("My PRs"));

        test_helpers::write_response(&mut server_side, serde_json::json!({"created": true})).await;
        handle.await.unwrap();
    }
}
