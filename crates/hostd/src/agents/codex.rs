//! Codex: session rollouts under `sessions/`. A rollout carries the token
//! counts of every response and, with them, the account's limit windows
//! as the service last reported them.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use tendhost_protocol::{LimitReading, LimitWindow, UsageDay};

use super::{Tally, dirs};

#[derive(Deserialize)]
struct Line {
    timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    kind: Option<String>,
    payload: Option<Value>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    cache_write_input_tokens: u64,
    output_tokens: u64,
}

fn lines(accounts: &[PathBuf], since: DateTime<Utc>) -> impl Iterator<Item = (PathBuf, Vec<Line>)> {
    dirs::stores(accounts, "sessions")
        .into_iter()
        .flat_map(move |store| dirs::recent_logs(&store, since.into()))
        .filter_map(|path| {
            let file = std::fs::File::open(&path).ok()?;
            let parsed = BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter_map(|raw| serde_json::from_str::<Line>(&raw).ok())
                .collect();
            Some((path, parsed))
        })
}

/// Token totals per day and model from `since` on. The model of a
/// response is the one its turn was started with.
pub(super) fn usage(accounts: &[PathBuf], since: DateTime<Utc>) -> Vec<UsageDay> {
    let mut tally = Tally::default();
    let mut seen = HashSet::new();
    for (_, lines) in lines(accounts, since) {
        count(&lines, since, &mut seen, &mut tally);
    }
    tally.into_days()
}

fn count(lines: &[Line], since: DateTime<Utc>, seen: &mut HashSet<String>, tally: &mut Tally) {
    let mut model = String::from("unknown");
    for line in lines {
        let Some(payload) = &line.payload else {
            continue;
        };
        match line.kind.as_deref() {
            Some("turn_context") => {
                if let Some(name) = payload.get("model").and_then(Value::as_str) {
                    model = name.to_string();
                }
            }
            Some("token_usage_record") => {
                let (Some(at), Some(response)) = (
                    line.timestamp,
                    payload.get("response_id").and_then(Value::as_str),
                ) else {
                    continue;
                };
                let Some(usage) = payload
                    .get("usage")
                    .and_then(|u| serde_json::from_value::<Usage>(u.clone()).ok())
                else {
                    continue;
                };
                // the cached counts are part of the input count; when they
                // exceed it the format has changed and subtracting would
                // invent a number
                let nested = usage.cached_input_tokens + usage.cache_write_input_tokens;
                if at < since || nested > usage.input_tokens || !seen.insert(response.to_string()) {
                    continue;
                }
                let tokens = [
                    usage.input_tokens - nested,
                    usage.output_tokens,
                    usage.cached_input_tokens,
                    usage.cache_write_input_tokens,
                ];
                tally.add(at.date_naive(), &model, tokens);
            }
            _ => {}
        }
    }
}

/// The newest limit windows any session from `since` on recorded.
pub(super) fn limits(accounts: &[PathBuf], since: DateTime<Utc>) -> Option<LimitReading> {
    lines(accounts, since)
        .flat_map(|(_, lines)| lines)
        .filter_map(|line| reading(&line))
        .max_by_key(|reading| reading.observed_at)
}

fn reading(line: &Line) -> Option<LimitReading> {
    let payload = line.payload.as_ref()?;
    if line.kind.as_deref() != Some("event_msg") || payload.get("type")?.as_str()? != "token_count"
    {
        return None;
    }
    let limits = payload.get("rate_limits").filter(|v| v.is_object())?;
    let windows: Vec<LimitWindow> = ["primary", "secondary"]
        .iter()
        .filter_map(|slot| window(limits.get(*slot)?))
        .collect();
    if windows.is_empty() {
        return None;
    }
    Some(LimitReading {
        observed_at: line.timestamp?,
        plan: limits
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_string),
        windows,
    })
}

fn window(raw: &Value) -> Option<LimitWindow> {
    let used_percent = raw.get("used_percent")?.as_f64()?;
    let minutes = raw.get("window_minutes").and_then(Value::as_i64);
    Some(LimitWindow {
        name: minutes.map_or_else(|| "window".to_string(), span),
        used_ratio: (used_percent / 100.0) as f32,
        resets_at: raw
            .get("resets_at")
            .and_then(Value::as_i64)
            .and_then(|secs| Utc.timestamp_opt(secs, 0).single()),
    })
}

/// `5h` for 300 minutes, `7d` for 10080; the service's windows run a
/// minute short of the round figure at times, so the nearest unit wins.
fn span(minutes: i64) -> String {
    let hours = (minutes as f64 / 60.0).round() as i64;
    match hours {
        0 => format!("{minutes}m"),
        h if h % 24 == 0 => format!("{}d", h / 24),
        h => format!("{h}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[&str]) -> Vec<Line> {
        raw.iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    const CTX: &str =
        r#"{"timestamp":"2026-03-10T08:00:00Z","type":"turn_context","payload":{"model":"gpt-x"}}"#;

    fn record(at: &str, response: &str, input: u64, cached: u64, output: u64) -> String {
        format!(
            r#"{{"timestamp":"{at}","type":"token_usage_record","payload":{{"response_id":"{response}","usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"cache_write_input_tokens":0,"output_tokens":{output},"total_tokens":0}}}}}}"#
        )
    }

    fn tally(raw: &[&str]) -> Vec<UsageDay> {
        let since = Utc.with_ymd_and_hms(2026, 3, 9, 0, 0, 0).unwrap();
        let (mut seen, mut tally) = (HashSet::new(), Tally::default());
        count(&parse(raw), since, &mut seen, &mut tally);
        tally.into_days()
    }

    #[test]
    fn cached_input_is_taken_out_of_input_and_a_response_counts_once() {
        let (a, b) = (
            record("2026-03-10T08:00:05Z", "resp_1", 100, 60, 7),
            record("2026-03-10T08:00:09Z", "resp_2", 50, 0, 3),
        );
        let days = tally(&[CTX, &a, &a, &b]);
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].model, "gpt-x");
        assert_eq!(
            (
                days[0].input_tokens,
                days[0].cache_read_tokens,
                days[0].output_tokens
            ),
            (90, 60, 10)
        );
    }

    #[test]
    fn a_record_whose_cached_count_exceeds_its_input_is_left_out() {
        let bad = record("2026-03-10T08:00:05Z", "resp_1", 10, 60, 7);
        assert!(tally(&[CTX, &bad]).is_empty());
    }

    #[test]
    fn a_response_before_any_turn_context_is_filed_under_unknown() {
        let early = record("2026-03-10T08:00:05Z", "resp_1", 10, 0, 1);
        assert_eq!(tally(&[&early])[0].model, "unknown");
    }

    #[test]
    fn limit_windows_are_named_by_their_span_and_scaled_to_a_ratio() {
        let line = parse(&[
            r#"{"timestamp":"2026-03-10T08:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":42.5,"window_minutes":299,"resets_at":1773140400},"secondary":{"used_percent":3.0,"window_minutes":10080,"resets_at":null},"plan_type":"pro"}}}"#,
        ]);
        let reading = reading(&line[0]).unwrap();
        assert_eq!(reading.plan.as_deref(), Some("pro"));
        let got: Vec<_> = reading
            .windows
            .iter()
            .map(|w| (w.name.as_str(), w.used_ratio, w.resets_at.is_some()))
            .collect();
        assert_eq!(got, [("5h", 0.425, true), ("7d", 0.03, false)]);
    }

    #[test]
    fn a_count_without_limits_is_not_a_reading() {
        let lines = parse(&[
            r#"{"timestamp":"2026-03-10T08:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":null}}"#,
            r#"{"timestamp":"2026-03-10T08:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":null,"secondary":null}}}"#,
            r#"{"timestamp":"2026-03-10T08:00:00Z","type":"event_msg","payload":{"type":"task_started"}}"#,
        ]);
        assert!(lines.iter().all(|line| reading(line).is_none()));
    }

    #[test]
    fn spans_round_to_the_nearest_unit() {
        assert_eq!(
            [
                span(299),
                span(300),
                span(10080),
                span(1440),
                span(45),
                span(20)
            ],
            ["5h", "5h", "7d", "1d", "1h", "20m"]
        );
    }
}
