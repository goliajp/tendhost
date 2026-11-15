use std::str::FromStr;

use chrono::Duration;
use croner::Cron;
use serde::{Deserialize, Serialize};
use tendhost_kernel::{Actor, Error, Result, Timestamp};

/// A run no host collected within this long is recorded as missed.
pub const COLLECT_WITHIN: Duration = Duration::hours(6);
/// A started run whose end was never reported is given this long past its
/// timeout before it is recorded as lost.
pub const REPORT_GRACE: Duration = Duration::minutes(5);
pub const RUNS_KEPT_PER_JOB: usize = 200;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_COMMAND_BYTES: usize = 8 * 1024;
const MAX_TIMEOUT_SECS: u32 = 24 * 3600;
const MAX_NAME_CHARS: usize = 64;

/// When a job runs. Times are UTC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    /// only when someone asks
    Manual,
    Once {
        at: Timestamp,
    },
    /// five fields: minute hour day-of-month month day-of-week
    Cron {
        expr: String,
    },
}

impl Schedule {
    /// The first time strictly after `after` the schedule asks for a run.
    pub(crate) fn next_after(&self, after: Timestamp) -> Result<Option<Timestamp>> {
        match self {
            Schedule::Manual => Ok(None),
            Schedule::Once { at } => Ok(Some(*at).filter(|at| *at > after)),
            Schedule::Cron { expr } => Ok(Some(
                parse_cron(expr)?
                    .find_next_occurrence(&after, false)
                    .map_err(|e| {
                        Error::invalid("schedule", format!("{expr} never matches: {e}"))
                    })?,
            )),
        }
    }
}

fn parse_cron(expr: &str) -> Result<Cron> {
    if expr.split_whitespace().count() != 5 {
        return Err(Error::invalid(
            "schedule",
            "a cron expression has five fields: minute hour day-of-month month day-of-week",
        ));
    }
    Cron::from_str(expr).map_err(|e| Error::invalid("schedule", format!("{expr}: {e}")))
}

/// What a caller supplies to create a job.
#[derive(Debug, Clone, Deserialize)]
pub struct NewJob {
    pub name: String,
    pub host_id: String,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    pub schedule: Schedule,
}

fn default_timeout() -> u32 {
    3600
}

impl NewJob {
    pub(crate) fn validated(mut self, now: Timestamp) -> Result<Self> {
        self.name = self.name.trim().to_string();
        if self.name.is_empty() || self.name.chars().count() > MAX_NAME_CHARS {
            return Err(Error::invalid(
                "name",
                format!("must be 1 to {MAX_NAME_CHARS} characters"),
            ));
        }
        if self.command.trim().is_empty() {
            return Err(Error::invalid("command", "is empty"));
        }
        if self.command.len() > MAX_COMMAND_BYTES {
            return Err(Error::invalid(
                "command",
                format!("is longer than {MAX_COMMAND_BYTES} bytes"),
            ));
        }
        if self.timeout_secs == 0 || self.timeout_secs > MAX_TIMEOUT_SECS {
            return Err(Error::invalid(
                "timeout_secs",
                format!("must be 1 to {MAX_TIMEOUT_SECS}"),
            ));
        }
        match &self.schedule {
            Schedule::Once { at } if *at <= now => {
                return Err(Error::invalid("schedule", "the time is not in the future"));
            }
            Schedule::Cron { expr } => {
                parse_cron(expr)?;
            }
            _ => {}
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub host_id: String,
    pub command: String,
    pub timeout_secs: u32,
    pub schedule: Schedule,
    pub enabled: bool,
    pub created_by: Actor,
    pub created_at: Timestamp,
    /// when the schedule next asks for a run; absent for a manual job, a
    /// disabled one, and a one-time job that has run
    pub next_run_at: Option<Timestamp>,
    /// how the newest finished run ended, and when; absent before any
    #[serde(default)]
    pub last_run: Option<LastRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastRun {
    pub state: RunState,
    pub finished_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// waiting for the host to collect it
    Pending,
    /// handed to the host; no result yet
    Running,
    Succeeded,
    /// the command exited non-zero, or the host refused to run it
    Failed,
    /// stopped by the host at the job's timeout
    TimedOut,
    /// the host did not collect it in time
    Missed,
    /// handed to the host, which never reported how it ended
    Lost,
}

impl RunState {
    pub fn is_over(self) -> bool {
        !matches!(self, RunState::Pending | RunState::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub job_id: String,
    pub host_id: String,
    pub state: RunState,
    /// when it was asked for
    pub due_at: Timestamp,
    /// who asked: a person, or the system for the schedule
    pub requested_by: Actor,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub exit_code: Option<i32>,
    /// the end of what the command printed, at most [`MAX_OUTPUT_BYTES`]
    pub output: String,
}

/// A run handed to its host: what the host needs to execute it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub run_id: String,
    pub command: String,
    pub timeout_secs: u32,
}

/// How a host says a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub run_id: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output: String,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
}

impl Outcome {
    pub(crate) fn state(&self) -> RunState {
        match (self.timed_out, self.exit_code) {
            (true, _) => RunState::TimedOut,
            (false, Some(0)) => RunState::Succeeded,
            (false, _) => RunState::Failed,
        }
    }
}

/// The last [`MAX_OUTPUT_BYTES`] of `output`, cut at a character boundary.
pub(crate) fn tail(output: &str) -> String {
    if output.len() <= MAX_OUTPUT_BYTES {
        return output.to_string();
    }
    let mut start = output.len() - MAX_OUTPUT_BYTES;
    while !output.is_char_boundary(start) {
        start += 1;
    }
    output[start..].to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn at(h: u32, m: u32) -> Timestamp {
        Utc.with_ymd_and_hms(2026, 1, 31, h, m, 0).unwrap()
    }

    fn new_job(schedule: Schedule) -> NewJob {
        NewJob {
            name: " backup ".into(),
            host_id: "host_1".into(),
            command: "true".into(),
            timeout_secs: 60,
            schedule,
        }
    }

    #[test]
    fn cron_finds_the_next_match_strictly_after_and_across_a_month_end() {
        let daily = Schedule::Cron {
            expr: "30 3 * * *".into(),
        };
        assert_eq!(daily.next_after(at(3, 29)).unwrap(), Some(at(3, 30)));
        let next_day = Utc.with_ymd_and_hms(2026, 2, 1, 3, 30, 0).unwrap();
        assert_eq!(daily.next_after(at(3, 30)).unwrap(), Some(next_day));
    }

    #[test]
    fn once_is_due_only_while_it_is_still_ahead_and_manual_never() {
        let once = Schedule::Once { at: at(12, 0) };
        assert_eq!(once.next_after(at(11, 59)).unwrap(), Some(at(12, 0)));
        assert_eq!(once.next_after(at(12, 0)).unwrap(), None);
        assert_eq!(Schedule::Manual.next_after(at(0, 0)).unwrap(), None);
    }

    #[test]
    fn a_cron_expression_needs_exactly_five_valid_fields() {
        for bad in ["* * * *", "0 * * * * *", "61 * * * *", "nonsense"] {
            let job = new_job(Schedule::Cron { expr: bad.into() });
            assert!(
                matches!(
                    job.validated(at(0, 0)),
                    Err(Error::Invalid {
                        field: "schedule",
                        ..
                    })
                ),
                "{bad}"
            );
        }
        assert!(
            new_job(Schedule::Cron {
                expr: "*/5 * * * 1-5".into()
            })
            .validated(at(0, 0))
            .is_ok()
        );
    }

    #[test]
    fn a_new_job_is_checked_field_by_field() {
        let ok = new_job(Schedule::Manual);
        assert_eq!(ok.clone().validated(at(0, 0)).unwrap().name, "backup");
        let field = |job: NewJob| match job.validated(at(0, 0)) {
            Err(Error::Invalid { field, .. }) => field,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            field(NewJob {
                name: " ".into(),
                ..ok.clone()
            }),
            "name"
        );
        assert_eq!(
            field(NewJob {
                command: "  ".into(),
                ..ok.clone()
            }),
            "command"
        );
        assert_eq!(
            field(NewJob {
                command: "x".repeat(8193),
                ..ok.clone()
            }),
            "command"
        );
        assert_eq!(
            field(NewJob {
                timeout_secs: 0,
                ..ok.clone()
            }),
            "timeout_secs"
        );
        assert_eq!(
            field(NewJob {
                timeout_secs: 86401,
                ..ok.clone()
            }),
            "timeout_secs"
        );
        assert_eq!(field(new_job(Schedule::Once { at: at(0, 0) })), "schedule");
    }

    #[test]
    fn an_outcome_maps_to_one_state() {
        let outcome = |exit_code, timed_out| Outcome {
            run_id: "run_1".into(),
            exit_code,
            timed_out,
            output: String::new(),
            started_at: at(0, 0),
            finished_at: at(0, 1),
        };
        assert_eq!(outcome(Some(0), false).state(), RunState::Succeeded);
        assert_eq!(outcome(Some(3), false).state(), RunState::Failed);
        assert_eq!(outcome(None, false).state(), RunState::Failed);
        assert_eq!(outcome(None, true).state(), RunState::TimedOut);
    }

    #[test]
    fn long_output_keeps_its_end_and_stays_valid_text() {
        let long = format!("{}{}", "头".repeat(MAX_OUTPUT_BYTES), "end");
        let kept = tail(&long);
        assert!(kept.len() <= MAX_OUTPUT_BYTES && kept.ends_with("end"));
        assert_eq!(tail("short"), "short");
    }
}
