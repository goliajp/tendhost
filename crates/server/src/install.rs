//! The script a new host runs to install and enroll `tendhostd`.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

const TEMPLATE: &str = include_str!("../assets/install.sh");

/// The script with this server's address and version filled in. The
/// address is the one hosts will use, so it must be reachable from them.
pub(crate) fn script(server_url: &str) -> String {
    TEMPLATE
        .replace("__SERVER__", server_url)
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
}

/// The address a host should use for this server: the configured public
/// address, else the one this very request was sent to. A proxy that
/// terminates TLS says so in `X-Forwarded-Proto`.
pub(crate) fn address(public_url: Option<&str>, headers: &HeaderMap) -> Option<String> {
    if let Some(url) = public_url {
        return Some(url.to_string());
    }
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let host = header("x-forwarded-host").or_else(|| header("host"))?;
    let scheme = if header("x-forwarded-proto") == Some("https") {
        "https"
    } else {
        "http"
    };
    // the value goes into a shell script
    let plain = host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    plain.then(|| format!("{scheme}://{host}"))
}

pub(crate) async fn serve(public_url: Option<String>, headers: HeaderMap) -> Response {
    match address(public_url.as_deref(), &headers) {
        Some(server) => (
            [
                (CONTENT_TYPE, "text/x-shellscript; charset=utf-8"),
                (CACHE_CONTROL, "no-store"),
            ],
            script(&server),
        )
            .into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            "the request names no usable host; set TENDHOST_PUBLIC_URL\n",
        )
            .into_response(),
    }
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

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap()))
            .collect::<HeaderMap>()
    }

    #[test]
    fn the_configured_address_wins_and_otherwise_the_one_the_request_used() {
        let direct = headers(&[("host", "10.0.0.5:7080")]);
        assert_eq!(
            address(Some("https://tend.example.com"), &direct).unwrap(),
            "https://tend.example.com"
        );
        assert_eq!(address(None, &direct).unwrap(), "http://10.0.0.5:7080");
        let proxied = headers(&[
            ("host", "127.0.0.1:7080"),
            ("x-forwarded-host", "tend.example.com"),
            ("x-forwarded-proto", "https"),
        ]);
        assert_eq!(address(None, &proxied).unwrap(), "https://tend.example.com");
    }

    #[test]
    fn a_host_header_that_is_not_a_plain_address_is_refused() {
        assert_eq!(
            address(None, &headers(&[("host", "x\";rm -rf ~;\"")])),
            None
        );
        assert_eq!(address(None, &headers(&[("host", "a$(id)")])), None);
        assert_eq!(address(None, &headers(&[])), None);
    }
}
