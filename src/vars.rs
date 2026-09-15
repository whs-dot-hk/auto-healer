// Copyright (c) 수영 책방 Swimming Bookstore

use crate::config::{is_var_name, Query, Var};
use crate::shell;
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::time::Duration;

const DEFAULT_VAR_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn resolve(query: &Query) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (name, var) in &query.vars {
        let value = match var {
            Var::Literal(s) => s.clone(),
            Var::Command { command, timeout } => {
                let timeout = timeout.unwrap_or(DEFAULT_VAR_TIMEOUT);
                let output = shell::run(command, query.cwd.as_deref(), Some(timeout), true).await?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    bail!(
                        "var '{name}' command exited {}: {}",
                        output.status,
                        stderr.trim()
                    );
                }
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
        };
        tracing::debug!(query = %query.name, var = %name, value = %value, "resolved var");
        out.insert(name.clone(), value);
    }
    Ok(out)
}

#[derive(Clone, Copy)]
enum Quote {
    None,
    Double,
    Single,
}

impl Quote {
    fn closer(self) -> Option<char> {
        match self {
            Quote::Double => Some('"'),
            Quote::Single => Some('\''),
            Quote::None => None,
        }
    }
}

/// `${name}` in PromQL: quoted values are escaped; unquoted must be number/duration/ident.
pub fn subst_promql(template: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    subst(template, vars, true)
}

/// `${name}` in the heal command: raw value.
pub fn subst_raw(template: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    subst(template, vars, false)
}

fn subst(template: &str, vars: &BTreeMap<String, String>, promql: bool) -> Result<String> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    let mut quote = Quote::None;
    let mut escaped = false;

    while i < chars.len() {
        let c = chars[i];

        if promql {
            if let Some(closer) = quote.closer() {
                if escaped {
                    escaped = false;
                    out.push(c);
                    i += 1;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    out.push(c);
                    i += 1;
                    continue;
                }
                if c == closer {
                    quote = Quote::None;
                    out.push(c);
                    i += 1;
                    continue;
                }
            } else if c == '"' {
                quote = Quote::Double;
                out.push(c);
                i += 1;
                continue;
            } else if c == '\'' {
                quote = Quote::Single;
                out.push(c);
                i += 1;
                continue;
            }
        }

        if c == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            if let Some(rel) = chars[i + 2..].iter().position(|&ch| ch == '}') {
                let name: String = chars[i + 2..i + 2 + rel].iter().collect();
                if !is_var_name(&name) {
                    bail!("invalid placeholder '${{{name}}}'");
                }
                let Some(value) = vars.get(&name) else {
                    bail!("unknown variable '${{{name}}}'");
                };
                if promql {
                    out.push_str(&promql_insert(value, quote)?);
                } else {
                    out.push_str(value);
                }
                i += 3 + rel;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }
    Ok(out)
}

fn promql_insert(value: &str, quote: Quote) -> Result<String> {
    match quote {
        Quote::Double => Ok(escape_promql_string(value, '"')),
        Quote::Single => Ok(escape_promql_string(value, '\'')),
        Quote::None => {
            if is_safe_unquoted(value) {
                Ok(value.to_string())
            } else {
                bail!(
                    "var value {value:?} is not a number, duration, or identifier; put ${{var}} inside quotes"
                )
            }
        }
    }
}

fn escape_promql_string(value: &str, quote: char) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

fn is_safe_unquoted(value: &str) -> bool {
    !value.is_empty() && (is_number(value) || is_duration(value) || is_ident(value))
}

fn is_number(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    let (num, exp) = match s.split_once(['e', 'E']) {
        Some((n, e)) => (n, Some(e)),
        None => (s, None),
    };
    let mut parts = num.split('.');
    let a = parts.next().unwrap_or("");
    let b = parts.next();
    let extra = parts.next();
    let ok_num = extra.is_none()
        && (!a.is_empty() || b.is_some_and(|x| !x.is_empty()))
        && a.chars().all(|c| c.is_ascii_digit())
        && b.map(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(true);
    let ok_exp = match exp {
        None => true,
        Some(e) => {
            let e = e.strip_prefix(['+', '-']).unwrap_or(e);
            !e.is_empty() && e.chars().all(|c| c.is_ascii_digit())
        }
    };
    ok_num && ok_exp
}

fn is_duration(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut saw = false;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i >= bytes.len() {
            return false;
        }
        match bytes[i] {
            b's' | b'm' | b'h' | b'd' | b'w' | b'y' => i += 1,
            _ => return false,
        }
        saw = true;
    }
    saw
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == ':' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{parse, Query, Var};

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn query_with_vars(vars: BTreeMap<String, Var>) -> Query {
        let mut q = parse(
            r#"
[[queries]]
name = "q"
query = "up"
cron = "* * * * *"
debounce = "1s"
command = "true"
"#,
        )
        .expect("parse")
        .queries
        .remove(0);
        q.vars = vars;
        q
    }

    #[test]
    fn subst_known() {
        let v = map(&[("job", "web"), ("n", "3")]);
        assert_eq!(
            subst_promql(r#"up{job="${job}"} == 0 and replicas < ${n}"#, &v).expect("subst"),
            r#"up{job="web"} == 0 and replicas < 3"#
        );
    }

    #[test]
    fn subst_unknown_errors() {
        assert!(subst_promql("${missing}", &BTreeMap::new()).is_err());
    }

    #[test]
    fn subst_leaves_bare_dollar() {
        assert_eq!(
            subst_raw("echo $HOME and ${", &BTreeMap::new()).expect("subst"),
            "echo $HOME and ${"
        );
    }

    #[test]
    fn quoted_value_escapes_quotes() {
        let v = map(&[("job", r#"web"prod"#)]);
        assert_eq!(
            subst_promql(r#"up{job="${job}"}"#, &v).expect("subst"),
            r#"up{job="web\"prod"}"#
        );
    }

    #[test]
    fn single_quoted_escapes() {
        let v = map(&[("job", "a'b")]);
        assert_eq!(
            subst_promql("up{job='${job}'}", &v).expect("subst"),
            r#"up{job='a\'b'}"#
        );
    }

    #[test]
    fn unquoted_unsafe_rejected() {
        let v = map(&[("n", "1 or vector(1)")]);
        assert!(subst_promql("up > ${n}", &v).is_err());
    }

    #[test]
    fn numbers_ok() {
        for n in ["0", "0.8", ".5", "1e-3", "+2", "-4.2E+1"] {
            let v = map(&[("n", n)]);
            assert_eq!(subst_promql("x > ${n}", &v).expect(n), format!("x > {n}"));
        }
        assert!(!is_number("."));
        assert!(!is_number("e10"));
        assert!(!is_number("1."));
    }

    #[test]
    fn duration_and_ident_ok() {
        let v = map(&[("w", "5m"), ("job", "api"), ("m", "http_requests_total")]);
        assert_eq!(
            subst_promql(r#"rate(${m}{job="${job}"}[${w}])"#, &v).expect("subst"),
            r#"rate(http_requests_total{job="api"}[5m])"#
        );
        assert!(is_duration("1h30m"));
        assert!(!is_duration("5"));
        assert!(!is_duration("m5"));
    }

    #[test]
    fn raw_does_not_escape() {
        let v = map(&[("u", "web; rm")]);
        assert_eq!(subst_raw("systemctl restart ${u}", &v).expect("subst"), "systemctl restart web; rm");
    }

    #[tokio::test]
    async fn resolve_literal_and_command() {
        let mut vars = BTreeMap::new();
        vars.insert("job".into(), Var::Literal("web".into()));
        vars.insert(
            "n".into(),
            Var::Command {
                command: "printf '  3\\n'".into(),
                timeout: Some(Duration::from_secs(2)),
            },
        );
        let q = query_with_vars(vars);
        let got = resolve(&q).await.expect("resolve");
        assert_eq!(got.get("job").map(String::as_str), Some("web"));
        assert_eq!(got.get("n").map(String::as_str), Some("3"));
    }

    #[tokio::test]
    async fn resolve_command_failure() {
        let mut vars = BTreeMap::new();
        vars.insert(
            "n".into(),
            Var::Command {
                command: "exit 1".into(),
                timeout: Some(Duration::from_secs(2)),
            },
        );
        let q = query_with_vars(vars);
        assert!(resolve(&q).await.is_err());
    }
}
