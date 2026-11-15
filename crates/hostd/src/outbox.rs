use std::collections::VecDeque;
use std::path::Path;

use anyhow::{Context, Result};
use tendhost_protocol::Report;

/// Reports waiting to reach the server. They stay until a sync succeeds,
/// so a server that is down for a while loses nothing recent; past the
/// capacity the oldest go first.
pub struct Outbox {
    reports: VecDeque<Report>,
    capacity: usize,
}

impl Outbox {
    pub fn new(capacity: usize) -> Self {
        Self {
            reports: VecDeque::new(),
            capacity,
        }
    }

    pub fn push(&mut self, report: Report) {
        if self.reports.len() == self.capacity {
            self.reports.pop_front();
        }
        self.reports.push_back(report);
    }

    /// The oldest reports, at most `max`, left in place.
    pub fn batch(&self, max: usize) -> Vec<Report> {
        self.reports.iter().take(max).cloned().collect()
    }

    /// Drops the `count` oldest: the ones a successful sync just carried.
    pub fn sent(&mut self, count: usize) {
        self.reports.drain(..count.min(self.reports.len()));
    }

    /// Makes `path` hold exactly what is waiting: no file when nothing is.
    /// In normal operation every sync empties the outbox, so the disk is
    /// only written while the server cannot be reached.
    pub fn persist(&self, path: &Path) -> Result<()> {
        if self.reports.is_empty() {
            return match std::fs::remove_file(path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    Err(error).with_context(|| format!("removing {}", path.display()))
                }
                _ => Ok(()),
            };
        }
        let text = serde_json::to_vec(&self.reports).context("encoding the waiting reports")?;
        let partial = path.with_extension("partial");
        std::fs::write(&partial, text).with_context(|| format!("writing {}", partial.display()))?;
        std::fs::rename(&partial, path).with_context(|| format!("replacing {}", path.display()))
    }

    /// The outbox a previous run of the daemon left at `path`.
    pub fn restore(path: &Path, capacity: usize) -> Result<Self> {
        let mut outbox = Self::new(capacity);
        let text = match std::fs::read(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(outbox),
            Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
        };
        let reports: Vec<Report> = serde_json::from_slice(&text)
            .with_context(|| format!("{} is not a saved outbox", path.display()))?;
        reports.into_iter().for_each(|report| outbox.push(report));
        Ok(outbox)
    }

    pub fn len(&self) -> usize {
        self.reports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use tendhost_protocol::{Metrics, ReportBody};

    use super::*;

    fn report(n: i64) -> Report {
        Report {
            at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + Duration::seconds(n),
            body: ReportBody::Metrics(Metrics {
                load1: n as f32,
                memory_used_bytes: 0,
                uptime_secs: 0,
                disks: vec![],
            }),
        }
    }

    #[test]
    fn reports_stay_until_marked_sent() {
        let mut outbox = Outbox::new(10);
        (0..3).for_each(|n| outbox.push(report(n)));
        assert_eq!(outbox.batch(2), [report(0), report(1)]);
        assert_eq!(outbox.len(), 3);
        outbox.sent(2);
        assert_eq!(outbox.batch(10), [report(2)]);
        outbox.sent(5);
        assert!(outbox.is_empty());
    }

    #[test]
    fn waiting_reports_survive_a_restart_and_the_file_goes_when_they_are_sent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.json");
        assert!(Outbox::restore(&path, 10).unwrap().is_empty());

        let mut outbox = Outbox::new(10);
        (0..3).for_each(|n| outbox.push(report(n)));
        outbox.persist(&path).unwrap();
        let mut restored = Outbox::restore(&path, 10).unwrap();
        assert_eq!(restored.batch(10), [report(0), report(1), report(2)]);

        restored.sent(3);
        restored.persist(&path).unwrap();
        assert!(!path.exists());
        restored.persist(&path).unwrap();
    }

    #[test]
    fn a_damaged_file_is_reported_and_not_silently_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outbox.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Outbox::restore(&path, 10).is_err());
    }

    #[test]
    fn a_full_outbox_drops_its_oldest_report() {
        let mut outbox = Outbox::new(2);
        (0..3).for_each(|n| outbox.push(report(n)));
        assert_eq!(outbox.batch(10), [report(1), report(2)]);
    }
}
