// Copyright (c) 수영 책방 Swimming Bookstore

use crate::backoff::Backoff;
use crate::config::{parse_cron, Query};
use crate::prometheus::{self, Client};
use crate::shell;
use crate::vars;
use anyhow::Result;
use chrono::Utc;
use std::time::Instant;
use tokio::time::{sleep, Duration};

pub async fn run_query(client: Client, query: Query) -> Result<()> {
    let schedule = parse_cron(&query.cron)?;
    let mut backoff = Backoff::new(query.debounce, query.debounce_cap());

    tracing::info!(
        query = %query.name,
        cron = %query.cron,
        debounce = ?query.debounce,
        cap = ?query.debounce_cap(),
        "watching"
    );

    loop {
        let now = Utc::now();
        let Some(next) = schedule.after(&now).next() else {
            anyhow::bail!("cron '{}' for '{}' has no upcoming ticks", query.cron, query.name);
        };
        let wait = (next - now)
            .to_std()
            .unwrap_or(Duration::from_secs(0))
            .max(Duration::from_millis(50));
        tracing::debug!(query = %query.name, %next, "next tick");
        sleep(wait).await;

        if let Some(left) = backoff.remaining(Instant::now()) {
            tracing::debug!(query = %query.name, remaining = ?left, "quiet: skip prometheus");
            continue;
        }

        let vars_map = match vars::resolve(&query).await {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(query = %query.name, error = %err, "could not resolve ${{vars}}; skip tick");
                continue;
            }
        };

        let promql = match vars::subst_promql(&query.query, &vars_map) {
            Ok(q) => q,
            Err(err) => {
                tracing::warn!(query = %query.name, error = %err, "bad PromQL template; skip tick");
                continue;
            }
        };

        match evaluate(&client, &query.name, &promql).await {
            Ok(true) => {
                let command = match vars::subst_raw(&query.command, &vars_map) {
                    Ok(c) => c,
                    Err(err) => {
                        tracing::warn!(query = %query.name, error = %err, "bad command template; skip tick");
                        continue;
                    }
                };
                let quiet = backoff.current();
                tracing::info!(query = %query.name, command = %command, "fire");
                match shell::run(
                    &command,
                    query.cwd.as_deref(),
                    query.command_timeout,
                    false,
                )
                .await
                {
                    Ok(out) if out.status.success() => {
                        tracing::info!(query = %query.name, quiet = ?quiet, "command ok");
                    }
                    Ok(out) => {
                        tracing::error!(query = %query.name, status = %out.status, "command failed");
                    }
                    Err(err) => tracing::error!(query = %query.name, error = %err, "command error"),
                }
                backoff.on_fire(Instant::now());
            }
            Ok(false) => {
                if backoff.on_healthy() {
                    tracing::info!(query = %query.name, "healthy: backoff reset");
                }
            }
            Err(err) => tracing::warn!(query = %query.name, error = %err, "Prometheus query failed"),
        }
    }
}

async fn evaluate(client: &Client, name: &str, promql: &str) -> Result<bool> {
    tracing::debug!(query = %name, promql, "instant query");
    let values = client.instant_query(promql).await?;
    Ok(prometheus::should_fire(&values))
}
