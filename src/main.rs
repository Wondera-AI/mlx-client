mod auto_update;
mod macros;
mod serve;

use auto_update::check_for_update;
use clap::Parser;
use serve::ServeCommand;
use std::collections::HashMap;
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use utils::prelude::*;

#[derive(Parser)]
#[command(name = "mlx")]
struct Cli {
    #[command(subcommand)]
    command: ServeCommand,
}
define_error! {
    pub enum CommandError {
        Operation("Operation failed"),
        Config("Configuration invalid"),
        Validation("Validation failed"),
        Communication("Failed to communicate with server"),
        UsageError("Failed to parse key-value pair"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .init();

    debug!("Check autoupdate");
    // let update_enabled = std::env::var("UPDATE")
    //     .map(|value| value.to_lowercase() != "false")
    //     .unwrap_or(true);

    // if update_enabled {
    //     check_for_update().await;
    // }

    Cli::parse().command.execute().await?;

    Ok(())
}

fn parse_key_val_pairs(s: &str) -> Result<(String, String), CommandError> {
    let mut parts = s.splitn(2, '=');
    let key = parts
        .next()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            CommandError::UsageError("Missing or empty key in key=value pair".to_string())
        })?;
    let value = parts
        .next()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            CommandError::UsageError("Missing or empty value in key=value pair".to_string())
        })?;
    Ok((key.to_string(), value.to_string()))
}

fn parse_hashmap(input: &str) -> Result<HashMap<String, String>, CommandError> {
    input
        .split(',')
        .map(parse_key_val_pairs)
        .collect::<Result<HashMap<_, _>, _>>()
}

fn parse_clap_hashmap() -> clap::builder::ValueParser {
    clap::builder::ValueParser::new(parse_hashmap)
}
