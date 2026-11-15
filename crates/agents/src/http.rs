use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tendhost_http::{ApiResult, Signed};

use crate::model::{AgentView, UsageDay};
use crate::service::Agents;

/// Routes, relative to the api root: `/agents`, `/hosts/{id}/agents`,
/// `/hosts/{id}/agents/{agent}/usage`.
pub fn router(agents: Arc<Agents>) -> Router {
    Router::new()
        .route("/agents", get(overview))
        .route("/hosts/{id}/agents", put(set_tracked))
        .route("/hosts/{id}/agents/{agent}/usage", get(usage))
        .with_state(agents)
}

#[derive(Serialize, Deserialize)]
struct Tracking {
    tracked: Vec<String>,
}

#[derive(Deserialize)]
struct Span {
    #[serde(default = "default_days")]
    days: u32,
}

fn default_days() -> u32 {
    30
}

async fn overview(
    State(agents): State<Arc<Agents>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<AgentView>>> {
    Ok(Json(agents.overview(&principal)?))
}

async fn set_tracked(
    State(agents): State<Arc<Agents>>,
    Signed(principal): Signed,
    Path(host_id): Path<String>,
    Json(tracking): Json<Tracking>,
) -> ApiResult<Json<Tracking>> {
    Ok(Json(Tracking {
        tracked: agents.set_tracked(&principal, &host_id, &tracking.tracked)?,
    }))
}

async fn usage(
    State(agents): State<Arc<Agents>>,
    Signed(principal): Signed,
    Path((host_id, agent)): Path<(String, String)>,
    Query(span): Query<Span>,
) -> ApiResult<Json<Vec<UsageDay>>> {
    Ok(Json(agents.usage(&principal, &host_id, &agent, span.days)?))
}
