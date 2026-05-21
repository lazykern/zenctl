//! `zenctl containers` — contextual identity (container) operations.

use super::{confirm, CliOpts};
use crate::{client::Client, output};
use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::json;
use zenctl_protocol::Method;

#[derive(ValueEnum, Debug, Clone)]
pub enum ContainerColor {
    Blue,
    Turquoise,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
}

impl std::fmt::Display for ContainerColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum ContainerIcon {
    Briefcase,
    Dollar,
    Cart,
    Circle,
    Gift,
    Vacation,
    Food,
    Fruit,
    Pet,
    Tree,
    Chill,
    Fence,
    Fingerprint,
}

impl std::fmt::Display for ContainerIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(Subcommand)]
#[command(
    after_help = "EXAMPLES:\n  zenctl containers list\n  zenctl containers create Work --color blue --icon briefcase\n  zenctl containers update <cookie-store-id> --name Personal\n  zenctl containers remove <cookie-store-id>"
)]
pub enum Cmd {
    /// List containers.
    List,
    /// Create a container.
    Create {
        name: String,
        /// Container color (blue, turquoise, green, yellow, orange, red, pink, purple).
        #[arg(long, default_value = "blue")]
        color: ContainerColor,
        /// Container icon (briefcase, dollar, cart, circle, gift, vacation, food, fruit, pet, tree, chill, fence, fingerprint).
        #[arg(long, default_value = "circle")]
        icon: ContainerIcon,
    },
    /// Update a container.
    Update {
        cookie_store_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        color: Option<ContainerColor>,
        #[arg(long)]
        icon: Option<ContainerIcon>,
    },
    /// Remove a container.
    Remove { cookie_store_id: String },
}

pub async fn run(client: &mut Client, opts: &CliOpts, cmd: &Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            let value = client.call_raw(Method::ContainersList, json!({})).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                output::containers_table(&value);
            }
        }
        Cmd::Create { name, color, icon } => {
            let value = client
                .call_raw(
                    Method::ContainersCreate,
                    json!({ "name": name, "color": color.to_string(), "icon": icon.to_string() }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&value, &["cookieStoreId", "name", "color", "icon"])
                );
            }
        }
        Cmd::Update {
            cookie_store_id,
            name,
            color,
            icon,
        } => {
            let mut params = json!({ "cookie_store_id": cookie_store_id });
            if let Some(n) = name {
                params["name"] = json!(n);
            }
            if let Some(c) = color {
                params["color"] = json!(c.to_string());
            }
            if let Some(i) = icon {
                params["icon"] = json!(i.to_string());
            }
            let value = client.call_raw(Method::ContainersUpdate, params).await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&value, &["cookieStoreId", "name", "color", "icon"])
                );
            }
        }
        Cmd::Remove { cookie_store_id } => {
            confirm(opts, "remove container", cookie_store_id)?;
            let value = client
                .call_raw(
                    Method::ContainersRemove,
                    json!({ "cookie_store_id": cookie_store_id }),
                )
                .await?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "{}",
                    output::short_summary(&value, &["cookieStoreId", "name"])
                );
            }
        }
    }
    Ok(())
}
