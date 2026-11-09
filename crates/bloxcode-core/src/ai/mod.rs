pub mod json;
pub mod prompts;

use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::config::Config;

/// HTTP client for the Vly AI gateway (`https://integrations.vly.ai/v1/llm`),
/// which exposes an OpenAI-compatible chat completions API.
pub struct VlyClient {
    base_url: String,
    token: String,
    model: String,
    temperature: f32,
    agent: ureq::Agent,
}

impl VlyClient {
    pub fn from_config(config: &Config) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(240))
            .timeout_write(Duration::from_secs(60))
            .user_agent(concat!("BloxCode/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            base_url: config.base_url.trim_end_matches('/').to_string(),
            token: config.effective_token(),
            model: config.model.clone(),
            temperature: config.temperature,
            agent,
        }
    }

    pub fn endpoint(&self) -> String {
        if self.base_url.ends_with("/chat/completions") {
            self.base_url.clone()
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn has_token(&self) -> bool {
        !self.token.is_empty()
    }

    /// Send a system prompt plus conversation turns and return assistant text.
    /// Retries transient failures twice with backoff.
    pub fn chat(&self, system: &str, turns: &[(String, String)]) -> Result<String> {
        if !self.has_token() {
            bail!("no Vly deployment token configured");
        }
        let mut messages = vec![json!({"role": "system", "content": system})];
        for (role, content) in turns {
            messages.push(json!({"role": role, "content": content}));
        }
        let payload = json!({
            "model": self.model,
            "messages": messages,
            "temperature": self.temperature,
            "stream": false,
        });

        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.post(&payload) {
                Ok(text) => return Ok(text),
                Err(error) => {
                    let retryable = error.to_string().contains("HTTP 429")
                        || error.to_string().contains("HTTP 5")
                        || error.to_string().contains("transport");
                    if !retryable || attempt >= 3 {
                        return Err(error);
                    }
                    sleep(Duration::from_secs(if attempt == 1 { 2 } else { 6 }));
                }
            }
        }
    }

    fn post(&self, payload: &Value) -> Result<String> {
        let response = self
            .agent
            .post(&self.endpoint())
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .send_json(payload);

        match response {
            Ok(response) => {
                let body: Value = response
                    .into_json()
                    .map_err(|error| anyhow!("gateway returned invalid JSON: {error}"))?;
                extract_content(&body)
                    .ok_or_else(|| anyhow!("gateway response contained no message content"))
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                let hint = match code {
                    401 | 403 => " — check the Vly deployment token in Settings",
                    404 => " — check the gateway base URL in Settings",
                    429 => " — rate limited by the Vly gateway",
                    _ => "",
                };
                bail!(
                    "HTTP {code}{hint}: {}",
                    crate::util::truncate(body.trim(), 400)
                )
            }
            Err(error) => bail!("transport error: {error}"),
        }
    }

    /// Minimal round trip used by the Settings connection test.
    pub fn test_connection(&self) -> Result<String> {
        let text = self.chat(
            "You are a health check endpoint. Reply with the single word OK.",
            &[("user".into(), "ping".into())],
        )?;
        Ok(crate::util::truncate(text.trim(), 80))
    }
}

fn extract_content(body: &Value) -> Option<String> {
    let choice = body.get("choices")?.get(0)?;
    let message = choice.get("message");
    if let Some(content) = message.and_then(|m| m.get("content")) {
        if let Some(text) = content.as_str() {
            return Some(text.to_string());
        }
        if let Some(parts) = content.as_array() {
            let joined: String = parts
                .iter()
                .filter_map(|part| {
                    part.get("text")
                        .and_then(|t| t.as_str())
                        .or_else(|| part.as_str())
                })
                .collect();
            if !joined.is_empty() {
                return Some(joined);
            }
        }
    }
    if let Some(text) = choice.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }
    body.get("output_text")
        .and_then(|t| t.as_str())
        .map(|t| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_appends_chat_completions() {
        let config = Config {
            base_url: "https://integrations.vly.ai/v1/llm".into(),
            ..Default::default()
        };
        let client = VlyClient::from_config(&config);
        assert_eq!(
            client.endpoint(),
            "https://integrations.vly.ai/v1/llm/chat/completions"
        );
    }

    #[test]
    fn endpoint_respects_full_url() {
        let config = Config {
            base_url: "http://localhost:1234/v1/chat/completions".into(),
            ..Default::default()
        };
        let client = VlyClient::from_config(&config);
        assert_eq!(
            client.endpoint(),
            "http://localhost:1234/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_string_content() {
        let body = json!({"choices": [{"message": {"content": "hello"}}]});
        assert_eq!(extract_content(&body).as_deref(), Some("hello"));
    }

    #[test]
    fn extracts_array_content() {
        let body = json!({"choices": [{"message": {"content": [{"text": "a"}, {"text": "b"}]}}]});
        assert_eq!(extract_content(&body).as_deref(), Some("ab"));
    }

    #[test]
    fn missing_content_is_none() {
        assert!(extract_content(&json!({"choices": []})).is_none());
    }
}
