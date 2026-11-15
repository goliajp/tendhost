//! Shared by every module's http adapter. Holds no routes of its own.

use std::sync::Arc;

use axum::Json;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tendhost_kernel::{Error, Principal, Result};

pub const SESSION_COOKIE: &str = "tendhost_session";

/// What a request presents to prove who sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Bearer(String),
    Session(String),
}

/// Turns a credential into a principal. Implemented by whichever module
/// owns identities; the adapters only know this trait.
pub trait Authenticator: Send + Sync {
    fn authenticate(&self, credential: &Credential) -> Result<Principal>;
}

/// The credential of a request: a bearer token wins over a session cookie.
pub fn credential(headers: &HeaderMap) -> Option<Credential> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| Credential::Bearer(v.trim().to_string()));
    bearer.or_else(|| {
        headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(name, _)| *name == SESSION_COOKIE)
            .map(|(_, value)| Credential::Session(value.to_string()))
    })
}

/// Middleware: resolves the request's credential and leaves the principal
/// for [`Signed`]. It rejects nothing; a route that needs a principal asks
/// for one.
pub async fn authenticate(
    State(authenticator): State<Arc<dyn Authenticator>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(credential) = credential(request.headers())
        && let Ok(principal) = authenticator.authenticate(&credential)
    {
        request.extensions_mut().insert(principal);
    }
    next.run(request).await
}

/// Extractor: the principal behind the request, or 401.
pub struct Signed(pub Principal);

impl<S: Send + Sync> FromRequestParts<S> for Signed {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> ApiResult<Self> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .map(Signed)
            .ok_or(ApiError(Error::Unauthenticated))
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;

/// A kernel error on its way out as `{"error": {"code", "message"}}`.
#[derive(Debug)]
pub struct ApiError(pub Error);

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        match &self.0 {
            Error::NotFound { .. } => (StatusCode::NOT_FOUND, "not_found", self.0.to_string()),
            Error::Conflict(_) => (StatusCode::CONFLICT, "conflict", self.0.to_string()),
            Error::Invalid { .. } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid",
                self.0.to_string(),
            ),
            Error::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                self.0.to_string(),
            ),
            Error::Forbidden => (StatusCode::FORBIDDEN, "forbidden", self.0.to_string()),
            // the cause goes to the log, not to the caller
            Error::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "internal error".to_string(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Error::Internal(cause) = &self.0 {
            tracing::error!(%cause, "request failed");
        }
        let (status, code, message) = self.parts();
        (
            status,
            Json(serde_json::json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_static(value));
        }
        map
    }

    #[test]
    fn the_session_cookie_is_found_among_others() {
        let h = headers(&[("cookie", "theme=dark; tendhost_session=abc; x=1")]);
        assert_eq!(credential(&h), Some(Credential::Session("abc".into())));
    }

    #[test]
    fn a_cookie_whose_name_only_ends_like_ours_is_ignored() {
        let h = headers(&[("cookie", "not_tendhost_session=abc")]);
        assert_eq!(credential(&h), None);
    }

    #[test]
    fn a_bearer_token_wins_over_the_cookie() {
        let h = headers(&[
            ("cookie", "tendhost_session=abc"),
            ("authorization", "Bearer tok"),
        ]);
        assert_eq!(credential(&h), Some(Credential::Bearer("tok".into())));
    }

    #[test]
    fn other_authorization_schemes_are_not_credentials() {
        assert_eq!(
            credential(&headers(&[("authorization", "Basic abc")])),
            None
        );
        assert_eq!(credential(&headers(&[])), None);
    }

    #[test]
    fn an_internal_error_does_not_leak_its_cause() {
        let (status, code, message) = ApiError(Error::internal("disk /var/lib full")).parts();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(code, "internal");
        assert!(!message.contains("disk"));
    }
}
