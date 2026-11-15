use std::collections::VecDeque;

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
    fn a_full_outbox_drops_its_oldest_report() {
        let mut outbox = Outbox::new(2);
        (0..3).for_each(|n| outbox.push(report(n)));
        assert_eq!(outbox.batch(10), [report(1), report(2)]);
    }
}
