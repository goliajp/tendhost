use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Duration;
use serde::Deserialize;
use tendhost_http::{ApiResult, Signed};
use tendhost_kernel::{Clock, Timestamp};

use crate::model::{Presence, Sample};
use crate::service::Telemetry;

#[derive(Clone)]
struct Ctx {
    telemetry: Arc<Telemetry>,
    clock: Arc<dyn Clock>,
}

/// Routes, relative to the api root: `/presence`, `/hosts/{id}/samples`.
pub fn router(telemetry: Arc<Telemetry>, clock: Arc<dyn Clock>) -> Router {
    Router::new()
        .route("/presence", get(presence))
        .route("/hosts/{id}/samples", get(samples))
        .with_state(Ctx { telemetry, clock })
}

#[derive(Deserialize)]
struct Window {
    /// default: one hour ago
    since: Option<Timestamp>,
}

async fn presence(
    State(ctx): State<Ctx>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<Presence>>> {
    Ok(Json(ctx.telemetry.presence(&principal)?))
}

async fn samples(
    State(ctx): State<Ctx>,
    Signed(principal): Signed,
    Path(id): Path<String>,
    Query(window): Query<Window>,
) -> ApiResult<Json<Vec<Sample>>> {
    let since = window
        .since
        .unwrap_or_else(|| ctx.clock.now() - Duration::hours(1));
    Ok(Json(ctx.telemetry.samples(&principal, &id, since)?))
}
