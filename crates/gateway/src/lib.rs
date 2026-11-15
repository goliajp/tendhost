//! The server end of the host protocol. An adapter and nothing more: it
//! checks the host's credential, turns each report into a call on the
//! module that owns that kind of data, and holds no rule of its own.
//!
//! Modules do not know the protocol, and the daemon does not know the
//! modules. A new kind of report is one variant in the protocol and one
//! arm in [`deliver`].

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use tendhost_agents::Agents;
use tendhost_hosts::Hosts;
use tendhost_http::{ApiResult, Credential, credential};
use tendhost_jobs::Jobs;
use tendhost_kernel::{Error, Result};
use tendhost_protocol as wire;
use tendhost_telemetry::Telemetry;

/// How often a host is asked to sync.
pub const SYNC_EVERY_SECS: u32 = 30;

#[derive(Clone)]
struct Ctx {
    hosts: Arc<Hosts>,
    telemetry: Arc<Telemetry>,
    jobs: Arc<Jobs>,
    agents: Arc<Agents>,
}

/// Routes at the paths the protocol names, relative to the server root.
pub fn router(
    hosts: Arc<Hosts>,
    telemetry: Arc<Telemetry>,
    jobs: Arc<Jobs>,
    agents: Arc<Agents>,
) -> Router {
    Router::new()
        .route(wire::ENROLL_PATH, post(enroll))
        .route(wire::SYNC_PATH, post(sync))
        .with_state(Ctx {
            hosts,
            telemetry,
            jobs,
            agents,
        })
}

async fn enroll(
    State(ctx): State<Ctx>,
    Json(request): Json<wire::EnrollRequest>,
) -> ApiResult<(StatusCode, Json<wire::EnrollResponse>)> {
    check_protocol(request.protocol)?;
    let (host, credential) = ctx.hosts.enroll(
        &request.token,
        &request.name,
        mode(request.mode),
        facts(request.facts),
    )?;
    ctx.telemetry.heard_from(&host.id)?;
    Ok((
        StatusCode::CREATED,
        Json(wire::EnrollResponse {
            host_id: host.id,
            credential,
        }),
    ))
}

async fn sync(
    State(ctx): State<Ctx>,
    headers: HeaderMap,
    Json(request): Json<wire::SyncRequest>,
) -> ApiResult<Json<wire::SyncResponse>> {
    let Some(Credential::Bearer(presented)) = credential(&headers) else {
        return Err(Error::Unauthenticated.into());
    };
    let host_id = ctx.hosts.authenticate(&presented)?;
    check_protocol(request.protocol)?;
    ctx.telemetry.heard_from(&host_id)?;
    for report in request.reports {
        deliver(&ctx, &host_id, report)?;
    }
    let mut directives: Vec<wire::Directive> = ctx
        .jobs
        .collect(&host_id)?
        .into_iter()
        .map(|a| wire::Directive::RunJob {
            run_id: a.run_id,
            command: a.command,
            timeout_secs: a.timeout_secs,
        })
        .collect();
    directives.push(wire::Directive::TrackAgents {
        agents: ctx.agents.tracked(&host_id)?,
    });
    Ok(Json(wire::SyncResponse {
        directives,
        next_sync_secs: SYNC_EVERY_SECS,
    }))
}

fn deliver(ctx: &Ctx, host_id: &str, report: wire::Report) -> Result<()> {
    match report.body {
        wire::ReportBody::Facts(reported) => {
            ctx.hosts.record_facts(host_id, facts(reported), report.at)
        }
        wire::ReportBody::Metrics(metrics) => ctx.telemetry.record(
            host_id,
            &tendhost_telemetry::Sample {
                at: report.at,
                load1: metrics.load1,
                memory_used_bytes: metrics.memory_used_bytes,
                uptime_secs: metrics.uptime_secs,
                disks: metrics
                    .disks
                    .into_iter()
                    .map(|d| tendhost_telemetry::Disk {
                        mount: d.mount,
                        total_bytes: d.total_bytes,
                        used_bytes: d.used_bytes,
                    })
                    .collect(),
            },
        ),
        wire::ReportBody::RunFinished(result) => ctx.jobs.finish(
            host_id,
            tendhost_jobs::Outcome {
                run_id: result.run_id,
                exit_code: result.exit_code,
                timed_out: result.timed_out,
                output: result.output,
                started_at: result.started_at,
                finished_at: result.finished_at,
            },
        ),
        wire::ReportBody::Agents(found) => ctx.agents.record(
            host_id,
            report.at,
            found.agents.into_iter().map(agent).collect(),
        ),
        // from a newer daemon; nothing here owns it yet
        wire::ReportBody::Unknown => Ok(()),
    }
}

fn check_protocol(version: u32) -> Result<()> {
    if version == 0 {
        return Err(Error::invalid("protocol", "is 0; versions start at 1"));
    }
    Ok(())
}

fn mode(mode: wire::Mode) -> tendhost_hosts::Mode {
    match mode {
        wire::Mode::Observe => tendhost_hosts::Mode::Observe,
        wire::Mode::Manage => tendhost_hosts::Mode::Manage,
    }
}

fn facts(facts: wire::Facts) -> tendhost_hosts::Facts {
    tendhost_hosts::Facts {
        hostname: facts.hostname,
        os: facts.os,
        os_version: facts.os_version,
        arch: facts.arch,
        cpu_count: facts.cpu_count,
        memory_bytes: facts.memory_bytes,
        daemon_version: facts.daemon_version,
    }
}

fn agent(report: wire::AgentReport) -> tendhost_agents::Found {
    tendhost_agents::Found {
        agent: report.agent,
        account_count: report.account_count,
        supports_usage: report.supports_usage,
        supports_limits: report.supports_limits,
        detail: report.detail.map(|detail| tendhost_agents::FoundDetail {
            accounts: detail
                .accounts
                .into_iter()
                .map(|a| tendhost_agents::Account {
                    dir: a.dir,
                    label: a.label,
                    plan: a.plan,
                })
                .collect(),
            usage: detail
                .usage
                .into_iter()
                .map(|u| tendhost_agents::UsageDay {
                    day: u.day,
                    model: u.model,
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cache_read_tokens: u.cache_read_tokens,
                    cache_write_tokens: u.cache_write_tokens,
                })
                .collect(),
            limits: detail.limits.map(|l| tendhost_agents::LimitReading {
                observed_at: l.observed_at,
                plan: l.plan,
                windows: l
                    .windows
                    .into_iter()
                    .map(|w| tendhost_agents::LimitWindow {
                        name: w.name,
                        used_ratio: w.used_ratio,
                        resets_at: w.resets_at,
                    })
                    .collect(),
            }),
        }),
    }
}
