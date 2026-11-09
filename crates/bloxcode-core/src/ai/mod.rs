pub mod json;
pub mod models;
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
    /// Retries transient failures twice with backoff and adapts to models that
    /// reject a non-default temperature or expect a bare model id.
    pub fn chat(&self, system: &str, turns: &[(String, String)]) -> Result<String> {
        if !self.has_token() {
            bail!("no Vly deployment token configured");
        }
        let mut turns_with_system = vec![json!({"role": "system", "content": system})];
        for (role, content) in turns {
            turns_with_system.push(json!({"role": role, "content": content}));
        }
        let messages = json!(turns_with_system);

        // Some gateway aliases are bare (`gpt-5`) and some are canonical
        // (`openai/gpt-5`). Try the configured id first, then its bare form.
        let mut candidates = vec![self.model.clone()];
        if let Some((_, bare)) = self.model.split_once('/') {
            if !bare.is_empty() {
                candidates.push(bare.to_string());
            }
        }

        let mut candidate = 0usize;
        let mut include_temperature = true;
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let mut payload = json!({
                "model": candidates[candidate],
                "messages": messages.clone(),
                "temperature": self.temperature,
                "stream": false,
            });
            if !include_temperature {
                if let Some(object) = payload.as_object_mut() {
                    object.remove("temperature");
                }
            }

            match self.post(&payload) {
                Ok(text) => return Ok(text),
                Err(error) => {
                    let detail = error.to_string();
                    // Reasoning models only accept the provider default temperature.
                    if include_temperature && temperature_unsupported(&detail) {
                        include_temperature = false;
                        attempt = attempt.saturating_sub(1);
                        continue;
                    }
                    // A provider-prefixed id may not be routed by this deployment.
                    if candidate + 1 < candidates.len() && model_rejected(&detail) {
                        candidate += 1;
                        attempt = attempt.saturating_sub(1);
                        continue;
                    }
                    let retryable = detail.contains("HTTP 429")
                        || detail.contains("HTTP 5")
                        || detail.contains("transport");
                    if !retryable || attempt >= 3 {
                        return Err(error);
                    }
                    sleep(Duration::from_secs(if attempt == 1 { 2 } else { 6 }));
                }
            }
        }
    }

    /// URL of the OpenAI-compatible model catalogue for this deployment.
    pub fn models_endpoint(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        let base = base.strip_suffix("/chat/completions").unwrap_or(base);
        format!("{base}/models")
    }

    /// Fetch every text/coding model the deployment exposes. The gateway's own
    /// list is authoritative and supersedes the bundled fallback catalogue.
    pub fn list_models(&self) -> Result<Vec<models::ModelInfo>> {
        if !self.has_token() {
            bail!("no Vly deployment token configured");
        }
        let response = self
            .agent
            .get(&self.models_endpoint())
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/json")
            .call();

        match response {
            Ok(response) => {
                let body: Value = response
                    .into_json()
                    .map_err(|error| anyhow!("gateway returned invalid JSON: {error}"))?;
                let list = models::parse_models(&body);
                if list.is_empty() {
                    bail!("gateway returned no text or coding models");
                }
                Ok(list)
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                let hint = match code {
                    401 | 403 => " — check the Vly deployment token in Settings",
                    404 => " — this gateway does not expose a models endpoint",
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

fn temperature_unsupported(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("temperature")
        && (detail.contains("unsupported")
            || detail.contains("not support")
            || detail.contains("only support")
            || detail.contains("does not support")
            || detail.contains("invalid"))
}

fn model_rejected(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("model")
        && (detail.contains("not found")
            || detail.contains("does not exist")
            || detail.contains("not available")
            || detail.contains("unsupported")
            || detail.contains("invalid")
            || detail.contains("no such"))
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
    fn models_endpoint_is_derived_from_base_url() {
        let config = Config::default();
        let client = VlyClient::from_config(&config);
        assert_eq!(
            client.models_endpoint(),
            "https://integrations.vly.ai/v1/llm/models"
        );

        let config = Config {
            base_url: "http://localhost:1234/v1/chat/completions".into(),
            ..Default::default()
        };
        let client = VlyClient::from_config(&config);
        assert_eq!(client.models_endpoint(), "http://localhost:1234/v1/models");
    }

    #[test]
    fn detects_model_and_temperature_rejections() {
        assert!(temperature_unsupported(
            "HTTP 400: Unsupported value: 'temperature' does not support 0.4 with this model"
        ));
        assert!(!temperature_unsupported("HTTP 429: rate limited"));
        assert!(model_rejected("HTTP 404: model openai/gpt-5 not found"));
        assert!(!model_rejected("HTTP 429: rate limited"));
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

    fn fake_client(server: &tiny_http::Server, token: &str) -> VlyClient {
        let port = server.server_addr().to_ip().expect("ip listener").port();
        let config = Config {
            base_url: format!("http://127.0.0.1:{port}/v1/llm"),
            vly_token: token.to_string(),
            model: "openai/gpt-5".into(),
            ..Default::default()
        };
        VlyClient::from_config(&config)
    }

    fn json_response(body: &str, status: u16) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
        tiny_http::Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .expect("valid header"),
            )
    }

    fn request_body(request: &mut tiny_http::Request) -> String {
        let mut body = String::new();
        use std::io::Read;
        request.as_reader().read_to_string(&mut body).expect("body");
        body
    }

    #[test]
    fn list_models_calls_the_gateway_and_filters_non_text_models() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
        let client = fake_client(&server, "test-token");
        let handle = std::thread::spawn(move || {
            let mut request = server.recv().expect("request");
            assert_eq!(request.url(), "/v1/llm/models");
            let authorization = request
                .headers()
                .iter()
                .find(|header| {
                    header
                        .field
                        .to_string()
                        .eq_ignore_ascii_case("authorization")
                })
                .map(|header| header.value.as_str().to_string());
            assert_eq!(authorization.as_deref(), Some("Bearer test-token"));
            let body = "{\"data\":[{\"id\":\"openai/gpt-5\",\"name\":\"GPT-5\",\"type\":\"language\"},{\"id\":\"openai/text-embedding-3-large\",\"type\":\"embedding\"}]}";
            request.respond(json_response(body, 200)).expect("respond");
        });

        let models = client.list_models().expect("model list");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "openai/gpt-5");
        assert_eq!(models[0].provider, "openai");
        handle.join().expect("server thread");
    }

    #[test]
    fn chat_retries_without_temperature_when_the_model_rejects_it() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
        let client = fake_client(&server, "test-token");
        let handle = std::thread::spawn(move || {
            let mut first = server.recv().expect("first request");
            let first_body = request_body(&mut first);
            assert!(first_body.contains("\"temperature\""), "{first_body}");
            first
                .respond(json_response(
                    "{\"error\":{\"message\":\"Unsupported value: 'temperature' does not support 0.4 with this model\"}}",
                    400,
                ))
                .expect("respond");

            let mut second = server.recv().expect("second request");
            let second_body = request_body(&mut second);
            assert!(!second_body.contains("\"temperature\""), "{second_body}");
            second
                .respond(json_response(
                    "{\"choices\":[{\"message\":{\"content\":\"ok\"}}]}",
                    200,
                ))
                .expect("respond");
        });

        let reply = client
            .chat("system", &[("user".into(), "hello".into())])
            .expect("completion");
        assert_eq!(reply, "ok");
        handle.join().expect("server thread");
    }

    #[test]
    fn chat_falls_back_to_the_bare_model_id() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
        let client = fake_client(&server, "test-token");
        let handle = std::thread::spawn(move || {
            let mut first = server.recv().expect("first request");
            let first_body = request_body(&mut first);
            assert!(
                first_body.contains("\"model\":\"openai/gpt-5\""),
                "{first_body}"
            );
            first
                .respond(json_response(
                    "{\"error\":{\"message\":\"model openai/gpt-5 not found\"}}",
                    404,
                ))
                .expect("respond");

            let mut second = server.recv().expect("second request");
            let second_body = request_body(&mut second);
            assert!(second_body.contains("\"model\":\"gpt-5\""), "{second_body}");
            second
                .respond(json_response(
                    "{\"choices\":[{\"message\":{\"content\":\"bare ok\"}}]}",
                    200,
                ))
                .expect("respond");
        });

        let reply = client
            .chat("system", &[("user".into(), "hello".into())])
            .expect("completion");
        assert_eq!(reply, "bare ok");
        handle.join().expect("server thread");
    }
}
