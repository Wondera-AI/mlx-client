mod auto_update;
mod macros;
mod serve;

use auto_update::check_for_update;
use clap::Parser;

use serve::ServeCommand;
use std::collections::HashMap;
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use utils::prelude::*;

use crate::serve::deploy::DeployError;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Missing default value for required field {field} of type {field_type}")]
    MissingDefault { field: String, field_type: String },

    #[error("Deploy command error: {0}")]
    Deploy(#[from] DeployError),

    #[error("Config command error: {0}")]
    Config(String),

    #[error("Failed to parse arguments: {0}")]
    Parse(String),

    #[error("Command execution failed: {0}")]
    Execution(String),

    #[error("Usage error: {0}")]
    Usage(String),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HTTP(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// #[async_trait::async_trait]
// pub trait CommandHandler<C> {
//     async fn handle(cmd: C) -> error_stack::Result<(), CommandError>;
// }

/// The Handler trait defines how commands are executed and errors are handled.
/// It uses error-stack's Report type to provide rich error context and stack traces.
#[async_trait::async_trait]
pub trait Handler {
    /// The command type this handler processes
    type Command;

    /// The error type that can occur during command execution
    type Error: std::error::Error + Send + Sync + 'static;

    /// Execute the command and return a Result wrapped in a Report
    /// This provides stack traces and error context for debugging
    async fn handle(command: Self::Command) -> Result<(), Report<Self::Error>>;
}

/// A trait for converting regular Results into Report-wrapped Results
/// This helps maintain error context while keeping code ergonomic
pub trait IntoReport<T, E> {
    fn into_report(self) -> Result<T, Report<E>>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> IntoReport<T, E> for Result<T, E> {
    fn into_report(self) -> Result<T, Report<E>> {
        self.map_err(Report::new)
    }
}

#[derive(Parser)]
#[command(name = "mlx")]
struct Cli {
    #[command(subcommand)]
    command: ServeCommand,
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

/// This function provides a `ValueParser` for Clap to parse a `HashMap<K, V>`
/// from a command-line argument where key-value pairs are expected to be
/// in the format `key=value`, and the pairs are separated by commas.
///
/// Usage example:
///
/// ```rust
/// #[arg(
///     help = "Node selector labels (format: key1=value1,key2=value2)",
///     value_parser = crate::parse_clap_hashmap::<String, i32>()
/// )]
/// node_selectors: Option<HashMap<String, i32>>,
/// ```
///
/// This will allow an argument like:
///
/// `--node-selectors foo=1,bar=42`
fn parse_key_val_pairs(s: &str) -> Result<(String, String), CommandError> {
    let mut parts = s.splitn(2, '=');
    let key = parts
        .next()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| CommandError::Usage("Missing or empty key in key=value pair".to_string()))?;
    let value = parts
        .next()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            CommandError::Usage("Missing or empty value in key=value pair".to_string())
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

// GET SERVER URL
use once_cell::sync::Lazy;
use reqwest::get;
use std::sync::Arc;
use tokio::sync::OnceCell;

static LOCAL_SERVER_URL: &str = "http://localhost:3000/test";
// static REMOTE_SERVER_URL: &str = "http://3.132.162.86:30000/test";

// static REMOTE_SERVER_URL: &str = "http://52.14.40.210:30000/test";
static REMOTE_SERVER_URL: &str = "http://3.132.162.86:30000/test";

static SERVER_URL: Lazy<OnceCell<Arc<String>>> = Lazy::new(|| OnceCell::new());

async fn lazy_load_server_url() -> Arc<String> {
    // Try connecting to the local server if remote unavailable
    if is_server_available(LOCAL_SERVER_URL).await {
        println!("Connected to local server: {}", LOCAL_SERVER_URL);
        return Arc::new(LOCAL_SERVER_URL.to_string());
    }
    // Try connecting to the remote server first
    if is_server_available(REMOTE_SERVER_URL).await {
        println!("Connected to remote server: {}", REMOTE_SERVER_URL);
        return Arc::new(REMOTE_SERVER_URL.to_string());
    }
    // Panic if neither server is reachable
    panic!("No server available: could not connect to either local or remote server");
}

async fn is_server_available(url: &str) -> bool {
    match get(url).await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

async fn get_server_url() -> Arc<String> {
    SERVER_URL
        .get_or_init(|| async { lazy_load_server_url().await })
        .await
        .clone()
}
