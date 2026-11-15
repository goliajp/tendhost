//! From something going wrong on a host to a message leaving the server,
//! across telemetry, alerts and notify, none of which knows the others.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use chrono::{Duration, TimeZone, Utc};
use serde_json::{Value, json};
use tendhost_kernel::{Clock, ManualClock};
use tendhost_protocol::{
    Disk, ENROLL_PATH, EnrollRequest, Facts, Metrics, Mode, Report, ReportBody, SYNC_PATH,
    SyncRequest, VERSION,
};
use tendhost_server::App;
use tendhost_store::Store;
use tower::ServiceExt;

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    auth: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, auth);
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

struct World {
    app: App,
    clock: Arc<ManualClock>,
    owner: String,
    host: String,
    host_id: String,
    /// titles the webhook receiver got
    received: Arc<Mutex<Vec<String>>>,
}

async fn world() -> World {
    let clock = Arc::new(ManualClock::at(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let app = tendhost_server::assemble(&Store::temp().unwrap(), clock.clone(), Default::default())
        .unwrap();
    let form =
        json!({ "setup_token": app.setup_token, "name": "owner", "password": "a long password" });
    let request = Request::post("/api/v1/setup")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(form.to_string()))
        .unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let request = Request::post("/api/v1/tokens")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "name": "test", "role": "owner" }).to_string(),
        ))
        .unwrap();
    let bytes = to_bytes(
        app.router
            .clone()
            .oneshot(request)
            .await
            .unwrap()
            .into_body(),
        1 << 20,
    )
    .await
    .unwrap();
    let token: Value = serde_json::from_slice(&bytes).unwrap();
    let owner = format!("Bearer {}", token["secret"].as_str().unwrap());

    let (_, opened) = call(&app.router, "POST", "/api/v1/enrollments", &owner, None).await;
    let facts = Facts {
        hostname: "db-01".into(),
        os: "linux".into(),
        os_version: "6.12".into(),
        arch: "x86_64".into(),
        cpu_count: 4,
        memory_bytes: 8 << 30,
        daemon_version: "0.1.0".into(),
    };
    let request = EnrollRequest {
        protocol: VERSION,
        token: opened["token"].as_str().unwrap().into(),
        name: "db-01".into(),
        mode: Mode::Manage,
        facts,
    };
    let (_, enrolled) = call(
        &app.router,
        "POST",
        ENROLL_PATH,
        "",
        Some(serde_json::to_value(request).unwrap()),
    )
    .await;

    let received: Arc<Mutex<Vec<String>>> = Arc::default();
    async fn take(
        State(received): State<Arc<Mutex<Vec<String>>>>,
        axum::Json(body): axum::Json<Value>,
    ) {
        received
            .lock()
            .unwrap()
            .push(body["title"].as_str().unwrap().to_string());
    }
    let receiver = Router::new()
        .route("/hook", post(take))
        .with_state(received.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, receiver).await.unwrap() });
    let channel = json!({ "name": "hook", "target": { "kind": "webhook", "url": url } });
    assert_eq!(
        call(
            &app.router,
            "POST",
            "/api/v1/channels",
            &owner,
            Some(channel)
        )
        .await
        .0,
        StatusCode::CREATED
    );

    World {
        clock,
        owner,
        host: format!("Bearer {}", enrolled["credential"].as_str().unwrap()),
        host_id: enrolled["host_id"].as_str().unwrap().to_string(),
        received,
        app,
    }
}

async fn report_disk(w: &World, used_bytes: u64) {
    let metrics = Metrics {
        load1: 0.1,
        memory_used_bytes: 1,
        uptime_secs: 1,
        disks: vec![Disk {
            mount: "/data".into(),
            total_bytes: 100,
            used_bytes,
        }],
    };
    let reports = vec![Report {
        at: w.clock.now(),
        body: ReportBody::Metrics(metrics),
    }];
    let request = serde_json::to_value(SyncRequest {
        protocol: VERSION,
        reports,
    })
    .unwrap();
    assert_eq!(
        call(&w.app.router, "POST", SYNC_PATH, &w.host, Some(request))
            .await
            .0,
        StatusCode::OK
    );
}

async fn open_summaries(w: &World) -> Vec<String> {
    let (_, alerts) = call(&w.app.router, "GET", "/api/v1/alerts", &w.owner, None).await;
    alerts
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["summary"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_full_disk_and_a_silent_host_are_alerted_by_name_and_sent_out() {
    let w = world().await;
    report_disk(&w, 95).await;
    assert_eq!(open_summaries(&w).await, ["db-01: disk /data is 95% full"]);

    w.clock.advance(Duration::seconds(90));
    w.app.tick();
    assert_eq!(
        open_summaries(&w).await,
        ["db-01: disk /data is 95% full", "db-01 is offline"]
    );

    report_disk(&w, 40).await;
    assert!(open_summaries(&w).await.is_empty());

    assert_eq!(w.app.deliver_notifications().await.unwrap(), 4);
    let mut titles = w.received.lock().unwrap().clone();
    titles.sort();
    assert_eq!(
        titles,
        [
            "Resolved: db-01 is offline",
            "Resolved: db-01: disk /data is 95% full",
            "db-01 is offline",
            "db-01: disk /data is 95% full",
        ]
    );
    let (_, deliveries) = call(&w.app.router, "GET", "/api/v1/deliveries", &w.owner, None).await;
    assert_eq!(deliveries.as_array().unwrap().len(), 4);
    assert!(
        deliveries
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["ok"] == true)
    );
}

#[tokio::test]
async fn removing_a_host_clears_everything_every_module_kept_about_it() {
    let w = world().await;
    let auth = &w.owner;
    report_disk(&w, 95).await;
    let job = json!({ "name": "j", "host_id": w.host_id, "command": "true", "schedule": { "kind": "manual" } });
    assert_eq!(
        call(&w.app.router, "POST", "/api/v1/jobs", auth, Some(job))
            .await
            .0,
        StatusCode::CREATED
    );
    let tracking = json!({ "tracked": ["codex"] });
    let path = format!("/api/v1/hosts/{}/agents", w.host_id);
    assert_eq!(
        call(&w.app.router, "PUT", &path, auth, Some(tracking))
            .await
            .0,
        StatusCode::OK
    );

    let path = format!("/api/v1/hosts/{}", w.host_id);
    assert_eq!(
        call(&w.app.router, "DELETE", &path, auth, None).await.0,
        StatusCode::NO_CONTENT
    );
    for list in [
        "/api/v1/hosts",
        "/api/v1/presence",
        "/api/v1/jobs",
        "/api/v1/agents",
        "/api/v1/alerts",
    ] {
        assert_eq!(
            call(&w.app.router, "GET", list, auth, None).await.1,
            json!([]),
            "{list}"
        );
    }
    // nothing is sent about a host that was removed on purpose
    w.app.deliver_notifications().await.unwrap();
    assert_eq!(
        *w.received.lock().unwrap(),
        ["db-01: disk /data is 95% full"]
    );
}

#[tokio::test]
async fn alert_settings_change_what_is_raised() {
    let w = world().await;
    let settings =
        json!({ "host_offline": true, "disk_usage": null, "agent_limit": 0.9, "job_failed": true });
    let (status, saved) = call(
        &w.app.router,
        "PUT",
        "/api/v1/alerts/settings",
        &w.owner,
        Some(settings),
    )
    .await;
    assert_eq!(
        (status, saved["disk_usage"].is_null()),
        (StatusCode::OK, true)
    );
    report_disk(&w, 99).await;
    assert!(open_summaries(&w).await.is_empty());
}
