//! Claude Code: transcripts under `projects/`, account metadata in
//! `.claude.json`.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tendhost_protocol::UsageDay;

use super::{Labelled, Tally, dirs};

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

/// The address and plan Claude Code notes beside its settings. For the
/// default directory that file sits next to it, in the home.
pub(super) fn label(dir: &Path) -> Option<Labelled> {
    let beside = dir.parent().map(|home| home.join(".claude.json"));
    [Some(dir.join(".claude.json")), beside]
        .into_iter()
        .flatten()
        .find_map(|path| {
            let text = std::fs::read_to_string(path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&text).ok()?;
            let account = json.get("oauthAccount")?;
            Some(Labelled {
                label: account.get("emailAddress")?.as_str()?.to_string(),
                plan: account
                    .get("planDisplayName")
                    .and_then(|p| p.as_str())
                    .map(str::to_string),
            })
        })
}

/// Token totals per day and model from `since` on. One response is
/// written as several lines repeating the same counts; it is counted
/// once, by message id and request id.
pub(super) fn usage(accounts: &[PathBuf], since: DateTime<Utc>) -> Vec<UsageDay> {
    let mut tally = Tally::default();
    let mut seen = HashSet::new();
    for store in dirs::stores(accounts, "projects") {
        for path in dirs::recent_logs(&store, since.into()) {
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            for raw in BufReader::new(file).lines().map_while(Result::ok) {
                count(&raw, since, &mut seen, &mut tally);
            }
        }
    }
    tally.into_days()
}

fn count(raw: &str, since: DateTime<Utc>, seen: &mut HashSet<(String, String)>, tally: &mut Tally) {
    if !raw.contains("\"assistant\"") {
        return;
    }
    let Ok(line) = serde_json::from_str::<Line>(raw) else {
        return;
    };
    let (Some("assistant"), Some(at), Some(message)) =
        (line.kind.as_deref(), line.timestamp, line.message)
    else {
        return;
    };
    let (Some(id), Some(model), Some(usage)) = (message.id, message.model, message.usage) else {
        return;
    };
    // written by the client itself for errors and interruptions, not a response
    if at < since || model == "<synthetic>" {
        return;
    }
    if !seen.insert((id, line.request_id.unwrap_or_default())) {
        return;
    }
    let tokens = [
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_read_input_tokens,
        usage.cache_creation_input_tokens,
    ];
    tally.add(at.date_naive(), &model, tokens);
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn line(at: &str, id: &str, request: &str, model: &str, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{at}","requestId":"{request}","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":10,"output_tokens":{output},"cache_read_input_tokens":100,"cache_creation_input_tokens":7}}}}}}"#
        )
    }

    fn tally(lines: &[String]) -> Vec<UsageDay> {
        let since = Utc.with_ymd_and_hms(2026, 3, 9, 0, 0, 0).unwrap();
        let (mut seen, mut tally) = (HashSet::new(), Tally::default());
        lines
            .iter()
            .for_each(|raw| count(raw, since, &mut seen, &mut tally));
        tally.into_days()
    }

    #[test]
    fn a_response_written_as_several_lines_is_counted_once() {
        let days = tally(&[
            line("2026-03-10T08:00:00Z", "m1", "r1", "opus", 5),
            line("2026-03-10T08:00:00Z", "m1", "r1", "opus", 5),
            line("2026-03-10T09:00:00Z", "m2", "r2", "opus", 6),
        ]);
        assert_eq!(days.len(), 1);
        assert_eq!(
            (
                days[0].input_tokens,
                days[0].output_tokens,
                days[0].cache_read_tokens,
                days[0].cache_write_tokens
            ),
            (20, 11, 200, 14)
        );
    }

    #[test]
    fn totals_are_split_by_utc_day_and_model_and_older_lines_are_left_out() {
        let days = tally(&[
            line("2026-03-08T23:59:59Z", "m0", "r0", "opus", 99),
            line("2026-03-09T23:59:59Z", "m1", "r1", "opus", 1),
            line("2026-03-10T00:00:00Z", "m2", "r2", "opus", 2),
            line("2026-03-10T00:00:01Z", "m3", "r3", "haiku", 4),
        ]);
        let got: Vec<_> = days
            .iter()
            .map(|d| (d.day.to_string(), d.model.as_str(), d.output_tokens))
            .collect();
        assert_eq!(
            got,
            [
                ("2026-03-09".into(), "opus", 1),
                ("2026-03-10".into(), "haiku", 4),
                ("2026-03-10".into(), "opus", 2)
            ]
        );
    }

    #[test]
    fn lines_that_are_not_responses_are_skipped() {
        let days = tally(&[
            r#"{"type":"user","timestamp":"2026-03-10T08:00:00Z","message":{"content":"mentions \"assistant\""}}"#.into(),
            line("2026-03-10T08:00:00Z", "m1", "r1", "<synthetic>", 5),
            "not json at all \"assistant\"".into(),
            r#"{"type":"assistant","timestamp":"2026-03-10T08:00:00Z","message":{"id":"m9","model":"opus"}}"#.into(),
        ]);
        assert!(days.is_empty());
    }

    #[test]
    fn the_address_is_found_inside_a_profile_and_beside_the_default_directory() {
        let home = tempfile::tempdir().unwrap();
        let json = r#"{"oauthAccount":{"emailAddress":"ada@example.com"}}"#;
        let (default, profile) = (
            home.path().join(".claude"),
            home.path().join(".claude-profile-2"),
        );
        std::fs::create_dir_all(&default).unwrap();
        std::fs::create_dir_all(&profile).unwrap();
        assert!(label(&default).is_none());
        std::fs::write(home.path().join(".claude.json"), json).unwrap();
        assert_eq!(label(&default).unwrap().label, "ada@example.com");
        std::fs::write(profile.join(".claude.json"), json.replace("ada", "bob")).unwrap();
        assert_eq!(label(&profile).unwrap().label, "bob@example.com");
    }
}
