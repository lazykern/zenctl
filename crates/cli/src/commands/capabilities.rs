//! `zenctl capabilities` — list supported protocol methods and their tiers.

use super::CliOpts;
use crate::{client::Client, output};
use anyhow::Result;

pub async fn run(client: &mut Client, opts: &CliOpts) -> Result<()> {
    let caps = client.capabilities().await?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&caps)?);
    } else {
        output::capabilities_table(&caps);
    }
    Ok(())
}
