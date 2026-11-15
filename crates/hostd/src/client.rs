use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use tendhost_protocol::{
    ENROLL_PATH, EnrollRequest, EnrollResponse, Facts, Mode, Report, SYNC_PATH, SyncRequest,
    SyncResponse, VERSION,
};

pub struct Client {
    http: reqwest::Client,
    server: String,
}

/// How a sync that got an answer ended.
#[derive(Debug)]
pub enum Synced {
    Accepted(SyncResponse),
    /// The server does not know this host's credential: the host was
    /// removed there. Retrying cannot help.
    Rejected,
}

impl Client {
    pub fn new(server: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("tendhostd/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building the http client")?;
        Ok(Self {
            http,
            server: server.trim_end_matches('/').to_string(),
        })
    }

    pub async fn enroll(
        &self,
        token: &str,
        name: &str,
        mode: Mode,
        facts: Facts,
    ) -> Result<EnrollResponse> {
        let request = EnrollRequest {
            protocol: VERSION,
            token: token.to_string(),
            name: name.to_string(),
            mode,
            facts,
        };
        let response = self
            .http
            .post(format!("{}{ENROLL_PATH}", self.server))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("reaching {}", self.server))?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "the server refused the enrollment ({status}): {}",
                error_message(response).await
            );
        }
        response.json().await.context("reading the server's answer")
    }

    pub async fn sync(&self, credential: &str, reports: Vec<Report>) -> Result<Synced> {
        let response = self
            .http
            .post(format!("{}{SYNC_PATH}", self.server))
            .bearer_auth(credential)
            .json(&SyncRequest {
                protocol: VERSION,
                reports,
            })
            .send()
            .await
            .with_context(|| format!("reaching {}", self.server))?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Ok(Synced::Rejected);
        }
        if !status.is_success() {
            bail!(
                "the server answered {status}: {}",
                error_message(response).await
            );
        }
        Ok(Synced::Accepted(
            response
                .json()
                .await
                .context("reading the server's answer")?,
        ))
    }
}

/// The message of a `{"error": {"message"}}` body, or the body itself.
async fn error_message(response: reqwest::Response) -> String {
    let text = response.text().await.unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|body| body["error"]["message"].as_str().map(str::to_string))
        .unwrap_or(text)
}
