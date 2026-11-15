pub type Result<T> = std::result::Result<T, Error>;

/// Every failure a use case can report. Inbound adapters map the variant to
/// their own vocabulary (an http status, an exit code); the message is safe
/// to show to the caller except for `Internal`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{kind} not found")]
    NotFound { kind: &'static str },
    #[error("{0}")]
    Conflict(String),
    #[error("{field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("not signed in")]
    Unauthenticated,
    #[error("not allowed")]
    Forbidden,
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            reason: reason.into(),
        }
    }

    pub fn internal(source: impl std::fmt::Display) -> Self {
        Self::Internal(source.to_string())
    }
}
