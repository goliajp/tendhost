use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tendhost_kernel::{Error, Result, Timestamp};

/// The agents that can be tracked.
pub const KINDS: &[&str] = &["claude", "codex", "gemini", "kimi"];
pub const USAGE_KEPT_DAYS: i64 = 90;

/// An agent a host says it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub agent: String,
    pub account_count: u32,
    pub supports_usage: bool,
    pub supports_limits: bool,
    pub seen_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub dir: String,
    pub label: String,
    pub plan: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageDay {
    pub day: NaiveDate,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitReading {
    pub observed_at: Timestamp,
    pub plan: Option<String>,
    pub windows: Vec<LimitWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub name: String,
    pub used_ratio: f32,
    pub resets_at: Option<Timestamp>,
}

/// What a host reports about one agent.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub agent: String,
    pub account_count: u32,
    pub supports_usage: bool,
    pub supports_limits: bool,
    pub detail: Option<FoundDetail>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FoundDetail {
    pub accounts: Vec<Account>,
    pub usage: Vec<UsageDay>,
    pub limits: Option<LimitReading>,
}

/// What is kept about a tracked agent besides its usage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Detail {
    pub accounts: Vec<Account>,
    pub limits: Option<LimitReading>,
}

/// One agent on one host, as the api shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentView {
    pub host_id: String,
    #[serde(flatten)]
    pub installed: Installed,
    pub tracked: bool,
    /// empty until tracked and reported
    pub accounts: Vec<Account>,
    /// absent when not tracked, when the agent does not keep its limits
    /// locally, or before the first reading
    pub limits: Option<LimitReading>,
}

/// The agents to track, as stored: known kinds, sorted, without repeats.
pub(crate) fn normalize_tracked(raw: &[String]) -> Result<Vec<String>> {
    let mut agents: Vec<String> = raw.iter().map(|a| a.trim().to_lowercase()).collect();
    agents.sort();
    agents.dedup();
    if let Some(unknown) = agents.iter().find(|a| !KINDS.contains(&a.as_str())) {
        return Err(Error::invalid(
            "tracked",
            format!("{unknown} is not one of {}", KINDS.join(", ")),
        ));
    }
    Ok(agents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_agents_are_known_kinds_sorted_without_repeats() {
        let tracked =
            normalize_tracked(&[" Codex".into(), "claude".into(), "codex".into()]).unwrap();
        assert_eq!(tracked, ["claude", "codex"]);
        assert!(
            normalize_tracked(&["cursor".into()])
                .unwrap_err()
                .to_string()
                .contains("cursor")
        );
        assert!(normalize_tracked(&[]).unwrap().is_empty());
    }
}
