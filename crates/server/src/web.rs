//! The dashboard. With the `web` feature its built files are part of the
//! binary; without it the server is the api alone and says so.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

#[cfg(feature = "web")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../web/dist"]
struct Dist;

/// Everything no route claimed. An unknown address under the api or the
/// host protocol is an error in the api's own form; anything else belongs
/// to the dashboard.
pub(crate) async fn fallback(uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") || path.starts_with("/hostd/") {
        let body = r#"{"error":{"code":"not_found","message":"no such endpoint"}}"#;
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response();
    }
    serve(uri).await
}

/// Serves a built file, or the app's entry page for any other path, so
/// that an address inside the app can be opened directly.
#[cfg(feature = "web")]
async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, name) = match Dist::get(path) {
        Some(file) if !path.is_empty() => (file, path),
        _ => match Dist::get("index.html") {
            Some(index) => (index, "index.html"),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "the dashboard was built without an index.html",
                )
                    .into_response();
            }
        },
    };
    // built assets carry a content hash in their name; the entry page does not
    let cache = if name.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype()),
            (header::CACHE_CONTROL, cache),
        ],
        file.data,
    )
        .into_response()
}

#[cfg(not(feature = "web"))]
async fn serve(_: Uri) -> Response {
    let page = "This tendhost server was built without the dashboard. The api is under /api/v1.\n";
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        page,
    )
        .into_response()
}
