use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::Value;

/// The server's api, as json in and json out.
pub struct Api {
    http: reqwest::Client,
    server: String,
    token: String,
}

impl Api {
    pub fn new(server: &str, token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            server: server.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.call(Method::GET, path, None).await
    }

    pub async fn send(&self, method: Method, path: &str, body: Value) -> Result<Value> {
        self.call(method, path, Some(body)).await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.call(Method::DELETE, path, None).await.map(|_| ())
    }

    /// A failed request becomes an error carrying the server's own
    /// sentence.
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut request = self
            .http
            .request(method, format!("{}/api/v1{path}", self.server))
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("reaching {}", self.server))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("reading the server's answer")?;
        let json: Value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::Null)
        };
        if !status.is_success() {
            let message = json["error"]["message"]
                .as_str()
                .map(str::to_string)
                .unwrap_or(text);
            bail!("{message} ({status})");
        }
        Ok(json)
    }
}
