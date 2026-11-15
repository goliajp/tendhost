use std::sync::atomic::{AtomicI64, Ordering};

use serde::{Deserialize, Serialize};
use tendhost_kernel::{Envelope, Principal, Result, Role, Sink};
use tendhost_store::Keyspace;

const ENTRIES: &str = "entries";
pub const MAX_PAGE: usize = 500;

/// One line of the log. `seq` numbers the entries in the order they were
/// recorded; time alone cannot, since several can share an instant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: i64,
    #[serde(flatten)]
    pub event: Envelope,
}

/// Keys: `entry:<seq>`, `entries` (by seq). Entries are written once and
/// never changed or removed.
pub struct Audit {
    ks: Keyspace,
    last_seq: AtomicI64,
}

impl Audit {
    pub fn new(keyspace: Keyspace) -> Result<Self> {
        let last_seq = AtomicI64::new(keyspace.index_max(ENTRIES)?.unwrap_or(0));
        Ok(Self {
            ks: keyspace,
            last_seq,
        })
    }

    fn record(&self, envelope: &Envelope) -> Result<()> {
        let seq = self.last_seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.ks.put(
            &format!("entry:{seq}"),
            &Entry {
                seq,
                event: envelope.clone(),
            },
        )?;
        self.ks.index_put(ENTRIES, seq, &seq.to_string())
    }

    /// Newest first: up to `limit` entries with `seq` below `before`, or
    /// the newest ones when `before` is absent.
    pub fn entries(
        &self,
        principal: &Principal,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Entry>> {
        principal.require(Role::Admin)?;
        // seqs are contiguous, so a page is exactly a range of them
        let max = before.map_or(self.last_seq.load(Ordering::SeqCst), |seq| seq - 1);
        let min = max.saturating_sub(limit.min(MAX_PAGE) as i64 - 1);
        let mut entries = Vec::new();
        for seq in self
            .ks
            .index_range(ENTRIES, min, max)?
            .iter()
            .rev()
            .take(limit.min(MAX_PAGE))
        {
            entries.extend(self.ks.get::<Entry>(&format!("entry:{seq}"))?);
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

    use chrono::{TimeZone, Utc};
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

    fn fixture() -> (Arc<Audit>, Bus, Store) {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock);
        let store = Store::temp().unwrap();
        let audit = Arc::new(Audit::new(store.keyspace("audit")).unwrap());
        bus.attach(audit.clone());
        (audit, bus, store)
    }

    #[test]
    fn audited_events_are_kept_with_their_actor_and_others_are_not() {
        let (audit, bus, _store) = fixture();
        bus.publish(Actor::User("usr_a".into()), &Renamed { to: "db".into() });
        bus.publish(Actor::Host("host_1".into()), &Measured { load: 0.5 });
        let entries = audit.entries(&admin(), None, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event.topic, "host.renamed");
        assert_eq!(entries[0].event.actor, Actor::User("usr_a".into()));
        assert_eq!(entries[0].event.payload["to"], "db");
    }

    #[test]
    fn entries_recorded_in_the_same_instant_page_in_order_without_gaps_or_repeats() {
        let (audit, bus, _store) = fixture();
        for n in 0..5 {
            bus.publish(Actor::System, &Renamed { to: n.to_string() });
        }
        let names = |page: &[Entry]| {
            page.iter()
                .map(|e| e.event.payload["to"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let first = audit.entries(&admin(), None, 2).unwrap();
        assert_eq!(names(&first), ["4", "3"]);
        let second = audit.entries(&admin(), Some(first[1].seq), 2).unwrap();
        assert_eq!(names(&second), ["2", "1"]);
        let third = audit.entries(&admin(), Some(second[1].seq), 2).unwrap();
        assert_eq!(names(&third), ["0"]);
        assert!(
            audit
                .entries(&admin(), Some(third[0].seq), 2)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn numbering_continues_after_a_restart() {
        let (audit, bus, store) = fixture();
        bus.publish(Actor::System, &Renamed { to: "a".into() });
        drop(audit);
        let reopened = Audit::new(store.keyspace("audit")).unwrap();
        reopened.accept(&Envelope {
            id: "evt_x".into(),
            topic: "host.renamed".into(),
            at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            actor: Actor::System,
            audited: true,
            payload: serde_json::json!({ "to": "b" }),
        });
        let seqs: Vec<_> = reopened
            .entries(&admin(), None, 10)
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect();
        assert_eq!(seqs, [2, 1]);
    }

    #[test]
    fn a_viewer_cannot_read_the_log() {
        let (audit, _bus, _store) = fixture();
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
