// Copyright (c) 수영 책방 Swimming Bookstore

mod backoff;
mod config;
mod prometheus;
mod runner;
mod shell;
mod vars;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "auto-healer", about = "Query Prometheus on a cron and run shell commands with debounce")]
struct Args {
    /// Path to TOML config
    #[arg(short, long, default_value = "auto-healer.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .without_time()
        .with_target(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let cfg = config::load(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;

    tracing::info!(
        queries = cfg.queries.len(),
        prometheus = %cfg.prometheus.url,
        "starting: one loop per query"
    );

    let client = prometheus::Client::new(&cfg.prometheus)?;
    let mut handles = Vec::with_capacity(cfg.queries.len());

    for query in cfg.queries {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            if let Err(err) = runner::run_query(client, query).await {
                tracing::error!(error = %err, "query loop exited");
            }
        }));
    }

    signal::ctrl_c().await.ok();
    tracing::info!("shutting down");
    for handle in handles {
        handle.abort();
    }
    Ok(())
}
