use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use tendhost_http::{ApiResult, Signed};
use tendhost_kernel::{Envelope, Timestamp};

use crate::service::Audit;

/// Routes, relative to the api root: `/audit`.
pub fn router(audit: Arc<Audit>) -> Router {
    Router::new()
        .route("/audit", get(entries))
        .with_state(audit)
}

#[derive(Deserialize)]
struct Page {
    before: Option<Timestamp>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

async fn entries(
    State(audit): State<Arc<Audit>>,
    Signed(principal): Signed,
    Query(page): Query<Page>,
) -> ApiResult<Json<Vec<Envelope>>> {
    Ok(Json(audit.entries(&principal, page.before, page.limit)?))
}
