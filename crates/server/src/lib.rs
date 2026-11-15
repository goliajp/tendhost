//! The composition root. This is the one place that knows every module:
//! it builds them, connects them, and mounts their routes. It holds no
//! business rule.

pub mod config;
mod install;
mod ports;
mod web;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tendhost_agents::Agents;
use tendhost_alerts::Alerts;
use tendhost_audit::Audit;
use tendhost_hosts::Hosts;
use tendhost_http::Authenticator;
use tendhost_identity::Identity;
use tendhost_jobs::Jobs;
use tendhost_kernel::{Bus, Clock, Result};
use tendhost_notify::Notify;
use tendhost_store::Store;
use tendhost_telemetry::Telemetry;

const TICK_EVERY: Duration = Duration::from_secs(15);

pub struct App {
    pub router: Router,
    /// Present until the owner account exists; shown to the operator.
    pub setup_token: Option<String>,
    telemetry: Arc<Telemetry>,
    jobs: Arc<Jobs>,
    notify: Arc<Notify>,
}

impl App {
    /// Starts the work that runs on a timer. Separate from [`assemble`] so
    /// that a test decides when time passes.
    pub fn spawn_background(self: &Arc<Self>) {
        let app = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(TICK_EVERY);
            loop {
                tick.tick().await;
                app.tick();
            }
        });
        let notify = self.notify.clone();
        tokio::spawn(async move { notify.work().await });
    }

    /// One pass of everything that depends on time passing: hosts gone
    /// silent, jobs due, runs that can no longer end.
    pub fn tick(&self) {
        if let Err(error) = self.telemetry.sweep() {
            tracing::error!(%error, "presence sweep failed");
        }
        if let Err(error) = self.jobs.tick() {
            tracing::error!(%error, "jobs tick failed");
        }
    }

    /// Sends the notifications that are waiting; the background worker
    /// does this continuously.
    pub async fn deliver_notifications(&self) -> Result<usize> {
        self.notify.deliver_waiting().await
    }
}

/// What the server needs to know about where it runs.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// The address people and hosts reach the server at.
    pub public_url: String,
    /// Mark the session cookie `Secure`.
    pub secure_cookies: bool,
}

pub fn assemble(store: &Store, clock: Arc<dyn Clock>, settings: Settings) -> Result<App> {
    let identity = tendhost_identity::Settings {
        secure_cookies: settings.secure_cookies,
    };
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

    let agents = Arc::new(Agents::new(
        store.keyspace("agents"),
        bus.clone(),
        clock.clone(),
        Arc::new(ports::HostsKnown(hosts.clone())),
    ));
    bus.attach(agents.clone());

    let alerts = Arc::new(Alerts::new(
        store.keyspace("alerts"),
        bus.clone(),
        clock.clone(),
        Arc::new(ports::HostsNames(hosts.clone())),
    ));
    bus.attach(alerts.clone());

    let notify = Arc::new(Notify::new(
        store.keyspace("notify"),
        bus.clone(),
        clock.clone(),
    ));
    bus.attach(notify.clone());

    let authenticator: Arc<dyn Authenticator> = identity.clone();
    let api = Router::new()
        .merge(tendhost_identity::router(identity))
        .merge(tendhost_audit::router(audit))
        .merge(tendhost_hosts::router(hosts.clone()))
        .merge(tendhost_telemetry::router(telemetry.clone(), clock))
        .merge(tendhost_jobs::router(jobs.clone()))
        .merge(tendhost_agents::router(agents.clone()))
        .merge(tendhost_alerts::router(alerts))
        .merge(tendhost_notify::router(notify.clone()))
        .layer(from_fn_with_state(
            authenticator,
            tendhost_http::authenticate,
        ));

    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/install.sh",
            get(move || install::serve(settings.public_url.clone())),
        )
        .nest("/api/v1", api)
        .merge(tendhost_gateway::router(
            hosts,
            telemetry.clone(),
            jobs.clone(),
            agents,
        ))
        .fallback(web::fallback);
    Ok(App {
        router,
        setup_token,
        telemetry,
        jobs,
        notify,
    })
}
