//! The composition root. This is the one place that knows every module:
//! it builds them, connects them, and mounts their routes. It holds no
//! business rule.

pub mod config;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tendhost_audit::Audit;
use tendhost_hosts::Hosts;
use tendhost_http::Authenticator;
use tendhost_identity::Identity;
use tendhost_kernel::{Bus, Clock, Result};
use tendhost_store::Store;
use tendhost_telemetry::Telemetry;

const SWEEP_EVERY: Duration = Duration::from_secs(15);

pub struct App {
    pub router: Router,
    /// Present until the owner account exists; shown to the operator.
    pub setup_token: Option<String>,
    telemetry: Arc<Telemetry>,
}

impl App {
    /// Starts the work that runs on a timer. Separate from [`assemble`] so
    /// that a test decides when time passes.
    pub fn spawn_background(&self) {
        let telemetry = self.telemetry.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(SWEEP_EVERY);
            loop {
                tick.tick().await;
                if let Err(error) = telemetry.sweep() {
                    tracing::error!(%error, "presence sweep failed");
                }
            }
        });
    }
}

pub fn assemble(
    store: &Store,
    clock: Arc<dyn Clock>,
    identity: tendhost_identity::Settings,
) -> Result<App> {
    let bus = Bus::new(clock.clone());

    let audit = Arc::new(Audit::new(store.keyspace("audit"))?);
    bus.attach(audit.clone());

    let identity = Arc::new(Identity::new(
        store.keyspace("identity"),
        bus.clone(),
        clock.clone(),
        identity,
    ));
    let setup_token = identity.begin_setup()?;

    let hosts = Arc::new(Hosts::new(
        store.keyspace("hosts"),
        bus.clone(),
        clock.clone(),
    ));

    let telemetry = Arc::new(Telemetry::new(
        store.keyspace("telemetry"),
        bus.clone(),
        clock.clone(),
    ));
    bus.attach(telemetry.clone());

    let authenticator: Arc<dyn Authenticator> = identity.clone();
    let api = Router::new()
        .merge(tendhost_identity::router(identity))
        .merge(tendhost_audit::router(audit))
        .merge(tendhost_hosts::router(hosts.clone()))
        .merge(tendhost_telemetry::router(telemetry.clone(), clock))
        .layer(from_fn_with_state(
            authenticator,
            tendhost_http::authenticate,
        ));

    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api/v1", api)
        .merge(tendhost_gateway::router(hosts, telemetry.clone()));
    Ok(App {
        router,
        setup_token,
        telemetry,
    })
}
