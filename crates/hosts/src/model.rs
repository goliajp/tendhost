use chrono::Duration;
use serde::{Deserialize, Serialize};
use tendhost_kernel::{Actor, Error, Result, Timestamp};

pub const ENROLLMENT_LIFETIME: Duration = Duration::minutes(15);
const MAX_NAME_CHARS: usize = 63;
const MAX_LABEL_CHARS: usize = 32;
const MAX_LABELS: usize = 16;

/// What the server may do on a host; decided on the host when it enrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Observe,
    Manage,
}

/// What a host says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Facts {
    pub hostname: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_count: u32,
    pub memory_bytes: u64,
    pub daemon_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub id: String,
    pub name: String,
    pub mode: Mode,
    pub labels: Vec<String>,
    pub enrolled_at: Timestamp,
    /// the enrollment that admitted it, as [`NewEnrollment::id`] named it
    pub enrollment_id: String,
    pub facts: Facts,
    /// when the host last reported `facts`
    pub facts_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Enrollment {
    pub id: String,
    pub opened_by: Actor,
    pub expires_at: Timestamp,
}

/// An enrollment at the one moment its token is known.
#[derive(Debug, Clone, Serialize)]
pub struct NewEnrollment {
    /// carried by the host this enrollment admits, so whoever opened it
    /// can recognise that host
    pub id: String,
    pub token: String,
    pub expires_at: Timestamp,
}

/// A host name as stored: lowercase, made of letters, digits, `-`, `.`
/// and `_`. Anything else in what the host or a person supplied becomes
/// `-`, so a hostname such as `Ada's MacBook` still enrolls.
pub(crate) fn normalize_name(raw: &str) -> Result<String> {
    let name: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches('-').to_string();
    if name.is_empty() {
        return Err(Error::invalid("name", "has no letter or digit"));
    }
    if name.len() > MAX_NAME_CHARS {
        return Err(Error::invalid(
            "name",
            format!("is longer than {MAX_NAME_CHARS} characters"),
        ));
    }
    Ok(name)
}

/// Labels as stored: trimmed, without duplicates, sorted.
pub(crate) fn normalize_labels(raw: &[String]) -> Result<Vec<String>> {
    let mut labels: Vec<String> = raw.iter().map(|l| l.trim().to_string()).collect();
    labels.sort();
    labels.dedup();
    if labels.iter().any(|l| l.is_empty()) {
        return Err(Error::invalid("labels", "contains an empty label"));
    }
    if labels.iter().any(|l| l.chars().count() > MAX_LABEL_CHARS) {
        return Err(Error::invalid(
            "labels",
            format!("a label is longer than {MAX_LABEL_CHARS} characters"),
        ));
    }
    if labels.len() > MAX_LABELS {
        return Err(Error::invalid(
            "labels",
            format!("more than {MAX_LABELS} labels"),
        ));
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hostname_with_spaces_and_punctuation_becomes_a_usable_name() {
        assert_eq!(normalize_name(" Ada's MacBook ").unwrap(), "ada-s-macbook");
        assert_eq!(normalize_name("db-01.internal").unwrap(), "db-01.internal");
    }

    #[test]
    fn a_name_with_nothing_usable_or_too_long_is_rejected() {
        assert!(normalize_name(" !!! ").is_err());
        assert!(normalize_name(&"a".repeat(64)).is_err());
        assert!(normalize_name(&"a".repeat(63)).is_ok());
    }

    #[test]
    fn labels_are_trimmed_deduplicated_and_sorted() {
        let labels = normalize_labels(&[" web ".into(), "db".into(), "web".into()]).unwrap();
        assert_eq!(labels, ["db", "web"]);
        assert!(normalize_labels(&["  ".into()]).is_err());
        assert!(normalize_labels(&["x".repeat(33)]).is_err());
        assert!(normalize_labels(&(0..17).map(|n| n.to_string()).collect::<Vec<_>>()).is_err());
    }
}
