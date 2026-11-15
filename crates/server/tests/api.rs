//! The assembled server, driven through its http surface.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tendhost_kernel::SystemClock;
use tendhost_store::Store;
use tower::ServiceExt;

struct Reply {
    status: StatusCode,
    cookie: Option<String>,
    body: Value,
}

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    auth: Option<(&str, &str)>,
    body: Option<Value>,
) -> Reply {
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
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    Reply {
        status,
        cookie,
        body,
    }
}

fn app() -> (Router, String) {
    let store = Store::temp().unwrap();
    let app = tendhost_server::assemble(&store, Arc::new(SystemClock), Default::default()).unwrap();
    (app.router, app.setup_token.unwrap())
}

async fn set_up(router: &Router, setup_token: &str) -> String {
    let form =
        json!({ "setup_token": setup_token, "name": "owner", "password": "a long password" });
    let reply = call(router, "POST", "/api/v1/setup", None, Some(form)).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    reply.cookie.unwrap()
}

#[tokio::test]
async fn first_run_setup_signs_the_owner_in() {
    let (router, setup_token) = app();
    assert_eq!(
        call(&router, "GET", "/api/v1/setup", None, None).await.body,
        json!({ "needed": true })
    );
    assert_eq!(
        call(&router, "GET", "/api/v1/session", None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    let cookie = set_up(&router, &setup_token).await;
    assert_eq!(
        call(&router, "GET", "/api/v1/setup", None, None).await.body,
        json!({ "needed": false })
    );
    let me = call(
        &router,
        "GET",
        "/api/v1/session",
        Some(("cookie", &cookie)),
        None,
    )
    .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(
        (me.body["name"].as_str(), me.body["role"].as_str()),
        (Some("owner"), Some("owner"))
    );
    assert!(me.body.get("password_hash").is_none());
}

#[tokio::test]
async fn setup_cannot_be_repeated_or_guessed() {
    let (router, setup_token) = app();
    let guess =
        json!({ "setup_token": "thsetup_guess", "name": "eve", "password": "a long password" });
    let reply = call(&router, "POST", "/api/v1/setup", None, Some(guess)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["error"]["code"], "invalid");
    assert!(reply.cookie.is_none());

    set_up(&router, &setup_token).await;
    let again = json!({ "setup_token": setup_token, "name": "eve", "password": "a long password" });
    assert_ne!(
        call(&router, "POST", "/api/v1/setup", None, Some(again))
            .await
            .status,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn sign_in_and_out_through_the_cookie() {
    let (router, setup_token) = app();
    set_up(&router, &setup_token).await;

    let wrong = json!({ "name": "owner", "password": "not the password" });
    let reply = call(&router, "POST", "/api/v1/session", None, Some(wrong)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert!(reply.cookie.is_none());

    let right = json!({ "name": "owner", "password": "a long password" });
    let cookie = call(&router, "POST", "/api/v1/session", None, Some(right))
        .await
        .cookie
        .unwrap();
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/v1/session",
            Some(("cookie", &cookie)),
            None
        )
        .await
        .status,
        StatusCode::OK
    );

    let out = call(
        &router,
        "DELETE",
        "/api/v1/session",
        Some(("cookie", &cookie)),
        None,
    )
    .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/v1/session",
            Some(("cookie", &cookie)),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_api_token_is_held_to_its_role() {
    let (router, setup_token) = app();
    let cookie = set_up(&router, &setup_token).await;
    let form = json!({ "name": "reader", "role": "viewer" });
    let made = call(
        &router,
        "POST",
        "/api/v1/tokens",
        Some(("cookie", &cookie)),
        Some(form),
    )
    .await;
    assert_eq!(made.status, StatusCode::CREATED);
    let bearer = format!("Bearer {}", made.body["secret"].as_str().unwrap());

    assert_eq!(
        call(
            &router,
            "GET",
            "/api/v1/session",
            Some(("authorization", &bearer)),
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    let listing = call(
        &router,
        "GET",
        "/api/v1/users",
        Some(("authorization", &bearer)),
        None,
    )
    .await;
    assert_eq!(listing.status, StatusCode::FORBIDDEN);
    assert_eq!(listing.body["error"]["code"], "forbidden");

    let listed = call(
        &router,
        "GET",
        "/api/v1/tokens",
        Some(("cookie", &cookie)),
        None,
    )
    .await;
    assert_eq!(listed.body.as_array().unwrap().len(), 1);
    assert!(listed.body[0].get("secret").is_none() && listed.body[0].get("fingerprint").is_none());
}

#[tokio::test]
async fn what_identity_does_shows_up_in_the_audit_log() {
    let (router, setup_token) = app();
    let cookie = set_up(&router, &setup_token).await;
    let form = json!({ "name": "bob", "password": "a long password", "role": "viewer" });
    let bob = call(
        &router,
        "POST",
        "/api/v1/users",
        Some(("cookie", &cookie)),
        Some(form),
    )
    .await;
    assert_eq!(bob.status, StatusCode::CREATED);

    assert_eq!(
        call(&router, "GET", "/api/v1/audit", None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let log = call(
        &router,
        "GET",
        "/api/v1/audit",
        Some(("cookie", &cookie)),
        None,
    )
    .await;
    assert_eq!(log.status, StatusCode::OK);
    let entries = log.body.as_array().unwrap();
    let topics: Vec<_> = entries
        .iter()
        .map(|e| e["topic"].as_str().unwrap())
        .collect();
    assert_eq!(topics, ["identity.user_created", "identity.owner_created"]);
    assert_eq!(entries[0]["actor"]["kind"], "user");
    assert_eq!(entries[0]["payload"]["user_id"], bob.body["id"]);
    assert!(!log.body.to_string().contains("a long password"));
}

#[tokio::test]
async fn health_needs_no_credential() {
    let (router, _) = app();
    let response = router
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_install_script_is_public_and_an_unknown_endpoint_is_a_json_error() {
    let store = Store::temp().unwrap();
    let settings = tendhost_server::Settings {
        public_url: "https://tend.example.com".into(),
        secure_cookies: true,
    };
    let router = tendhost_server::assemble(&store, Arc::new(SystemClock), settings)
        .unwrap()
        .router;

    let response = router
        .clone()
        .oneshot(Request::get("/install.sh").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let script = String::from_utf8(
        to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(script.contains("SERVER=\"https://tend.example.com\""));

    let missing = call(&router, "GET", "/api/v1/nothing", None, None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["error"]["code"], "not_found");
    let missing = call(&router, "POST", "/hostd/v9/sync", None, None).await;
    assert_eq!(missing.body["error"]["code"], "not_found");
}
