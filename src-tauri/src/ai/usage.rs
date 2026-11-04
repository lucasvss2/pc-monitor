//! Opt-in Claude limits from the undocumented `oauth/usage` endpoint, read with
//! the Claude Code token. The token is borrowed for one request and never renewed.
use super::{Bucket, Window};
use crate::ports::HttpError;
use serde::Deserialize;
use serde_json::Value;

/// `GET https://api.anthropic.com/api/oauth/usage`.
pub trait UsageApi: Send {
    fn usage(&self, token: &str) -> Result<Value, HttpError>;
}

/// Until the user opts in, no request is possible.
pub struct NoUsage;
impl UsageApi for NoUsage {
    fn usage(&self, _: &str) -> Result<Value, HttpError> {
        Err(HttpError::default())
    }
}

#[cfg(target_os = "linux")]
pub struct AnthropicUsage;
#[cfg(target_os = "linux")]
impl UsageApi for AnthropicUsage {
    fn usage(&self, token: &str) -> Result<Value, HttpError> {
        let response = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| HttpError::default())?
            .get("https://api.anthropic.com/api/oauth/usage")
            .header("User-Agent", "kvojps-monitor")
            .header("anthropic-beta", "oauth-2025-04-20")
            .bearer_auth(token)
            .send()
            .map_err(|_| HttpError::default())?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(HttpError::response(
                status,
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok()),
            ));
        }
        response.json().map_err(|_| HttpError {
            status: Some(status),
            ..HttpError::default()
        })
    }
}

/// The token and its expiry, read from `~/.claude/.credentials.json` for one
/// request. Never cloned, logged, serialized or written back.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub access_token: String,
    /// Epoch milliseconds.
    pub expires_at: Option<u64>,
}
impl Token {
    pub fn read(credentials: &str) -> Option<Token> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Credentials {
            claude_ai_oauth: Option<Token>,
        }
        serde_json::from_str::<Credentials>(credentials)
            .ok()?
            .claude_ai_oauth
            .filter(|token| !token.access_token.is_empty())
    }
    /// Identity of a refused token, so the same one is never sent again.
    pub fn id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.access_token.hash(&mut hash);
        hash.finish()
    }
}

/// The main bucket, then weekly buckets per model that came non-null.
/// Unknown fields are ignored; None when neither main window came.
pub fn buckets(response: &Value) -> Option<Vec<Bucket>> {
    let window = |value: &Value, duration_mins| {
        Some(Window {
            used_percent: Some(value["utilization"].as_f64()?),
            duration_mins: Some(duration_mins),
            resets_at: value["resets_at"]
                .as_str()
                .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.timestamp_millis().max(0) as u64),
            reset: false,
        })
    };
    let primary = window(&response["five_hour"], 300);
    let secondary = window(&response["seven_day"], 10080);
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    let mut buckets = vec![Bucket {
        id: "claude".into(),
        name: None,
        primary,
        secondary,
    }];
    for (key, id, name) in MODELS {
        if let Some(weekly) = window(&response[key], 10080) {
            buckets.push(Bucket {
                id: id.into(),
                name: Some(name.into()),
                primary: None,
                secondary: Some(weekly),
            });
        }
    }
    Some(buckets)
}

/// Weekly windows per model: response key, bucket id and model name.
const MODELS: [(&str, &str, &str); 2] = [
    ("seven_day_opus", "claude-opus", "Opus"),
    ("seven_day_sonnet", "claude-sonnet", "Sonnet"),
];
