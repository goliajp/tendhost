use serde::{Deserialize, Serialize};
use tendhost_kernel::{Error, Result, Timestamp};

pub const HISTORY_KEPT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    HostOffline,
    DiskUsage,
    AgentLimit,
    JobFailed,
}

/// Which conditions raise an alert, and from what level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub host_offline: bool,
    /// a disk at or above this share of its capacity; absent turns it off
    pub disk_usage: Option<f32>,
    /// an agent's limit window at or above this share; absent turns it off
    pub agent_limit: Option<f32>,
    pub job_failed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host_offline: true,
            disk_usage: Some(0.9),
            agent_limit: Some(0.9),
            job_failed: true,
        }
    }
}

impl Settings {
    pub(crate) fn validated(self) -> Result<Self> {
        for (field, threshold) in [
            ("disk_usage", self.disk_usage),
            ("agent_limit", self.agent_limit),
        ] {
            if threshold.is_some_and(|t| !(0.01..=1.0).contains(&t)) {
                return Err(Error::invalid(field, "must be between 0.01 and 1"));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    /// names the condition; at most one alert per key is open
    pub key: String,
    pub kind: Kind,
    pub host_id: String,
    /// one sentence, e.g. `db-01 is offline`
    pub summary: String,
    pub detail: String,
    pub opened_at: Timestamp,
    pub resolved_at: Option<Timestamp>,
}

pub(crate) fn percent(ratio: f32) -> String {
    format!("{}%", (ratio * 100.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_must_be_a_share() {
        assert!(Settings::default().validated().is_ok());
        assert!(
            Settings {
                disk_usage: None,
                ..Settings::default()
            }
            .validated()
            .is_ok()
        );
        assert!(
            Settings {
                disk_usage: Some(1.5),
                ..Settings::default()
            }
            .validated()
            .is_err()
        );
        assert!(
            Settings {
                agent_limit: Some(0.0),
                ..Settings::default()
            }
            .validated()
            .is_err()
        );
    }

    #[test]
    fn a_ratio_reads_as_a_whole_percent() {
        assert_eq!(
            [percent(0.905), percent(1.0), percent(0.004)],
            ["91%", "100%", "0%"]
        );
    }
}
