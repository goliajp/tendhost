use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tendhost_http::{ApiResult, Signed};

use crate::ports::ActorNames;
use crate::service::{Audit, Entry};

/// Routes, relative to the api root: `/audit`.
pub fn router(audit: Arc<Audit>, names: Arc<dyn ActorNames>) -> Router {
    Router::new()
        .route("/audit", get(entries))
        .with_state(Ctx { audit, names })
}

#[derive(Clone)]
struct Ctx {
    audit: Arc<Audit>,
    names: Arc<dyn ActorNames>,
}

#[derive(Deserialize)]
struct Page {
    /// a `seq`; entries below it are returned
    before: Option<i64>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

/// An entry with the present name of who acted, looked up when the log is
/// read: the entry itself keeps only the id, which never changes.
#[derive(Serialize)]
struct Line {
    #[serde(flatten)]
    entry: Entry,
    actor_name: Option<String>,
}

async fn entries(
    State(ctx): State<Ctx>,
    Signed(principal): Signed,
    Query(page): Query<Page>,
) -> ApiResult<Json<Vec<Line>>> {
    let entries = ctx.audit.entries(&principal, page.before, page.limit)?;
    let lines = entries
        .into_iter()
        .map(|entry| Line {
            actor_name: ctx.names.name(&entry.event.actor),
            entry,
        })
        .collect();
    Ok(Json(lines))
}
