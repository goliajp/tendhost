//! The daemon's own code against a real server on a real socket.

use std::sync::Arc;

use serde_json::{Value, json};
use tendhost_hostd::client::{Client, Synced};
use tendhost_hostd::collect;
use tendhost_hostd::config::Config;
use tendhost_hostd::run::{Idle, Removed, Reporter};
use tendhost_kernel::SystemClock;
use tendhost_protocol::Mode;
use tendhost_store::Store;

struct Server {
    url: String,
    http: reqwest::Client,
    /// an owner's api token
    owner: String,
}

impl Server {
    async fn start() -> Self {
        let app = tendhost_server::assemble(
            &Store::temp().unwrap(),
            Arc::new(SystemClock),
            Default::default(),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let setup_token = app.setup_token.clone().unwrap();
        tokio::spawn(async move { axum::serve(listener, app.router).await.unwrap() });

        let http = reqwest::Client::new();
        let form =
            json!({ "setup_token": setup_token, "name": "owner", "password": "a long password" });
        let response = http
            .post(format!("{url}/api/v1/setup"))
            .json(&form)
            .send()
            .await
            .unwrap();
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let token: Value = http
            .post(format!("{url}/api/v1/tokens"))
            .header("cookie", cookie)
            .json(&json!({ "name": "test", "role": "owner" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Self {
            url,
            http,
            owner: token["secret"].as_str().unwrap().to_string(),
        }
    }

    async fn get(&self, path: &str) -> Value {
        let request = self
            .http
            .get(format!("{}{path}", self.url))
            .bearer_auth(&self.owner);
        request.send().await.unwrap().json().await.unwrap()
    }

    async fn enrollment_token(&self) -> String {
        let request = self
            .http
            .post(format!("{}/api/v1/enrollments", self.url))
            .bearer_auth(&self.owner);
        let opened: Value = request.send().await.unwrap().json().await.unwrap();
        opened["token"].as_str().unwrap().to_string()
    }

    async fn enroll(&self, name: &str) -> Config {
        self.enroll_as(name, Mode::Observe).await
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let request = self
            .http
            .post(format!("{}{path}", self.url))
            .bearer_auth(&self.owner)
            .json(&body);
        let response = request.send().await.unwrap();
        (response.status().as_u16(), response.json().await.unwrap())
    }

    async fn enroll_as(&self, name: &str, mode: Mode) -> Config {
        let token = self.enrollment_token().await;
        let client = Client::new(&self.url).unwrap();
        let enrolled = client
            .enroll(&token, name, mode, collect::facts().unwrap())
            .await
            .unwrap();
        Config {
            server: self.url.clone(),
            host_id: enrolled.host_id,
            credential: enrolled.credential,
            mode,
        }
    }
}

#[tokio::test]
async fn an_enrolled_daemon_shows_up_with_this_machines_facts_and_measurements() {
    let server = Server::start().await;
    let config = server.enroll("End To End").await;
    let mut reporter = Reporter::new(config.clone(), None).unwrap();
    let wait = reporter.cycle().await.unwrap();
    assert_eq!(wait.as_secs(), 30);
    reporter.cycle().await.unwrap();

    let hosts = server.get("/api/v1/hosts").await;
    assert_eq!(hosts.as_array().unwrap().len(), 1);
    assert_eq!(hosts[0]["id"], config.host_id.as_str());
    assert_eq!(hosts[0]["name"], "end-to-end");
    assert_eq!(
        hosts[0]["facts"]["hostname"],
        collect::hostname().unwrap().as_str()
    );
    assert_eq!(
        hosts[0]["facts"]["cpu_count"],
        collect::facts().unwrap().cpu_count
    );

    let presence = server.get("/api/v1/presence").await;
    assert_eq!(presence[0]["online"], true);
    let samples = server
        .get(&format!("/api/v1/hosts/{}/samples", config.host_id))
        .await;
    assert_eq!(samples.as_array().unwrap().len(), 2);
    assert!(samples[0]["memory_used_bytes"].as_u64().unwrap() > 0);
    assert!(!samples[0]["disks"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_bad_enrollment_token_is_refused_with_the_servers_reason() {
    let server = Server::start().await;
    let client = Client::new(&server.url).unwrap();
    let error = client
        .enroll("the_guess", "x", Mode::Observe, collect::facts().unwrap())
        .await
        .unwrap_err();
    let text = format!("{error:#}");
    assert!(
        text.contains("422") && text.contains("not a valid enrollment token"),
        "{text}"
    );
}

#[tokio::test]
async fn a_daemon_whose_host_was_removed_is_told_so() {
    let server = Server::start().await;
    let config = server.enroll("gone").await;
    let request = server
        .http
        .delete(format!("{}/api/v1/hosts/{}", server.url, config.host_id))
        .bearer_auth(&server.owner);
    assert_eq!(request.send().await.unwrap().status(), 204);

    let client = Client::new(&server.url).unwrap();
    assert!(matches!(
        client.sync(&config.credential, vec![]).await.unwrap(),
        Synced::Rejected
    ));
    let error = Reporter::new(config, None)
        .unwrap()
        .cycle()
        .await
        .unwrap_err();
    assert!(error.is::<Removed>());
}

#[tokio::test]
async fn an_unreachable_server_is_an_error_and_not_a_removal() {
    let config = Config {
        server: "http://127.0.0.1:9".into(),
        host_id: "host_x".into(),
        credential: "thh_x".into(),
        mode: Mode::Observe,
    };
    let error = Reporter::new(config, None)
        .unwrap()
        .cycle()
        .await
        .unwrap_err();
    assert!(!error.is::<Removed>());
    assert!(format!("{error:#}").contains("reaching http://127.0.0.1:9"));
}

#[tokio::test]
async fn a_job_runs_on_a_managing_host_and_its_output_comes_back() {
    let server = Server::start().await;
    let config = server.enroll_as("worker", Mode::Manage).await;
    let job = json!({ "name": "hello", "host_id": config.host_id, "command": "echo hello from $(uname -s); exit 4",
        "timeout_secs": 30, "schedule": { "kind": "manual" } });
    let (status, job) = server.post("/api/v1/jobs", job).await;
    assert_eq!(status, 201, "{job}");
    let job_id = job["id"].as_str().unwrap();
    let (status, _) = server
        .post(&format!("/api/v1/jobs/{job_id}/runs"), json!({}))
        .await;
    assert_eq!(status, 201);

    let mut reporter = Reporter::new(config, None).unwrap();
    reporter.cycle().await.unwrap();
    assert!(matches!(
        reporter.idle(std::time::Duration::from_secs(20)).await,
        Idle::ResultReady
    ));
    reporter.deliver_results().await.unwrap();

    let runs = server.get(&format!("/api/v1/jobs/{job_id}/runs")).await;
    assert_eq!(runs.as_array().unwrap().len(), 1);
    assert_eq!(runs[0]["state"], "failed");
    assert_eq!(runs[0]["exit_code"], 4);
    assert!(
        runs[0]["output"]
            .as_str()
            .unwrap()
            .starts_with("hello from ")
    );
}

#[tokio::test]
async fn the_server_refuses_a_job_for_a_host_that_only_reports() {
    let server = Server::start().await;
    let config = server.enroll("watcher").await;
    let job = json!({ "name": "x", "host_id": config.host_id, "command": "true", "schedule": { "kind": "manual" } });
    let (status, reply) = server.post("/api/v1/jobs", job).await;
    assert_eq!(status, 422);
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--manage")
    );
}

#[tokio::test]
async fn reports_made_while_the_server_is_away_are_kept_on_disk_and_delivered_later() {
    let server = Server::start().await;
    let config = server.enroll("patient").await;
    let dir = tempfile::tempdir().unwrap();
    let spool = dir.path().join("outbox.json");

    let away = Config {
        server: "http://127.0.0.1:9".into(),
        ..config.clone()
    };
    let mut reporter = Reporter::new(away, Some(spool.clone())).unwrap();
    assert!(reporter.cycle().await.is_err());
    assert!(reporter.cycle().await.is_err());
    assert!(spool.exists());
    drop(reporter);

    // a restarted daemon, with the server back
    let mut reporter = Reporter::new(config.clone(), Some(spool.clone())).unwrap();
    reporter.cycle().await.unwrap();
    assert!(!spool.exists());
    let samples = server
        .get(&format!("/api/v1/hosts/{}/samples", config.host_id))
        .await;
    assert_eq!(samples.as_array().unwrap().len(), 3);
}
