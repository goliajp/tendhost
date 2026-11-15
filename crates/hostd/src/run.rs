use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use chrono::Utc;
use tendhost_protocol::{Directive, Facts, Mode, Report, ReportBody, RunResult};
use tokio::sync::{Notify, mpsc};

use crate::client::{Client, Synced};
use crate::config::Config;
use crate::outbox::Outbox;
use crate::{collect, exec};

/// A day of reports at the default interval.
const OUTBOX_CAPACITY: usize = 2880;
const BATCH: usize = 500;
const FIRST_INTERVAL: Duration = Duration::from_secs(30);

/// One host's reporting state between syncs.
pub struct Reporter {
    client: Client,
    config: Config,
    outbox: Outbox,
    /// where waiting reports are kept across restarts; nowhere when absent
    spool: Option<PathBuf>,
    /// the facts the server last accepted
    reported_facts: Option<Facts>,
    finished_tx: mpsc::UnboundedSender<RunResult>,
    finished_rx: mpsc::UnboundedReceiver<RunResult>,
    /// signalled when a command finishes, so its result does not wait for
    /// the next scheduled sync
    wake: Arc<Notify>,
}

impl Reporter {
    /// `spool`: a file to keep undelivered reports in.
    pub fn new(config: Config, spool: Option<PathBuf>) -> Result<Self> {
        let outbox = match &spool {
            Some(path) => Outbox::restore(path, OUTBOX_CAPACITY)?,
            None => Outbox::new(OUTBOX_CAPACITY),
        };
        let (finished_tx, finished_rx) = mpsc::unbounded_channel();
        Ok(Self {
            client: Client::new(&config.server)?,
            config,
            outbox,
            spool,
            reported_facts: None,
            finished_tx,
            finished_rx,
            wake: Arc::new(Notify::new()),
        })
    }

    /// Measures, then sends everything not yet delivered and starts what
    /// the server asks for. Returns how long the server asks to wait
    /// before the next cycle. A failed send keeps the reports.
    pub async fn cycle(&mut self) -> Result<Duration> {
        let now = Utc::now();
        let facts = collect::facts()?;
        let facts_changed = self.reported_facts.as_ref() != Some(&facts);
        if facts_changed {
            self.outbox.push(Report {
                at: now,
                body: ReportBody::Facts(facts.clone()),
            });
        }
        self.outbox.push(Report {
            at: now,
            body: ReportBody::Metrics(collect::metrics()?),
        });
        let sent = self.flush().await;
        self.persist()?;
        let next = sent?;
        if facts_changed {
            self.reported_facts = Some(facts);
        }
        Ok(next)
    }

    /// Sends the results of commands that finished since the last sync,
    /// without measuring.
    pub async fn deliver_results(&mut self) -> Result<()> {
        let sent = self.flush().await;
        self.persist()?;
        sent.map(|_| ())
    }

    /// Waits for `wait` to pass or for a command to finish, whichever
    /// comes first.
    pub async fn idle(&self, wait: Duration) -> Idle {
        tokio::select! {
            _ = tokio::time::sleep(wait) => Idle::TimeToMeasure,
            _ = self.wake.notified() => Idle::ResultReady,
        }
    }

    async fn flush(&mut self) -> Result<Duration> {
        let mut next = FIRST_INTERVAL;
        loop {
            while let Ok(result) = self.finished_rx.try_recv() {
                self.outbox.push(Report {
                    at: result.finished_at,
                    body: ReportBody::RunFinished(result),
                });
            }
            if self.outbox.is_empty() {
                return Ok(next);
            }
            let batch = self.outbox.batch(BATCH);
            let count = batch.len();
            match self.client.sync(&self.config.credential, batch).await? {
                Synced::Accepted(response) => {
                    self.outbox.sent(count);
                    next = Duration::from_secs(u64::from(response.next_sync_secs).clamp(5, 3600));
                    response
                        .directives
                        .into_iter()
                        .for_each(|directive| self.obey(directive));
                }
                Synced::Rejected => bail!(Removed),
            }
        }
    }

    fn obey(&self, directive: Directive) {
        match directive {
            Directive::RunJob {
                run_id,
                command,
                timeout_secs,
            } => {
                let (finished, wake) = (self.finished_tx.clone(), self.wake.clone());
                let allowed = self.config.mode == Mode::Manage;
                tokio::spawn(async move {
                    let result = if allowed {
                        exec::execute(
                            run_id,
                            &command,
                            Duration::from_secs(u64::from(timeout_secs)),
                        )
                        .await
                    } else {
                        refused(run_id)
                    };
                    // the receiver lives as long as the reporter
                    let _ = finished.send(result);
                    wake.notify_one();
                });
            }
            // from a newer server
            Directive::Unknown => {}
        }
    }

    fn persist(&self) -> Result<()> {
        match &self.spool {
            Some(path) => self.outbox.persist(path),
            None => Ok(()),
        }
    }
}

pub enum Idle {
    TimeToMeasure,
    ResultReady,
}

/// The answer of a host that only reports to a command it was sent.
fn refused(run_id: String) -> RunResult {
    let now = Utc::now();
    RunResult {
        run_id,
        exit_code: None,
        timed_out: false,
        output: "this host is enrolled to report only and runs no commands; enroll it again with --manage to allow them\n"
            .into(),
        started_at: now,
        finished_at: now,
    }
}

/// The server no longer knows this host.
#[derive(Debug)]
pub struct Removed;

impl std::fmt::Display for Removed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the server no longer knows this host; it was removed there. Enroll it again to bring it back")
    }
}

impl std::error::Error for Removed {}

/// Reports until the host is removed on the server. Any other failure is
/// logged and the next cycle tries again.
pub async fn run(config: Config, spool: Option<PathBuf>) -> Result<()> {
    tracing::info!(server = %config.server, host = %config.host_id, mode = ?config.mode, "tendhostd is reporting");
    let mut reporter = Reporter::new(config, spool)?;
    let mut wait = FIRST_INTERVAL;
    let mut measure = true;
    loop {
        let outcome = if measure {
            reporter.cycle().await.map(Some)
        } else {
            reporter.deliver_results().await.map(|_| None)
        };
        match outcome {
            Ok(Some(next)) => wait = next,
            Ok(None) => {}
            Err(error) if error.is::<Removed>() => return Err(error),
            Err(error) => tracing::warn!(error = format!("{error:#}"), "sync failed; will retry"),
        }
        measure = matches!(reporter.idle(wait).await, Idle::TimeToMeasure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_only_host_answers_a_command_with_a_refusal() {
        let result = refused("run_1".into());
        assert_eq!((result.exit_code, result.timed_out), (None, false));
        assert!(result.output.contains("--manage"));
    }
}
