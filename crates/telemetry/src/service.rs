use std::sync::Arc;

use serde::Deserialize;
use tendhost_kernel::{Actor, Bus, Clock, Envelope, Principal, Result, Role, Sink, Timestamp};
use tendhost_store::Keyspace;

use crate::events::{DiskUse, HostOffline, HostOnline, SampleRecorded};
use crate::model::{OFFLINE_AFTER, Presence, RETENTION, Sample, thin};

const PRESENT: &str = "present";

/// Keys: `presence:<host>`, `present` (hosts by last seen),
/// `series:<host>` (samples by time; each member is the sample itself).
pub struct Telemetry {
    ks: Keyspace,
    bus: Bus,
    clock: Arc<dyn Clock>,
}

/// This module's view of `host.removed`.
#[derive(Deserialize)]
struct Removed {
    host_id: String,
}

impl Telemetry {
    pub fn new(keyspace: Keyspace, bus: Bus, clock: Arc<dyn Clock>) -> Self {
        Self {
            ks: keyspace,
            bus,
            clock,
        }
    }

    /// The host just made contact.
    pub fn heard_from(&self, host_id: &str) -> Result<()> {
        let now = self.clock.now();
        let was_online = self
            .ks
            .get::<Presence>(&presence_key(host_id))?
            .is_some_and(|p| p.online);
        self.put_presence(&Presence {
            host_id: host_id.to_string(),
            last_seen: now,
            online: true,
        })?;
        if !was_online {
            self.bus.publish(
                Actor::Host(host_id.to_string()),
                &HostOnline {
                    host_id: host_id.to_string(),
                },
            );
        }
        Ok(())
    }

    /// Adds a measurement. Recording the same sample again changes nothing.
    pub fn record(&self, host_id: &str, sample: &Sample) -> Result<()> {
        let series = series_key(host_id);
        let member = serde_json::to_string(sample).map_err(tendhost_kernel::Error::internal)?;
        self.ks
            .index_put(&series, sample.at.timestamp_millis(), &member)?;
        self.ks
            .index_trim(&series, (self.clock.now() - RETENTION).timestamp_millis())?;
        let disks = sample
            .disks
            .iter()
            .map(|d| DiskUse {
                mount: d.mount.clone(),
                total_bytes: d.total_bytes,
                used_bytes: d.used_bytes,
            })
            .collect();
        self.bus.publish(
            Actor::Host(host_id.to_string()),
            &SampleRecorded {
                host_id: host_id.to_string(),
                at: sample.at,
                disks,
            },
        );
        Ok(())
    }

    /// Marks hosts not heard from for [`OFFLINE_AFTER`] as offline. Run on
    /// a timer; returns the hosts that went offline in this pass.
    pub fn sweep(&self) -> Result<Vec<String>> {
        let silent_since = (self.clock.now() - OFFLINE_AFTER).timestamp_millis();
        let mut went_offline = Vec::new();
        for host_id in self.ks.index_range(PRESENT, i64::MIN, silent_since)? {
            let Some(mut presence) = self.ks.get::<Presence>(&presence_key(&host_id))? else {
                continue;
            };
            if !presence.online {
                continue;
            }
            presence.online = false;
            self.ks.put(&presence_key(&host_id), &presence)?;
            self.bus.publish(
                Actor::System,
                &HostOffline {
                    host_id: host_id.clone(),
                    last_seen: presence.last_seen,
                },
            );
            went_offline.push(host_id);
        }
        Ok(went_offline)
    }

    /// Every host heard from at least once, longest silent first.
    pub fn presence(&self, principal: &Principal) -> Result<Vec<Presence>> {
        principal.require(Role::Viewer)?;
        let mut all = Vec::new();
        for host_id in self.ks.index_all(PRESENT)? {
            all.extend(self.ks.get::<Presence>(&presence_key(&host_id))?);
        }
        Ok(all)
    }

    /// A host's samples from `since` on, oldest first, thinned to at most
    /// `points` when there are more (0 for all of them).
    pub fn samples(
        &self,
        principal: &Principal,
        host_id: &str,
        since: Timestamp,
        points: usize,
    ) -> Result<Vec<Sample>> {
        principal.require(Role::Viewer)?;
        let samples: Result<Vec<Sample>> = self
            .ks
            .index_range(&series_key(host_id), since.timestamp_millis(), i64::MAX)?
            .iter()
            .map(|member| serde_json::from_str(member).map_err(tendhost_kernel::Error::internal))
            .collect();
        Ok(thin(samples?, points))
    }

    fn forget(&self, host_id: &str) -> Result<()> {
        self.ks.index_remove(PRESENT, host_id)?;
        self.ks.delete(&presence_key(host_id))?;
        self.ks.index_trim(&series_key(host_id), i64::MAX)?;
        Ok(())
    }

    fn put_presence(&self, presence: &Presence) -> Result<()> {
        self.ks.put(&presence_key(&presence.host_id), presence)?;
        self.ks.index_put(
            PRESENT,
            presence.last_seen.timestamp_millis(),
            &presence.host_id,
        )
    }
}

impl Sink for Telemetry {
    fn accept(&self, envelope: &Envelope) {
        if let Some(removed) = envelope.view::<Removed>("host.removed")
            && let Err(error) = self.forget(&removed.host_id)
        {
            tracing::error!(%error, host = %removed.host_id, "measurements of a removed host were not deleted");
        }
    }
}

fn presence_key(host_id: &str) -> String {
    format!("presence:{host_id}")
}

fn series_key(host_id: &str) -> String {
    format!("series:{host_id}")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{Duration, TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Event, ManualClock};
    use tendhost_store::Store;

    use super::*;
    use crate::model::Disk;

    #[derive(Default)]
    struct Topics(Mutex<Vec<String>>);

    impl Sink for Topics {
        fn accept(&self, envelope: &Envelope) {
            self.0.lock().unwrap().push(envelope.topic.clone());
        }
    }

    #[derive(Serialize)]
    struct HostRemoved {
        host_id: String,
        name: String,
    }

    impl Event for HostRemoved {
        const TOPIC: &'static str = "host.removed";
    }

    struct Fixture {
        telemetry: Arc<Telemetry>,
        bus: Bus,
        clock: Arc<ManualClock>,
        topics: Arc<Topics>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let topics = Arc::new(Topics::default());
        let telemetry = Arc::new(Telemetry::new(
            Store::temp().unwrap().keyspace("telemetry"),
            bus.clone(),
            clock.clone(),
        ));
        bus.attach(telemetry.clone());
        bus.attach(topics.clone());
        Fixture {
            telemetry,
            bus,
            clock,
            topics,
        }
    }

    fn viewer() -> Principal {
        Principal {
            actor: Actor::User("usr_v".into()),
            role: Role::Viewer,
        }
    }

    fn sample(at: Timestamp, load1: f32) -> Sample {
        Sample {
            at,
            load1,
            memory_used_bytes: 1 << 30,
            uptime_secs: 60,
            disks: vec![Disk {
                mount: "/".into(),
                total_bytes: 100,
                used_bytes: 40,
            }],
        }
    }

    #[test]
    fn a_host_goes_offline_after_silence_and_online_again_on_contact() {
        let f = fixture();
        f.telemetry.heard_from("host_a").unwrap();
        f.clock.advance(OFFLINE_AFTER - Duration::seconds(1));
        assert!(f.telemetry.sweep().unwrap().is_empty());
        f.clock.advance(Duration::seconds(1));
        assert_eq!(f.telemetry.sweep().unwrap(), ["host_a"]);
        assert!(f.telemetry.sweep().unwrap().is_empty());
        assert!(!f.telemetry.presence(&viewer()).unwrap()[0].online);

        f.telemetry.heard_from("host_a").unwrap();
        assert!(f.telemetry.presence(&viewer()).unwrap()[0].online);
        assert_eq!(
            *f.topics.0.lock().unwrap(),
            [
                "telemetry.host_online",
                "telemetry.host_offline",
                "telemetry.host_online"
            ]
        );
    }

    #[test]
    fn regular_contact_keeps_a_host_online_without_repeating_the_event() {
        let f = fixture();
        for _ in 0..5 {
            f.telemetry.heard_from("host_a").unwrap();
            f.clock.advance(Duration::seconds(30));
            assert!(f.telemetry.sweep().unwrap().is_empty());
        }
        assert_eq!(*f.topics.0.lock().unwrap(), ["telemetry.host_online"]);
    }

    #[test]
    fn only_the_silent_host_goes_offline() {
        let f = fixture();
        f.telemetry.heard_from("host_a").unwrap();
        f.clock.advance(Duration::seconds(60));
        f.telemetry.heard_from("host_b").unwrap();
        f.clock.advance(Duration::seconds(30));
        assert_eq!(f.telemetry.sweep().unwrap(), ["host_a"]);
        let presence = f.telemetry.presence(&viewer()).unwrap();
        let online: Vec<_> = presence
            .iter()
            .map(|p| (p.host_id.as_str(), p.online))
            .collect();
        assert_eq!(online, [("host_a", false), ("host_b", true)]);
    }

    #[test]
    fn samples_come_back_in_time_order_from_the_asked_moment_and_resending_adds_nothing() {
        let f = fixture();
        let t0 = f.clock.now();
        for n in [2, 0, 1] {
            f.telemetry
                .record("host_a", &sample(t0 + Duration::minutes(n), n as f32))
                .unwrap();
        }
        f.telemetry
            .record("host_a", &sample(t0 + Duration::minutes(1), 1.0))
            .unwrap();
        f.telemetry.record("host_b", &sample(t0, 9.0)).unwrap();
        let loads = |since| {
            let samples = f.telemetry.samples(&viewer(), "host_a", since, 0).unwrap();
            samples.iter().map(|s| s.load1).collect::<Vec<_>>()
        };
        assert_eq!(loads(t0), [0.0, 1.0, 2.0]);
        assert_eq!(loads(t0 + Duration::minutes(1)), [1.0, 2.0]);
    }

    #[test]
    fn samples_older_than_the_retention_are_dropped() {
        let f = fixture();
        let t0 = f.clock.now();
        f.telemetry.record("host_a", &sample(t0, 1.0)).unwrap();
        f.clock.advance(RETENTION + Duration::seconds(1));
        f.telemetry
            .record("host_a", &sample(f.clock.now(), 2.0))
            .unwrap();
        let kept = f.telemetry.samples(&viewer(), "host_a", t0, 0).unwrap();
        assert_eq!(kept.iter().map(|s| s.load1).collect::<Vec<_>>(), [2.0]);
    }

    #[test]
    fn a_removed_host_leaves_nothing_behind() {
        let f = fixture();
        let t0 = f.clock.now();
        f.telemetry.heard_from("host_a").unwrap();
        f.telemetry.heard_from("host_b").unwrap();
        f.telemetry.record("host_a", &sample(t0, 1.0)).unwrap();
        f.bus.publish(
            Actor::System,
            &HostRemoved {
                host_id: "host_a".into(),
                name: "a".into(),
            },
        );
        assert!(
            f.telemetry
                .samples(&viewer(), "host_a", t0, 0)
                .unwrap()
                .is_empty()
        );
        let left: Vec<_> = f
            .telemetry
            .presence(&viewer())
            .unwrap()
            .into_iter()
            .map(|p| p.host_id)
            .collect();
        assert_eq!(left, ["host_b"]);
        f.clock.advance(OFFLINE_AFTER);
        assert_eq!(f.telemetry.sweep().unwrap(), ["host_b"]);
    }
}
