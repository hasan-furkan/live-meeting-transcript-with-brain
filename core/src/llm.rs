use reqwest::Client;
use serde_json::{Value, json};
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::BuildHasher;
use std::time::{Duration, SystemTime};

const TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// `https` anywhere; plain `http` only to the local machine (local proxies).
/// Rejects userinfo, query and fragment so the key never goes to a host the user did not mean.
pub fn validate_api_base_url(s: &str) -> Result<(), &'static str> {
    const INVALID: &str = "Invalid API URL";
    let url = reqwest::Url::parse(s).map_err(|_| INVALID)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(INVALID);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(INVALID);
    }
    match url.scheme() {
        "https" => Ok(()),
        "http"
            if matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
            ) =>
        {
            Ok(())
        }
        _ => Err(INVALID),
    }
}

pub struct LlmImage {
    pub media_type: String,
    pub base64: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmError {
    Network,
    Http(u16),
    BadResponse,
    MissingKey,
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network => write!(f, "network error"),
            Self::Http(status) => write!(f, "http error {status}"),
            Self::BadResponse => write!(f, "unexpected response"),
            Self::MissingKey => write!(f, "API key missing"),
        }
    }
}

impl std::error::Error for LlmError {}

pub struct LlmClient {
    http: Client,
    endpoint: String,
    api_key: String,
    model: String,
    session_id: String,
}

/// 32 lowercase hex chars from two independently seeded hashers; OpenCode Go
/// requires some session id on every request, other providers ignore it.
fn new_session_id() -> String {
    let time = RandomState::new().hash_one(SystemTime::now());
    let pid = RandomState::new().hash_one(std::process::id());
    format!("{time:016x}{pid:016x}")
}

impl LlmClient {
    pub fn new(base_url: &str, api_key: String, model: &str) -> Self {
        // Building only fails when the TLS backend cannot start; the default
        // client keeps assist working, just without the 60 s timeout.
        let http = Client::builder()
            .timeout(TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            http,
            endpoint: format!("{}/v1/messages", base_url.trim_end_matches('/')),
            api_key,
            model: model.to_string(),
            session_id: new_session_id(),
        }
    }

    pub async fn complete(
        &self,
        system: &str,
        user_text: &str,
        image: Option<&LlmImage>,
        max_tokens: u32,
    ) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::MissingKey);
        }
        let body = self.request_body(system, user_text, image, max_tokens);
        let mut attempt = 0_u32;
        loop {
            let error = match self.send(&body).await {
                Ok(Some(reply)) => return Ok(reply),
                Ok(None) => return Err(LlmError::BadResponse),
                Err(LlmError::Http(status)) if status < 500 => return Err(LlmError::Http(status)),
                Err(error) => error,
            };
            if attempt == 1 {
                return Err(error);
            }
            attempt += 1;
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    fn request_body(
        &self,
        system: &str,
        user_text: &str,
        image: Option<&LlmImage>,
        max_tokens: u32,
    ) -> Value {
        let mut content = Vec::with_capacity(2);
        if let Some(image) = image {
            content.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.media_type,
                    "data": image.base64,
                },
            }));
        }
        content.push(json!({ "type": "text", "text": user_text }));
        json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{ "role": "user", "content": content }],
        })
    }

    /// `Ok(None)` for a 2xx reply without a text block.
    async fn send(&self, body: &Value) -> Result<Option<String>, LlmError> {
        let response = self
            .http
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-opencode-session", &self.session_id)
            .json(body)
            .send()
            .await
            .map_err(|_| LlmError::Network)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(LlmError::Http(status));
        }
        let value: Value = response.json().await.map_err(|_| LlmError::BadResponse)?;
        let Some(blocks) = value.get("content").and_then(Value::as_array) else {
            return Ok(None);
        };
        let mut text = String::new();
        let mut found = false;
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("text") {
                found = true;
                if let Some(part) = block.get("text").and_then(Value::as_str) {
                    text.push_str(part);
                }
            }
        }
        Ok(found.then_some(text))
    }
}

/// The first `{` through the last `}`; fences and surrounding prose are dropped.
pub(crate) fn json_slice(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_http::MockServer;
    use serde_json::{Value, json};

    fn text_response(parts: &[&str]) -> String {
        let content: Vec<Value> = parts
            .iter()
            .map(|text| json!({ "type": "text", "text": text }))
            .collect();
        json!({ "content": content }).to_string()
    }

    fn client(server: &MockServer) -> LlmClient {
        LlmClient::new(&server.url, "test-key".into(), "test-model")
    }

    #[test]
    fn api_url_accepts_https_and_local_http() {
        assert!(validate_api_base_url("https://opencode.ai/inference/go/anthropic").is_ok());
        assert!(validate_api_base_url("http://localhost:8080").is_ok());
        assert!(validate_api_base_url("http://127.0.0.1:8080/v1").is_ok());
        assert!(validate_api_base_url("http://[::1]:8080").is_ok());
    }

    #[test]
    fn api_url_rejects_cleartext_remote_and_userinfo() {
        assert_eq!(
            validate_api_base_url("http://example.com"),
            Err("Invalid API URL")
        );
        assert_eq!(
            validate_api_base_url("https://user:pw@example.com"),
            Err("Invalid API URL")
        );
        assert_eq!(
            validate_api_base_url("https://user@example.com"),
            Err("Invalid API URL")
        );
        assert_eq!(
            validate_api_base_url("https://example.com/?q=1"),
            Err("Invalid API URL")
        );
        assert_eq!(
            validate_api_base_url("https://example.com/#frag"),
            Err("Invalid API URL")
        );
        assert_eq!(
            validate_api_base_url("ftp://example.com"),
            Err("Invalid API URL")
        );
        assert_eq!(validate_api_base_url("not a url"), Err("Invalid API URL"));
        assert_eq!(validate_api_base_url(""), Err("Invalid API URL"));
    }

    #[tokio::test]
    async fn complete_sends_headers_and_parses_text() {
        let body = json!({ "content": [
            { "type": "thinking", "thinking": "ignored" },
            { "type": "text", "text": "hello " },
            { "type": "text", "text": "world" },
        ] })
        .to_string();
        let server = MockServer::start(vec![(200, body)]).await;
        // A trailing slash on the base URL must not produce `//v1/messages`.
        let client = LlmClient::new(&format!("{}/", server.url), "test-key".into(), "test-model");
        let reply = client.complete("sys", "user", None, 5).await.unwrap();
        assert_eq!(reply, "hello world");

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/messages");
        assert_eq!(request.header("x-api-key"), Some("test-key"));
        assert_eq!(request.header("anthropic-version"), Some("2023-06-01"));
        let session = request.header("x-opencode-session").unwrap_or_default();
        assert_eq!(session.len(), 32, "{session}");
        assert!(
            session
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "{session}"
        );
        let sent: Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(sent["model"], "test-model");
        assert_eq!(sent["max_tokens"], 5);
        assert_eq!(sent["system"], "sys");
        assert_eq!(sent["messages"][0]["role"], "user");
        assert_eq!(sent["messages"][0]["content"][0]["type"], "text");
        assert_eq!(sent["messages"][0]["content"][0]["text"], "user");
    }

    #[tokio::test]
    async fn retries_once_on_500_then_ok() {
        let server = MockServer::start(vec![
            (500, String::from("{}")),
            (200, text_response(&["recovered"])),
        ])
        .await;
        let reply = client(&server).complete("s", "u", None, 5).await.unwrap();
        assert_eq!(reply, "recovered");
        assert_eq!(server.requests().len(), 2);
    }

    #[tokio::test]
    async fn session_id_stable_across_requests_and_retry() {
        let server = MockServer::start(vec![
            (500, String::from("{}")),
            (200, text_response(&["one"])),
            (200, text_response(&["two"])),
        ])
        .await;
        let client = client(&server);
        client.complete("s", "u", None, 5).await.unwrap();
        client.complete("s", "u", None, 5).await.unwrap();
        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        let first = requests[0].header("x-opencode-session").unwrap();
        for request in &requests[1..] {
            assert_eq!(request.header("x-opencode-session"), Some(first));
        }
    }

    #[tokio::test]
    async fn session_id_differs_between_clients() {
        let server = MockServer::start(vec![
            (200, text_response(&["a"])),
            (200, text_response(&["b"])),
        ])
        .await;
        client(&server).complete("s", "u", None, 5).await.unwrap();
        client(&server).complete("s", "u", None, 5).await.unwrap();
        let requests = server.requests();
        let first = requests[0].header("x-opencode-session").unwrap();
        let second = requests[1].header("x-opencode-session").unwrap();
        assert_ne!(first, second);
    }

    #[tokio::test]
    async fn no_retry_on_401() {
        let server = MockServer::start(vec![(401, String::from("{}"))]).await;
        let err = client(&server)
            .complete("s", "u", None, 5)
            .await
            .unwrap_err();
        assert!(matches!(err, LlmError::Http(401)));
        assert_eq!(server.requests().len(), 1);
    }

    #[tokio::test]
    async fn error_display_hides_key() {
        let server =
            MockServer::start(vec![(401, String::from("{\"error\":\"body-secret\"}"))]).await;
        let client = LlmClient::new(&server.url, "sk-secret-key".into(), "m");
        let err = client.complete("s", "u", None, 5).await.unwrap_err();
        let text = err.to_string();
        assert!(!text.contains("sk-secret-key"), "{text}");
        assert!(!text.contains("body-secret"), "{text}");
    }

    #[tokio::test]
    async fn image_block_sent_first() {
        let server = MockServer::start(vec![(200, text_response(&["ok"]))]).await;
        let image = LlmImage {
            media_type: "image/png".into(),
            base64: "aGVsbG8=".into(),
        };
        client(&server)
            .complete("s", "look", Some(&image), 5)
            .await
            .unwrap();
        let sent: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
        let content = sent["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["type"], "base64");
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert_eq!(content[0]["source"]["data"], "aGVsbG8=");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[1]["text"], "look");
    }

    #[tokio::test]
    async fn missing_key_sends_nothing() {
        let server = MockServer::start(vec![(200, text_response(&["x"]))]).await;
        let client = LlmClient::new(&server.url, String::new(), "m");
        let err = client.complete("s", "u", None, 5).await.unwrap_err();
        assert!(matches!(err, LlmError::MissingKey));
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn no_text_block_is_bad_response() {
        let body = json!({ "content": [{ "type": "thinking", "thinking": "hmm" }] }).to_string();
        let server = MockServer::start(vec![(200, body)]).await;
        let err = client(&server)
            .complete("s", "u", None, 5)
            .await
            .unwrap_err();
        assert!(matches!(err, LlmError::BadResponse));
    }
}
