use chrono::Duration;
use serde::{Deserialize, Serialize};
use tendhost_kernel::{Error, Result, Role, Timestamp};

pub const SESSION_LIFETIME: Duration = Duration::days(30);
pub const MIN_PASSWORD_CHARS: usize = 10;
/// This many wrong passwords for one name within [`FAILURE_WINDOW`] lock
/// sign-in for that name for [`LOCKOUT`].
pub const MAX_FAILURES: u32 = 5;
pub const FAILURE_WINDOW: Duration = Duration::minutes(15);
pub const LOCKOUT: Duration = Duration::minutes(15);
const MAX_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredUser {
    #[serde(flatten)]
    pub user: User,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Session {
    pub user_id: String,
    pub expires_at: Timestamp,
}

/// Wrong passwords recently tried for one name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Failures {
    pub count: u32,
    pub first_at: Timestamp,
    pub locked_until: Option<Timestamp>,
}

/// An api token as it is listed: everything but the secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub user_id: String,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredToken {
    #[serde(flatten)]
    pub token: Token,
    pub fingerprint: String,
}

/// A token at the one moment its secret is known.
#[derive(Debug, Clone, Serialize)]
pub struct NewToken {
    #[serde(flatten)]
    pub token: Token,
    pub secret: String,
}

/// The form a user name is stored and compared in.
pub(crate) fn normalize_name(raw: &str) -> Result<String> {
    let name = raw.trim().to_lowercase();
    if name.is_empty() {
        return Err(Error::invalid("name", "is empty"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(Error::invalid(
            "name",
            format!("is longer than {MAX_NAME_CHARS} characters"),
        ));
    }
    if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Error::invalid("name", "contains whitespace"));
    }
    Ok(name)
}

pub(crate) fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(Error::invalid(
            "password",
            format!("is shorter than {MIN_PASSWORD_CHARS} characters"),
        ));
    }
    Ok(())
}

pub(crate) fn check_label(field: &'static str, raw: &str) -> Result<String> {
    let label = raw.trim();
    if label.is_empty() {
        return Err(Error::invalid(field, "is empty"));
    }
    if label.chars().count() > MAX_NAME_CHARS {
        return Err(Error::invalid(
            field,
            format!("is longer than {MAX_NAME_CHARS} characters"),
        ));
    }
    Ok(label.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_lowercased() {
        assert_eq!(normalize_name("  Ada ").unwrap(), "ada");
    }

    #[test]
    fn empty_spaced_and_overlong_names_are_rejected() {
        assert!(normalize_name("   ").is_err());
        assert!(normalize_name("a b").is_err());
        assert!(normalize_name(&"a".repeat(65)).is_err());
        assert!(normalize_name(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn password_length_counts_characters_not_bytes() {
        assert!(check_password("123456789").is_err());
        assert!(check_password("1234567890").is_ok());
        assert!(check_password("密码密码密码密码密码").is_ok());
    }
}
