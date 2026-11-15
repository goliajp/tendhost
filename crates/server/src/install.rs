//! The script a new host runs to install and enroll `tendhostd`.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;

const TEMPLATE: &str = include_str!("../assets/install.sh");

/// The script with this server's address and version filled in. The
/// address is the one hosts will use, so it must be reachable from them.
pub(crate) fn script(server_url: &str) -> String {
    TEMPLATE
        .replace("__SERVER__", server_url)
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
}

pub(crate) async fn serve(server_url: String) -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, "text/x-shellscript; charset=utf-8"),
            (CACHE_CONTROL, "no-store"),
        ],
        script(&server_url),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_names_this_server_and_this_version_and_keeps_no_placeholder() {
        let script = script("https://tend.example.com");
        assert!(script.contains("SERVER=\"https://tend.example.com\""));
        assert!(script.contains(&format!("VERSION=\"{}\"", env!("CARGO_PKG_VERSION"))));
        assert!(!script.contains("__"));
        assert!(script.starts_with("#!/bin/sh\n"));
    }
}
