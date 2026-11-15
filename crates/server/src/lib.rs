//! The composition root. This is the one place that knows every module:
//! it builds them, connects them, and mounts their routes. It holds no
//! business rule.

pub mod config;
mod ports;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tendhost_audit::Audit;
use tendhost_hosts::Hosts;
use tendhost_http::Authenticator;
use tendhost_identity::Identity;
use tendhost_jobs::Jobs;
use tendhost_kernel::{Bus, Clock, Result};
use tendhost_store::Store;
use tendhost_telemetry::Telemetry;

const TICK_EVERY: Duration = Duration::from_secs(15);

pub struct App {
    pub router: Router,
    /// Present until the owner account exists; shown to the operator.
    pub setup_token: Option<String>,
    telemetry: Arc<Telemetry>,
    jobs: Arc<Jobs>,
}

impl App {
    /// Starts the work that runs on a timer. Separate from [`assemble`] so
    /// that a test decides when time passes.
    pub fn spawn_background(&self) {
        let (telemetry, jobs) = (self.telemetry.clone(), self.jobs.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(TICK_EVERY);
            loop {
                tick.tick().await;
                if let Err(error) = telemetry.sweep() {
                    tracing::error!(%error, "presence sweep failed");
                }
                if let Err(error) = jobs.tick() {
                    tracing::error!(%error, "jobs tick failed");
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

    let jobs = Arc::new(Jobs::new(
        store.keyspace("jobs"),
        bus.clone(),
        clock.clone(),
        Arc::new(ports::HostsDirectory(hosts.clone())),
    ));
    bus.attach(jobs.clone());

    let authenticator: Arc<dyn Authenticator> = identity.clone();
    let api = Router::new()
        .merge(tendhost_identity::router(identity))
        .merge(tendhost_audit::router(audit))
        .merge(tendhost_hosts::router(hosts.clone()))
        .merge(tendhost_telemetry::router(telemetry.clone(), clock))
        .merge(tendhost_jobs::router(jobs.clone()))
        .layer(from_fn_with_state(
            authenticator,
            tendhost_http::authenticate,
        ));

    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api/v1", api)
        .merge(tendhost_gateway::router(
            hosts,
            telemetry.clone(),
            jobs.clone(),
        ));
    Ok(App {
        router,
        setup_token,
        telemetry,
        jobs,
    })
}
