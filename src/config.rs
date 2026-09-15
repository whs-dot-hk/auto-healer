use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub prometheus: Prometheus,
    pub queries: Vec<Query>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Prometheus {
    #[serde(default = "default_prometheus_url")]
    pub url: String,
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: Duration,
}

impl Default for Prometheus {
    fn default() -> Self {
        Self {
            url: default_prometheus_url(),
            timeout: default_timeout(),
        }
    }
}

fn default_prometheus_url() -> String {
    "http://127.0.0.1:9090".into()
}

fn default_timeout() -> Duration {
    Duration::from_secs(15)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Query {
    pub name: String,
    pub query: String,
    /// 5 fields (`min hour day month dow`) or 6 (`sec min hour day month dow`).
    pub cron: String,
    /// Quiet period after a fire; doubles on later fires until `debounce_max`.
    #[serde(with = "humantime_serde")]
    pub debounce: Duration,
    /// Cap for exponential backoff. Defaults to 8× `debounce`.
    #[serde(default, with = "humantime_serde")]
    pub debounce_max: Option<Duration>,
    /// Passed to `sh -c`. `${var}` substituted each tick.
    pub command: String,
    #[serde(default, with = "humantime_serde")]
    pub command_timeout: Option<Duration>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub vars: BTreeMap<String, Var>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Var {
    Literal(String),
    Command {
        command: String,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
    },
}

pub fn load(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    parse(&raw)
}

pub fn parse(raw: &str) -> Result<Config> {
    let cfg: Config = toml::from_str(raw)?;
    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &Config) -> Result<()> {
    if cfg.queries.is_empty() {
        bail!("config has no [[queries]]");
    }
    let mut names = BTreeMap::new();
    for q in &cfg.queries {
        if q.name.trim().is_empty() {
            bail!("query name must not be empty");
        }
        if names.insert(q.name.as_str(), ()).is_some() {
            bail!("duplicate query name '{}'", q.name);
        }
        if q.query.trim().is_empty() {
            bail!("query '{}' has empty PromQL", q.name);
        }
        if q.command.trim().is_empty() {
            bail!("query '{}' has empty command", q.name);
        }
        parse_cron(&q.cron).with_context(|| format!("query '{}' has invalid cron", q.name))?;
        if q.debounce.is_zero() {
            bail!("query '{}' debounce must be > 0", q.name);
        }
        if let Some(max) = q.debounce_max {
            if max < q.debounce {
                bail!(
                    "query '{}': debounce_max ({max:?}) < debounce ({:?})",
                    q.name,
                    q.debounce
                );
            }
        }
        for (name, var) in &q.vars {
            if !is_var_name(name) {
                bail!(
                    "query '{}': var '{}' must match [A-Za-z_][A-Za-z0-9_]*",
                    q.name,
                    name
                );
            }
            if let Var::Command { command, .. } = var {
                if command.trim().is_empty() {
                    bail!("query '{}': var '{}' has empty command", q.name, name);
                }
            }
        }
    }
    Ok(())
}

pub fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

pub fn parse_cron(expr: &str) -> Result<cron::Schedule> {
    let expr = expr.trim();
    let n = expr.split_whitespace().count();
    let expr = match n {
        5 => format!("0 {expr}"),
        6 | 7 => expr.to_string(),
        n => bail!("expected 5–7 cron fields, got {n}"),
    };
    expr.parse().map_err(|e| anyhow::anyhow!("{e}"))
}

impl Query {
    pub fn debounce_cap(&self) -> Duration {
        self.debounce_max
            .unwrap_or_else(|| self.debounce.saturating_mul(8).max(self.debounce))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_toml() -> &'static str {
        r#"
[[queries]]
name = "q"
query = 'up == 0'
cron = "*/30 * * * * *"
debounce = "5s"
command = "true"
"#
    }

    #[test]
    fn parse_ok() {
        let cfg = parse(ok_toml()).expect("parse");
        assert_eq!(cfg.queries.len(), 1);
        assert_eq!(cfg.prometheus.url, "http://127.0.0.1:9090");
        assert_eq!(cfg.queries[0].debounce_cap(), Duration::from_secs(40));
    }

    #[test]
    fn empty_queries() {
        assert!(parse("[prometheus]\nurl='http://x'\n").is_err());
    }

    #[test]
    fn duplicate_names() {
        let raw = format!("{}\n{}", ok_toml(), ok_toml());
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn zero_debounce() {
        let raw = ok_toml().replace("5s", "0s");
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn debounce_max_too_small() {
        let raw = format!("{}\ndebounce_max = \"1s\"\n", ok_toml());
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn cron_five_fields() {
        parse_cron("*/1 * * * *").expect("5-field cron");
    }

    #[test]
    fn cron_bad() {
        assert!(parse_cron("* *").is_err());
        assert!(parse_cron("not a cron").is_err());
    }

    #[test]
    fn vars_and_command_var() {
        let raw = r#"
[[queries]]
name = "q"
query = 'up{job="${job}"} == 0'
cron = "* * * * *"
debounce = "1m"
command = "echo ${host}"
[queries.vars]
job = "web"
host = { command = "hostname", timeout = "2s" }
"#;
        let cfg = parse(raw).expect("parse");
        assert!(matches!(cfg.queries[0].vars.get("job"), Some(Var::Literal(_))));
        assert!(matches!(cfg.queries[0].vars.get("host"), Some(Var::Command { .. })));
    }

    #[test]
    fn bad_var_name() {
        let raw = r#"
[[queries]]
name = "q"
query = "up"
cron = "* * * * *"
debounce = "1m"
command = "true"
[queries.vars]
"1x" = "a"
"#;
        assert!(parse(raw).is_err());
    }

    #[test]
    fn var_name_rules() {
        assert!(is_var_name("job"));
        assert!(is_var_name("_a1"));
        assert!(!is_var_name(""));
        assert!(!is_var_name("1a"));
        assert!(!is_var_name("a-b"));
    }
}
