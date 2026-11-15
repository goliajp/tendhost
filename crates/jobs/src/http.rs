use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tendhost_http::{ApiResult, Signed};

use crate::model::{Job, NewJob, Run};
use crate::service::Jobs;

/// Routes, relative to the api root: `/jobs`, `/jobs/{id}`,
/// `/jobs/{id}/enable`, `/jobs/{id}/disable`, `/jobs/{id}/runs`.
pub fn router(jobs: Arc<Jobs>) -> Router {
    Router::new()
        .route("/jobs", get(list).post(create))
        .route("/jobs/{id}", get(one).delete(remove))
        .route("/jobs/{id}/enable", post(enable))
        .route("/jobs/{id}/disable", post(disable))
        .route("/jobs/{id}/runs", get(runs).post(request_run))
        .with_state(jobs)
}

#[derive(Deserialize)]
struct Filter {
    host: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

async fn list(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Query(filter): Query<Filter>,
) -> ApiResult<Json<Vec<Job>>> {
    Ok(Json(jobs.jobs(&principal, filter.host.as_deref())?))
}

async fn create(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Json(new): Json<NewJob>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    Ok((StatusCode::CREATED, Json(jobs.create(&principal, new)?)))
}

async fn one(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(jobs.job(&principal, &id)?))
}

async fn remove(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    jobs.remove(&principal, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn enable(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(jobs.set_enabled(&principal, &id, true)?))
}

async fn disable(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(jobs.set_enabled(&principal, &id, false)?))
}

async fn runs(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Vec<Run>>> {
    Ok(Json(jobs.runs(&principal, &id, page.limit.min(200))?))
}

async fn request_run(
    State(jobs): State<Arc<Jobs>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Run>)> {
    Ok((
        StatusCode::CREATED,
        Json(jobs.request_run(&principal, &id)?),
    ))
}
