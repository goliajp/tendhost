//! What `tendhostd` and the server say to each other. Types only: no
//! transport, no behaviour.
//!
//! The daemon starts every exchange. It enrolls once, then syncs on a
//! timer: it sends reports and receives directives.
//!
//! Both sides tolerate kinds they do not know (`Unknown`), so a newer
//! daemon can talk to an older server and the reverse.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const ENROLL_PATH: &str = "/hostd/v1/enroll";
pub const SYNC_PATH: &str = "/hostd/v1/sync";

/// What the server may do on a host. Chosen on the host at enrollment and
/// kept in the daemon's own configuration; the server cannot raise it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// report only
    Observe,
    /// report, and run the commands the server sends
    Manage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrollRequest {
    pub protocol: u32,
    /// the one-time enrollment token
    pub token: String,
    pub name: String,
    pub mode: Mode,
    pub facts: Facts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrollResponse {
    pub host_id: String,
    /// sent as a bearer token on every sync; shown this once
    pub credential: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRequest {
    pub protocol: u32,
    pub reports: Vec<Report>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResponse {
    pub directives: Vec<Directive>,
    pub next_sync_secs: u32,
}

/// Something the host observed, with the time it observed it. Sending the
/// same report twice has the effect of sending it once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub body: ReportBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReportBody {
    Facts(Facts),
    Metrics(Metrics),
    #[serde(other)]
    Unknown,
}

/// What rarely changes about a host.
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

/// One measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub load1: f32,
    pub memory_used_bytes: u64,
    pub uptime_secs: u64,
    pub disks: Vec<Disk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Directive {
    /// version 1 defines no directive yet; the variant keeps the slot so a
    /// daemon ignores the ones a later server sends
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_report_is_flat_json_tagged_by_kind() {
        let report = Report {
            at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            body: ReportBody::Metrics(Metrics {
                load1: 0.5,
                memory_used_bytes: 1,
                uptime_secs: 2,
                disks: vec![Disk {
                    mount: "/".into(),
                    total_bytes: 10,
                    used_bytes: 4,
                }],
            }),
        };
        let wire = serde_json::to_value(&report).unwrap();
        assert_eq!(wire["kind"], "metrics");
        assert_eq!(wire["at"], "2026-01-01T00:00:00Z");
        assert_eq!(wire["disks"][0]["mount"], "/");
        assert_eq!(serde_json::from_value::<Report>(wire).unwrap(), report);
    }

    #[test]
    fn kinds_added_later_decode_as_unknown_instead_of_failing() {
        let report: Report = serde_json::from_value(
            json!({ "at": "2026-01-01T00:00:00Z", "kind": "gpu", "temp": 70 }),
        )
        .unwrap();
        assert_eq!(report.body, ReportBody::Unknown);
        let directive: Directive =
            serde_json::from_value(json!({ "kind": "reboot", "in": 5 })).unwrap();
        assert_eq!(directive, Directive::Unknown);
    }

    #[test]
    fn mode_is_spelled_in_snake_case() {
        assert_eq!(serde_json::to_value(Mode::Observe).unwrap(), "observe");
        assert_eq!(
            serde_json::from_value::<Mode>(json!("manage")).unwrap(),
            Mode::Manage
        );
    }
}
