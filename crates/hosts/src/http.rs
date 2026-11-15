use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tendhost_http::{ApiResult, Signed};

use crate::model::{Host, NewEnrollment};
use crate::service::Hosts;

/// Routes, relative to the api root: `/hosts`, `/hosts/{id}`,
/// `/enrollments`.
pub fn router(hosts: Arc<Hosts>) -> Router {
    Router::new()
        .route("/hosts", get(list))
        .route("/hosts/{id}", get(one).patch(update).delete(remove))
        .route("/enrollments", post(open_enrollment))
        .with_state(hosts)
}

#[derive(Deserialize)]
struct Changes {
    name: Option<String>,
    labels: Option<Vec<String>>,
}

async fn list(
    State(hosts): State<Arc<Hosts>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<Host>>> {
    Ok(Json(hosts.hosts(&principal)?))
}

async fn one(
    State(hosts): State<Arc<Hosts>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<Json<Host>> {
    Ok(Json(hosts.host(&principal, &id)?))
}

async fn update(
    State(hosts): State<Arc<Hosts>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
    Json(changes): Json<Changes>,
) -> ApiResult<Json<Host>> {
    Ok(Json(hosts.update(
        &principal,
        &id,
        changes.name.as_deref(),
        changes.labels.as_deref(),
    )?))
}

async fn remove(
    State(hosts): State<Arc<Hosts>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    hosts.remove(&principal, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn open_enrollment(
    State(hosts): State<Arc<Hosts>>,
    Signed(principal): Signed,
) -> ApiResult<(StatusCode, Json<NewEnrollment>)> {
    Ok((
        StatusCode::CREATED,
        Json(hosts.open_enrollment(&principal)?),
    ))
}
