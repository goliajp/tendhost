use tendhost_kernel::{Envelope, Principal, Result, Role, Sink, Timestamp};
use tendhost_store::Keyspace;

const ENTRIES: &str = "entries";
pub const MAX_PAGE: usize = 500;

/// Keys: `entry:<event id>`, `entries` (by time). Entries are written once
/// and never changed or removed.
pub struct Audit {
    ks: Keyspace,
}

impl Audit {
    pub fn new(keyspace: Keyspace) -> Self {
        Self { ks: keyspace }
    }

    fn record(&self, envelope: &Envelope) -> Result<()> {
        self.ks.put(&format!("entry:{}", envelope.id), envelope)?;
        self.ks
            .index_put(ENTRIES, envelope.at.timestamp_millis(), &envelope.id)
    }

    /// Newest first: up to `limit` entries strictly older than `before`,
    /// or the newest ones when `before` is absent.
    pub fn entries(
        &self,
        principal: &Principal,
        before: Option<Timestamp>,
        limit: usize,
    ) -> Result<Vec<Envelope>> {
        principal.require(Role::Admin)?;
        let max = before.map_or(i64::MAX, |t| t.timestamp_millis() - 1);
        let ids = self.ks.index_range(ENTRIES, i64::MIN, max)?;
        let mut entries = Vec::new();
        for id in ids.iter().rev().take(limit.min(MAX_PAGE)) {
            entries.extend(self.ks.get::<Envelope>(&format!("entry:{id}"))?);
        }
        Ok(entries)
    }
}

impl Sink for Audit {
    fn accept(&self, envelope: &Envelope) {
        if !envelope.audited {
            return;
        }
        // the action already happened; the bus has no way to undo it
        if let Err(error) = self.record(envelope) {
            tracing::error!(%error, topic = %envelope.topic, event = %envelope.id, "audit entry was not written");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Actor, Bus, Error, Event, ManualClock};
    use tendhost_store::Store;

    use super::*;

    #[derive(Serialize)]
    struct Renamed {
        to: String,
    }

    impl Event for Renamed {
        const TOPIC: &'static str = "host.renamed";
    }

    #[derive(Serialize)]
    struct Measured {
        load: f32,
    }

    impl Event for Measured {
        const TOPIC: &'static str = "telemetry.measured";
        const AUDITED: bool = false;
    }

    fn admin() -> Principal {
        Principal {
            actor: Actor::User("usr_a".into()),
            role: Role::Admin,
        }
    }

    fn fixture() -> (Arc<Audit>, Bus, Arc<ManualClock>) {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let audit = Arc::new(Audit::new(Store::temp().unwrap().keyspace("audit")));
        bus.attach(audit.clone());
        (audit, bus, clock)
    }

    #[test]
    fn audited_events_are_kept_with_their_actor_and_others_are_not() {
        let (audit, bus, _) = fixture();
        bus.publish(Actor::User("usr_a".into()), &Renamed { to: "db".into() });
        bus.publish(Actor::Host("host_1".into()), &Measured { load: 0.5 });
        let entries = audit.entries(&admin(), None, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].topic, "host.renamed");
        assert_eq!(entries[0].actor, Actor::User("usr_a".into()));
        assert_eq!(entries[0].payload["to"], "db");
    }

    #[test]
    fn entries_page_from_newest_to_oldest_without_gaps_or_repeats() {
        let (audit, bus, clock) = fixture();
        for n in 0..5 {
            bus.publish(Actor::System, &Renamed { to: n.to_string() });
            clock.advance(Duration::seconds(1));
        }
        let names = |page: &[Envelope]| {
            page.iter()
                .map(|e| e.payload["to"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let first = audit.entries(&admin(), None, 2).unwrap();
        assert_eq!(names(&first), ["4", "3"]);
        let second = audit.entries(&admin(), Some(first[1].at), 2).unwrap();
        assert_eq!(names(&second), ["2", "1"]);
        let third = audit.entries(&admin(), Some(second[1].at), 2).unwrap();
        assert_eq!(names(&third), ["0"]);
    }

    #[test]
    fn a_viewer_cannot_read_the_log() {
        let (audit, _, _) = fixture();
        let viewer = Principal {
            actor: Actor::User("usr_v".into()),
            role: Role::Viewer,
        };
        assert!(matches!(
            audit.entries(&viewer, None, 10),
            Err(Error::Forbidden)
        ));
    }
}
