use crate::config::Prometheus;
use anyhow::{bail, Context, Result};
use reqwest::Url;
use serde::Deserialize;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    query_url: Url,
}

impl Client {
    pub fn new(cfg: &Prometheus) -> Result<Self> {
        let base = cfg.url.trim_end_matches('/');
        let query_url = Url::parse(&format!("{base}/api/v1/query"))
            .with_context(|| format!("invalid prometheus url: {}", cfg.url))?;
        let http = reqwest::Client::builder().timeout(cfg.timeout).build()?;
        Ok(Self { http, query_url })
    }

    pub async fn instant_query(&self, query: &str) -> Result<Vec<f64>> {
        let mut url = self.query_url.clone();
        url.query_pairs_mut().append_pair("query", query);

        let resp = self
            .http
            .get(url)
            .send()
            .await
            .context("prometheus request failed")?;
        let status = resp.status();
        let body: ApiResponse = resp.json().await.context("prometheus json")?;
        samples(status, body)
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    data: Option<Data>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Data {
    #[serde(rename = "resultType")]
    result_type: String,
    result: serde_json::Value,
}

fn samples(status: reqwest::StatusCode, body: ApiResponse) -> Result<Vec<f64>> {
    if !status.is_success() || body.status != "success" {
        let err = body.error.unwrap_or_else(|| status.to_string());
        bail!("prometheus query failed: {err}");
    }
    let data = body.data.context("prometheus response missing data")?;
    match data.result_type.as_str() {
        "vector" => parse_vector(&data.result),
        "scalar" | "string" => parse_pair(&data.result).map(|v| vec![v]),
        other => bail!("unsupported prometheus resultType {other}"),
    }
}

fn parse_vector(result: &serde_json::Value) -> Result<Vec<f64>> {
    let samples = result.as_array().context("vector result is not an array")?;
    let mut out = Vec::with_capacity(samples.len());
    for sample in samples {
        let value = sample.get("value").context("sample missing value")?;
        out.push(parse_pair(value)?);
    }
    Ok(out)
}

fn parse_pair(value: &serde_json::Value) -> Result<f64> {
    let pair = value.as_array().context("expected [ts, value] pair")?;
    let raw = pair
        .get(1)
        .and_then(|v| v.as_str())
        .context("missing sample string")?;
    parse_sample(raw)
}

fn parse_sample(raw: &str) -> Result<f64> {
    match raw {
        "+Inf" => Ok(f64::INFINITY),
        "-Inf" => Ok(f64::NEG_INFINITY),
        "NaN" => Ok(f64::NAN),
        _ => raw.parse().with_context(|| format!("not a number: {raw}")),
    }
}

/// Instant-query fire rule: any non-NaN sample.
pub fn should_fire(values: &[f64]) -> bool {
    values.iter().any(|v| !v.is_nan())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_json(json: &str) -> Result<Vec<f64>> {
        let body: ApiResponse = serde_json::from_str(json)?;
        samples(reqwest::StatusCode::OK, body)
    }

    #[test]
    fn vector_values() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {}, "value": [1, "0.5"]},
                    {"metric": {}, "value": [1, "2"]}
                ]
            }
        }"#;
        let v = parse_json(json).expect("parse");
        assert_eq!(v, vec![0.5, 2.0]);
        assert!(should_fire(&v));
    }

    #[test]
    fn empty_vector_does_not_fire() {
        let json = r#"{
            "status": "success",
            "data": { "resultType": "vector", "result": [] }
        }"#;
        let v = parse_json(json).expect("parse");
        assert!(v.is_empty());
        assert!(!should_fire(&v));
    }

    #[test]
    fn nan_does_not_fire() {
        assert!(!should_fire(&[f64::NAN]));
        assert!(should_fire(&[f64::NAN, 1.0]));
        assert!(should_fire(&[f64::INFINITY]));
    }

    #[test]
    fn scalar() {
        let json = r#"{
            "status": "success",
            "data": { "resultType": "scalar", "result": [1, "3"] }
        }"#;
        assert_eq!(parse_json(json).expect("parse"), vec![3.0]);
    }

    #[test]
    fn inf_and_nan_samples() {
        assert_eq!(parse_sample("+Inf").expect("inf"), f64::INFINITY);
        assert_eq!(parse_sample("-Inf").expect("-inf"), f64::NEG_INFINITY);
        assert!(parse_sample("NaN").expect("nan").is_nan());
        assert!(parse_sample("nope").is_err());
    }

    #[test]
    fn api_error() {
        let body: ApiResponse = serde_json::from_str(
            r#"{"status":"error","error":"bad query"}"#,
        )
        .expect("json");
        let err = samples(reqwest::StatusCode::OK, body).unwrap_err();
        assert!(err.to_string().contains("bad query"));
    }

    #[test]
    fn matrix_unsupported() {
        let json = r#"{
            "status": "success",
            "data": { "resultType": "matrix", "result": [] }
        }"#;
        assert!(parse_json(json).is_err());
    }
}
