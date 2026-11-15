use std::time::Duration;

use anyhow::{Result, bail};
use chrono::Utc;
use tendhost_protocol::{Facts, Report, ReportBody};

use crate::client::{Client, Synced};
use crate::collect;
use crate::config::Config;
use crate::outbox::Outbox;

/// A day of reports at the default interval.
const OUTBOX_CAPACITY: usize = 2880;
const BATCH: usize = 500;
const FIRST_INTERVAL: Duration = Duration::from_secs(30);

/// One host's reporting state between syncs.
pub struct Reporter {
    client: Client,
    config: Config,
    outbox: Outbox,
    /// the facts the server last accepted
    reported_facts: Option<Facts>,
}

impl Reporter {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            client: Client::new(&config.server)?,
            config,
            outbox: Outbox::new(OUTBOX_CAPACITY),
            reported_facts: None,
        })
    }

    /// Measures, then sends everything not yet delivered. Returns how long
    /// the server asks to wait before the next cycle. A failed send keeps
    /// the reports for the next one.
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

        let mut next = FIRST_INTERVAL;
        while !self.outbox.is_empty() {
            let batch = self.outbox.batch(BATCH);
            let count = batch.len();
            match self.client.sync(&self.config.credential, batch).await? {
                Synced::Accepted(response) => {
                    self.outbox.sent(count);
                    next = Duration::from_secs(u64::from(response.next_sync_secs).clamp(5, 3600));
                }
                Synced::Rejected => bail!(Removed),
            }
        }
        if facts_changed {
            self.reported_facts = Some(facts);
        }
        Ok(next)
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
pub async fn run(config: Config) -> Result<()> {
    tracing::info!(server = %config.server, host = %config.host_id, mode = ?config.mode, "tendhostd is reporting");
    let mut reporter = Reporter::new(config)?;
    let mut wait = FIRST_INTERVAL;
    loop {
        match reporter.cycle().await {
            Ok(next) => wait = next,
            Err(error) if error.is::<Removed>() => return Err(error),
            Err(error) => tracing::warn!(error = format!("{error:#}"), "sync failed; will retry"),
        }
        tokio::time::sleep(wait).await;
    }
}
