//! A host's life as the server sees it: enrolled through the host
//! protocol, reporting, shown through the api, removed.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
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
    auth: Option<(&str, &str)>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some((name, value)) = auth {
        request = request.header(name, value);
    }
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
    /// an owner's api token, as an `authorization` header value
    owner: String,
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
    let form = json!({ "name": "test", "role": "owner" });
    let (_, token) = call(
        &app.router,
        "POST",
        "/api/v1/tokens",
        Some(("cookie", &cookie)),
        Some(form),
    )
    .await;
    World {
        app,
        clock,
        owner: format!("Bearer {}", token["secret"].as_str().unwrap()),
    }
}

fn facts(os_version: &str) -> Facts {
    Facts {
        hostname: "Build Box".into(),
        os: "linux".into(),
        os_version: os_version.into(),
        arch: "x86_64".into(),
        cpu_count: 16,
        memory_bytes: 64 << 30,
        daemon_version: "0.1.0".into(),
    }
}

async fn enroll(w: &World) -> (String, String) {
    let auth = Some(("authorization", w.owner.as_str()));
    let (status, opened) = call(&w.app.router, "POST", "/api/v1/enrollments", auth, None).await;
    assert_eq!(status, StatusCode::CREATED);
    let request = EnrollRequest {
        protocol: VERSION,
        token: opened["token"].as_str().unwrap().into(),
        name: "Build Box".into(),
        mode: Mode::Observe,
        facts: facts("6.12"),
    };
    let (status, enrolled) = call(
        &w.app.router,
        "POST",
        ENROLL_PATH,
        None,
        Some(serde_json::to_value(request).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let credential = format!("Bearer {}", enrolled["credential"].as_str().unwrap());
    (
        enrolled["host_id"].as_str().unwrap().to_string(),
        credential,
    )
}

async fn sync(w: &World, credential: &str, reports: Vec<Report>) -> (StatusCode, Value) {
    let request = serde_json::to_value(SyncRequest {
        protocol: VERSION,
        reports,
    })
    .unwrap();
    call(
        &w.app.router,
        "POST",
        SYNC_PATH,
        Some(("authorization", credential)),
        Some(request),
    )
    .await
}

#[tokio::test]
async fn an_enrolled_host_is_listed_online_with_what_it_reported() {
    let w = world().await;
    let (host_id, credential) = enroll(&w).await;
    let auth = Some(("authorization", w.owner.as_str()));

    let metrics = Metrics {
        load1: 1.5,
        memory_used_bytes: 8 << 30,
        uptime_secs: 3600,
        disks: vec![Disk {
            mount: "/".into(),
            total_bytes: 1000,
            used_bytes: 400,
        }],
    };
    let reports = vec![
        Report {
            at: w.clock.now(),
            body: ReportBody::Metrics(metrics),
        },
        Report {
            at: w.clock.now() + Duration::seconds(1),
            body: ReportBody::Facts(facts("6.13")),
        },
    ];
    let (status, reply) = sync(&w, &credential, reports).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reply["next_sync_secs"], 30);

    let (_, hosts) = call(&w.app.router, "GET", "/api/v1/hosts", auth, None).await;
    assert_eq!(hosts.as_array().unwrap().len(), 1);
    assert_eq!(hosts[0]["id"], host_id.as_str());
    assert_eq!(hosts[0]["name"], "build-box");
    assert_eq!(hosts[0]["mode"], "observe");
    assert_eq!(hosts[0]["facts"]["os_version"], "6.13");
    assert!(!hosts.to_string().contains("thh_"));

    let (_, presence) = call(&w.app.router, "GET", "/api/v1/presence", auth, None).await;
    assert_eq!(
        presence,
        json!([{ "host_id": host_id, "last_seen": "2026-01-01T00:00:00Z", "online": true }])
    );

    let path = format!("/api/v1/hosts/{host_id}/samples");
    let (_, samples) = call(&w.app.router, "GET", &path, auth, None).await;
    assert_eq!(samples.as_array().unwrap().len(), 1);
    assert_eq!(samples[0]["load1"], 1.5);
    assert_eq!(samples[0]["disks"][0]["used_bytes"], 400);
}

#[tokio::test]
async fn the_host_protocol_accepts_only_a_host_credential() {
    let w = world().await;
    let (_, credential) = enroll(&w).await;
    assert_eq!(
        sync(&w, "Bearer thh_guess", vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(sync(&w, &w.owner, vec![]).await.0, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &w.app.router,
        "POST",
        SYNC_PATH,
        None,
        Some(json!({ "protocol": 1, "reports": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // and a host credential opens nothing in the api
    let (status, _) = call(
        &w.app.router,
        "GET",
        "/api/v1/hosts",
        Some(("authorization", &credential)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn enrollment_needs_a_token_an_admin_opened() {
    let w = world().await;
    let request = EnrollRequest {
        protocol: VERSION,
        token: "the_guess".into(),
        name: "x".into(),
        mode: Mode::Observe,
        facts: facts("6.12"),
    };
    let (status, reply) = call(
        &w.app.router,
        "POST",
        ENROLL_PATH,
        None,
        Some(serde_json::to_value(request).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply["error"]["code"], "invalid");
    assert_eq!(
        call(&w.app.router, "POST", "/api/v1/enrollments", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_report_kind_the_server_does_not_know_is_accepted_and_ignored() {
    let w = world().await;
    let (_, credential) = enroll(&w).await;
    let request = json!({ "protocol": 2, "reports": [{ "at": "2026-01-01T00:00:00Z", "kind": "gpu", "temp": 70 }] });
    let (status, _) = call(
        &w.app.router,
        "POST",
        SYNC_PATH,
        Some(("authorization", &credential)),
        Some(request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_removed_host_can_no_longer_sync_and_its_measurements_are_gone() {
    let w = world().await;
    let (host_id, credential) = enroll(&w).await;
    let auth = Some(("authorization", w.owner.as_str()));
    let metrics = Metrics {
        load1: 1.0,
        memory_used_bytes: 1,
        uptime_secs: 1,
        disks: vec![],
    };
    sync(
        &w,
        &credential,
        vec![Report {
            at: w.clock.now(),
            body: ReportBody::Metrics(metrics),
        }],
    )
    .await;

    let path = format!("/api/v1/hosts/{host_id}");
    assert_eq!(
        call(&w.app.router, "DELETE", &path, auth, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sync(&w, &credential, vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&w.app.router, "GET", "/api/v1/presence", auth, None)
            .await
            .1,
        json!([])
    );
    let (_, samples) = call(&w.app.router, "GET", &format!("{path}/samples"), auth, None).await;
    assert_eq!(samples, json!([]));

    let (_, log) = call(&w.app.router, "GET", "/api/v1/audit", auth, None).await;
    let topics: Vec<_> = log
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["topic"].as_str().unwrap())
        .collect();
    assert_eq!(
        topics[..3],
        ["host.removed", "host.enrolled", "host.enrollment_opened"]
    );
    assert!(!topics.iter().any(|t| t.starts_with("telemetry.")));
}
