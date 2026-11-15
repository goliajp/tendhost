use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use tendhost_http::{ApiResult, Signed};

use crate::model::{ChannelView, Delivery, NewChannel};
use crate::service::Notify;

/// Routes, relative to the api root: `/channels`, `/channels/{id}`,
/// `/channels/{id}/test`, `/deliveries`.
pub fn router(notify: Arc<Notify>) -> Router {
    Router::new()
        .route("/channels", get(list).post(create))
        .route("/channels/{id}", delete(remove))
        .route("/channels/{id}/test", post(test))
        .route("/deliveries", get(deliveries))
        .with_state(notify)
}

async fn list(
    State(notify): State<Arc<Notify>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<ChannelView>>> {
    Ok(Json(notify.channels(&principal)?))
}

async fn create(
    State(notify): State<Arc<Notify>>,
    Signed(principal): Signed,
    Json(new): Json<NewChannel>,
) -> ApiResult<(StatusCode, Json<ChannelView>)> {
    Ok((StatusCode::CREATED, Json(notify.create(&principal, new)?)))
}

async fn remove(
    State(notify): State<Arc<Notify>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    notify.remove(&principal, &id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test(
    State(notify): State<Arc<Notify>>,
    Signed(principal): Signed,
    Path(id): Path<String>,
) -> ApiResult<Json<Delivery>> {
    Ok(Json(notify.test(&principal, &id).await?))
}

async fn deliveries(
    State(notify): State<Arc<Notify>>,
    Signed(principal): Signed,
) -> ApiResult<Json<Vec<Delivery>>> {
    Ok(Json(notify.deliveries(&principal, 100)?))
}
