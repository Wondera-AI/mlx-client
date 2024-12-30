mod auto_update;
mod macros;
mod serve;

use auto_update::check_for_update;
use clap::Parser;
use serve::ServeCommand;
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
        Operation("Command operation failed"),
        Config("Command configuration invalid"),
        Validation("Command validation failed"),
        Communication("Failed to communicate with server"),
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

fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
