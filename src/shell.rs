use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub async fn run(
    command: &str,
    cwd: Option<&str>,
    timeout: Option<Duration>,
    capture: bool,
) -> Result<std::process::Output> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }
    if let Some(cwd) = cwd {
        cmd.current_dir(Path::new(cwd));
    }

    let child = cmd.spawn().context("failed to spawn command")?;
    match timeout {
        Some(timeout) => match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(out) => out.context("command failed"),
            Err(_) => bail!("command timed out after {timeout:?}"),
        },
        None => child.wait_with_output().await.context("command failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_ok() {
        let out = run("echo hi", None, None, true).await.expect("run");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
    }

    #[tokio::test]
    async fn nonzero() {
        let out = run("exit 7", None, None, true).await.expect("run");
        assert_eq!(out.status.code(), Some(7));
    }

    #[tokio::test]
    async fn times_out() {
        let err = run("sleep 5", None, Some(Duration::from_millis(80)), true)
            .await
            .expect_err("timeout");
        assert!(err.to_string().contains("timed out"));
    }
}
