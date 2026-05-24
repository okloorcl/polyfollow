mod app;
mod cli;
mod config;
mod output;
mod storage;
mod validate;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "polyfollow=info".into()),
        )
        .without_time()
        .init();

    app::run(Cli::parse()).await
}
