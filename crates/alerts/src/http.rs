use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use tendhost_http::{ApiResult, Signed};

use crate::model::{Alert, Settings};
use crate::service::Alerts;

/// Routes, relative to the api root: `/alerts`, `/alerts/settings`.
pub fn router(alerts: Arc<Alerts>) -> Router {
    Router::new()
        .route("/alerts", get(list))
        .route("/alerts/settings", get(settings).put(set_settings))
        .with_state(alerts)
}

#[derive(Deserialize)]
struct Filter {
    /// `open` (default) or `all`
    state: Option<String>,
}

async fn list(
    State(alerts): State<Arc<Alerts>>,
    Signed(principal): Signed,
    Query(filter): Query<Filter>,
) -> ApiResult<Json<Vec<Alert>>> {
    Ok(Json(match filter.state.as_deref() {
        Some("all") => alerts.history(&principal, 200)?,
        _ => alerts.open(&principal)?,
    }))
}

async fn settings(
    State(alerts): State<Arc<Alerts>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Settings>> {
    Ok(Json(alerts.settings(&principal)?))
}

async fn set_settings(
    State(alerts): State<Arc<Alerts>>,
    Signed(principal): Signed,
    Json(settings): Json<Settings>,
) -> ApiResult<Json<Settings>> {
    Ok(Json(alerts.set_settings(&principal, settings)?))
}
