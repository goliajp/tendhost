//! Finds the coding agents on this host and reads what they keep locally.
//!
//! Only files an agent writes for its own bookkeeping are read: session
//! logs and account metadata. Nothing here opens a credential file, a
//! keychain or a network connection.

mod claude;
mod codex;
mod dirs;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use tendhost_protocol::{AgentAccount, AgentDetail, AgentReport, LimitReading, UsageDay};

/// Usage is recounted for this many days back on every pass, today
/// included, so a day keeps being corrected until it is over.
pub const RECOUNT_DAYS: i64 = 2;
/// How far back usage is read when the server starts tracking an agent.
pub const BACKFILL_DAYS: i64 = 30;

/// Reads something from an agent's account directories, from a moment on.
type Reader<T> = fn(&[PathBuf], DateTime<Utc>) -> T;

struct Kind {
    name: &'static str,
    /// the directory an account lives in, under a home: `<base>` and
    /// `<base>-profile-<n>`
    base: &'static str,
    label: fn(&Path) -> Option<Labelled>,
    usage: Option<Reader<Vec<UsageDay>>>,
    limits: Option<Reader<Option<LimitReading>>>,
}

/// What an agent's own metadata says about an account.
pub(crate) struct Labelled {
    label: String,
    plan: Option<String>,
}

const KINDS: &[Kind] = &[
    Kind {
        name: "claude",
        base: ".claude",
        label: claude::label,
        usage: Some(claude::usage),
        limits: None,
    },
    Kind {
        name: "codex",
        base: ".codex",
        label: |_| None,
        usage: Some(codex::usage),
        limits: Some(codex::limits),
    },
    Kind {
        name: "gemini",
        base: ".gemini",
        label: dirs::gemini_label,
        usage: None,
        limits: None,
    },
    Kind {
        name: "kimi",
        base: ".kimi-code",
        label: |_| None,
        usage: None,
        limits: None,
    },
];

/// The homes whose agents this daemon reports: every user's when it runs
/// as root, its own otherwise.
pub fn homes(is_root: bool, own_home: Option<&str>) -> Vec<PathBuf> {
    if !is_root {
        return own_home.map(PathBuf::from).into_iter().collect();
    }
    let mut homes = vec![PathBuf::from("/root"), PathBuf::from("/var/root")];
    for parent in ["/home", "/Users"] {
        let Ok(entries) = std::fs::read_dir(parent) else {
            continue;
        };
        homes.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    homes.retain(|home| home.is_dir() && home.file_name().is_some_and(|n| n != "Shared"));
    homes.sort();
    homes
}

/// One report per agent found under `homes`. Detail is read only for the
/// agents in `tracked`, with usage for the last `days` days, today included.
pub fn collect(
    homes: &[PathBuf],
    tracked: &[String],
    now: DateTime<Utc>,
    days: i64,
) -> Vec<AgentReport> {
    let since = (now.date_naive() - Duration::days(days - 1))
        .and_hms_opt(0, 0, 0)
        .expect("midnight")
        .and_utc();
    let mut reports = Vec::new();
    for kind in KINDS {
        let accounts: Vec<PathBuf> = homes
            .iter()
            .flat_map(|home| dirs::account_dirs(home, kind.base))
            .collect();
        if accounts.is_empty() {
            continue;
        }
        let detail = tracked.iter().any(|t| t == kind.name).then(|| AgentDetail {
            accounts: accounts.iter().map(|dir| account(kind, dir)).collect(),
            usage: kind
                .usage
                .map(|read| read(&accounts, since))
                .unwrap_or_default(),
            limits: kind.limits.and_then(|read| read(&accounts, since)),
        });
        reports.push(AgentReport {
            agent: kind.name.to_string(),
            account_count: accounts.len() as u32,
            supports_usage: kind.usage.is_some(),
            supports_limits: kind.limits.is_some(),
            detail,
        });
    }
    reports
}

fn account(kind: &Kind, dir: &Path) -> AgentAccount {
    let labelled = (kind.label)(dir);
    let fallback = || {
        dir.file_name()
            .map(|n| n.to_string_lossy().trim_start_matches('.').to_string())
            .unwrap_or_default()
    };
    AgentAccount {
        dir: dir.display().to_string(),
        label: labelled
            .as_ref()
            .map(|l| l.label.clone())
            .unwrap_or_else(fallback),
        plan: labelled.and_then(|l| l.plan),
    }
}

/// Totals per day and model, built up line by line.
#[derive(Default)]
pub(crate) struct Tally(std::collections::BTreeMap<(NaiveDate, String), [u64; 4]>);

impl Tally {
    /// `tokens`: input, output, cache read, cache write.
    pub(crate) fn add(&mut self, day: NaiveDate, model: &str, tokens: [u64; 4]) {
        let total = self.0.entry((day, model.to_string())).or_default();
        total.iter_mut().zip(tokens).for_each(|(sum, n)| *sum += n);
    }

    pub(crate) fn into_days(self) -> Vec<UsageDay> {
        self.0
            .into_iter()
            .map(
                |((day, model), [input, output, cache_read, cache_write])| UsageDay {
                    day,
                    model,
                    input_tokens: input,
                    output_tokens: output,
                    cache_read_tokens: cache_read,
                    cache_write_tokens: cache_write,
                },
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn agents_are_found_by_their_directories_and_detail_is_read_only_when_tracked() {
        let home = tempfile::tempdir().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        write(
            &home.path().join(".claude/.claude.json"),
            r#"{"oauthAccount":{"emailAddress":"ada@example.com","planDisplayName":"Max"}}"#,
        );
        write(
            &home.path().join(".claude/projects/p/s.jsonl"),
            r#"{"type":"assistant","timestamp":"2026-03-10T08:00:00Z","requestId":"r1","message":{"id":"m1","model":"opus","usage":{"input_tokens":3,"output_tokens":5}}}"#,
        );
        std::fs::create_dir_all(home.path().join(".kimi-code")).unwrap();
        std::fs::create_dir_all(home.path().join(".kimi-code-profile-2")).unwrap();
        let homes = [home.path().to_path_buf()];

        let untracked = collect(&homes, &[], now, RECOUNT_DAYS);
        let names: Vec<_> = untracked
            .iter()
            .map(|r| (r.agent.as_str(), r.account_count))
            .collect();
        assert_eq!(names, [("claude", 1), ("kimi", 2)]);
        assert!(untracked.iter().all(|r| r.detail.is_none()));
        assert!(untracked[0].supports_usage && !untracked[0].supports_limits);
        assert!(!untracked[1].supports_usage);

        let tracked = collect(&homes, &["claude".into(), "kimi".into()], now, RECOUNT_DAYS);
        let claude = tracked[0].detail.as_ref().unwrap();
        assert_eq!(
            (
                claude.accounts[0].label.as_str(),
                claude.accounts[0].plan.as_deref()
            ),
            ("ada@example.com", Some("Max"))
        );
        assert_eq!(claude.usage.len(), 1);
        assert_eq!(
            (claude.usage[0].input_tokens, claude.usage[0].output_tokens),
            (3, 5)
        );
        let kimi = tracked[1].detail.as_ref().unwrap();
        let labels: Vec<_> = kimi.accounts.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["kimi-code", "kimi-code-profile-2"]);
        assert!(kimi.usage.is_empty() && kimi.limits.is_none());
    }

    #[test]
    fn how_far_back_usage_is_read_follows_the_days_asked() {
        let home = tempfile::tempdir().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let line = |day: u32, id: &str| {
            format!(
                r#"{{"type":"assistant","timestamp":"2026-03-{day:02}T08:00:00Z","requestId":"{id}","message":{{"id":"{id}","model":"opus","usage":{{"input_tokens":1,"output_tokens":1}}}}}}"#
            )
        };
        let log = [line(1, "a"), line(9, "b"), line(10, "c")].join("\n");
        write(&home.path().join(".claude/projects/p/s.jsonl"), &log);
        let days = |days| {
            let reports = collect(&[home.path().to_path_buf()], &["claude".into()], now, days);
            reports[0].detail.as_ref().unwrap().usage.len()
        };
        assert_eq!((days(RECOUNT_DAYS), days(BACKFILL_DAYS)), (2, 3));
    }

    #[test]
    fn a_host_without_agents_reports_none() {
        let home = tempfile::tempdir().unwrap();
        assert!(
            collect(
                &[home.path().to_path_buf()],
                &["claude".into()],
                Utc::now(),
                RECOUNT_DAYS
            )
            .is_empty()
        );
    }

    #[test]
    fn a_daemon_that_is_not_root_looks_only_in_its_own_home() {
        assert_eq!(
            homes(false, Some("/home/ada")),
            [PathBuf::from("/home/ada")]
        );
        assert!(homes(false, None).is_empty());
    }
}
