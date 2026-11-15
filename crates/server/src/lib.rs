//! The composition root. This is the one place that knows every module:
//! it builds them, connects them, and mounts their routes. It holds no
//! business rule.

pub mod config;

use std::sync::Arc;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tendhost_audit::Audit;
use tendhost_http::Authenticator;
use tendhost_identity::Identity;
use tendhost_kernel::{Bus, Clock, Result};
use tendhost_store::Store;

pub struct App {
    pub router: Router,
    /// Present until the owner account exists; shown to the operator.
    pub setup_token: Option<String>,
}

pub fn assemble(
    store: &Store,
    clock: Arc<dyn Clock>,
    identity: tendhost_identity::Settings,
) -> Result<App> {
    let bus = Bus::new(clock.clone());

    let audit = Arc::new(Audit::new(store.keyspace("audit")));
    bus.attach(audit.clone());

    let identity = Arc::new(Identity::new(
        store.keyspace("identity"),
        bus.clone(),
        clock,
        identity,
    ));
    let setup_token = identity.begin_setup()?;

    let authenticator: Arc<dyn Authenticator> = identity.clone();
    let api = Router::new()
        .merge(tendhost_identity::router(identity))
        .merge(tendhost_audit::router(audit))
        .layer(from_fn_with_state(
            authenticator,
            tendhost_http::authenticate,
        ));

    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api/v1", api);
    Ok(App {
        router,
        setup_token,
    })
}
