//! The commands against a real server on a real socket.

use std::sync::Arc;

use clap::Parser;
use serde_json::{Value, json};
use tendhost_cli::api::Api;
use tendhost_cli::commands::{self, Command};
use tendhost_kernel::SystemClock;
use tendhost_store::Store;

#[derive(Parser)]
struct Line {
    #[command(subcommand)]
    command: Command,
}

struct Server {
    url: String,
    api: Api,
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
        let request = http
            .post(format!("{url}/api/v1/tokens"))
            .header("cookie", cookie);
        let token: Value = request
            .json(&json!({ "name": "cli", "role": "owner" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Self {
            api: Api::new(&url, token["secret"].as_str().unwrap()),
            url,
        }
    }

    /// Runs a command line as typed after `tendhost`.
    async fn run(&self, line: &str) -> anyhow::Result<String> {
        let words = std::iter::once("tendhost").chain(line.split_whitespace());
        let mut out = String::new();
        commands::run(
            &self.api,
            Line::try_parse_from(words)?.command,
            false,
            &mut out,
        )
        .await?;
        Ok(out)
    }

    /// Enrolls a host the way a daemon would, and returns its name.
    async fn enroll(&self, name: &str, mode: &str) {
        let opened = self
            .api
            .send(reqwest::Method::POST, "/enrollments", json!({}))
            .await
            .unwrap();
        let facts = json!({ "hostname": name, "os": "linux", "os_version": "6.12", "arch": "x86_64",
            "cpu_count": 8, "memory_bytes": 17179869184u64, "daemon_version": "0.1.0" });
        let request = json!({ "protocol": 1, "token": opened["token"], "name": name, "mode": mode, "facts": facts });
        let response = reqwest::Client::new()
            .post(format!("{}/hostd/v1/enroll", self.url))
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }
}

#[tokio::test]
async fn hosts_are_listed_shown_relabelled_and_removed_by_name() {
    let server = Server::start().await;
    assert!(
        server
            .run("hosts")
            .await
            .unwrap()
            .contains("tendhost enroll")
    );
    server.enroll("db-01", "observe").await;

    let listed = server.run("hosts").await.unwrap();
    let row = listed.lines().nth(1).unwrap();
    assert!(
        row.starts_with("db-01  online  observe  linux  x86_64  8     16.0 GB"),
        "{listed}"
    );

    assert_eq!(
        server.run("host label db-01 web prod").await.unwrap(),
        "Labels: prod, web.\n"
    );
    assert_eq!(
        server.run("host rename db-01 db").await.unwrap(),
        "Renamed to db.\n"
    );
    let shown = server.run("host show db").await.unwrap();
    assert!(
        shown.contains("labels    prod, web") && shown.contains("system    linux 6.12 (x86_64)"),
        "{shown}"
    );

    assert_eq!(server.run("host rm db").await.unwrap(), "Removed db.\n");
    assert!(
        server
            .run("host show db")
            .await
            .unwrap_err()
            .to_string()
            .contains("no host called db")
    );
}

#[tokio::test]
async fn a_job_is_created_run_and_listed_and_a_reporting_host_is_refused() {
    let server = Server::start().await;
    server.enroll("worker", "manage").await;
    server.enroll("watcher", "observe").await;

    let refused = server
        .run("job add x --host watcher --command true")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("--manage") && refused.contains("422"),
        "{refused}"
    );

    let created = server
        .run("job add nightly --host worker --command true --cron 0_3_*_*_*")
        .await;
    assert!(created.unwrap_err().to_string().contains("five fields"));
    let jobs = server.api.send(
        reqwest::Method::POST,
        "/jobs",
        json!({ "name": "nightly", "host_id": server.api.get("/hosts").await.unwrap()[0]["id"],
            "command": "true", "schedule": { "kind": "cron", "expr": "0 3 * * *" } }),
    );
    jobs.await.unwrap();
    let listed = server.run("jobs").await.unwrap();
    assert!(
        listed
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("nightly  worker  0 3 * * *  yes"),
        "{listed}"
    );

    assert!(
        server
            .run("job runs nightly")
            .await
            .unwrap()
            .contains("has not run yet")
    );
    assert!(
        server
            .run("job run nightly")
            .await
            .unwrap()
            .starts_with("Queued.")
    );
    assert!(
        server
            .run("job runs nightly")
            .await
            .unwrap()
            .contains("pending")
    );
    assert_eq!(
        server.run("job disable nightly").await.unwrap(),
        "Job nightly is disabled.\n"
    );
    assert!(
        server
            .run("job run nightly")
            .await
            .unwrap_err()
            .to_string()
            .contains("disabled")
    );
    assert_eq!(
        server.run("job rm nightly").await.unwrap(),
        "Removed job nightly.\n"
    );
}

#[tokio::test]
async fn enroll_prints_the_command_for_the_new_host() {
    let server = Server::start().await;
    let help = server.run("enroll --manage").await.unwrap();
    let line = help.lines().find(|l| l.contains("install.sh")).unwrap();
    assert!(line.contains(&format!(
        "curl -fsSL {}/install.sh | sh -s -- --token the_",
        server.url
    )));
    assert!(line.ends_with("--manage"));
}

#[tokio::test]
async fn alerts_audit_and_tracking_read_plainly() {
    let server = Server::start().await;
    server.enroll("dev", "observe").await;
    assert_eq!(server.run("alerts").await.unwrap(), "No open alerts.\n");
    assert!(
        server
            .run("agents")
            .await
            .unwrap()
            .contains("No coding agents")
    );
    assert!(
        server
            .run("track dev codex")
            .await
            .unwrap()
            .starts_with("Tracking codex on dev.")
    );
    assert!(server.run("track dev").await.unwrap().contains("deleted"));
    assert!(
        server
            .run("track dev cursor")
            .await
            .unwrap_err()
            .to_string()
            .contains("cursor")
    );
    let audit = server.run("audit --limit 2").await.unwrap();
    assert_eq!(audit.lines().count(), 3);
    assert!(
        audit
            .lines()
            .nth(1)
            .unwrap()
            .contains("agents.tracking_changed")
    );
}
