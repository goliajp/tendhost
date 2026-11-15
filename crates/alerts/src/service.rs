use std::sync::Arc;

use serde::Deserialize;
use tendhost_kernel::{Actor, Bus, Clock, Envelope, Principal, Result, Role, Sink, Timestamp, id};
use tendhost_store::Keyspace;

use crate::events::{AlertOpened, AlertResolved, SettingsChanged};
use crate::model::{Alert, HISTORY_KEPT, Kind, Settings, percent};
use crate::ports::HostNames;

const OPEN: &str = "open";
const ALL: &str = "all";
const SETTINGS: &str = "settings";

/// Keys: `open:<key>` → alert id, `open` (open alert keys by opening
/// time), `alert:<id>`, `all` (alert ids by opening time), `settings`.
pub struct Alerts {
    ks: Keyspace,
    bus: Bus,
    clock: Arc<dyn Clock>,
    hosts: Arc<dyn HostNames>,
}

// This module's views of the events it reacts to.

#[derive(Deserialize)]
struct OfHost {
    host_id: String,
}

#[derive(Deserialize)]
struct WentOffline {
    host_id: String,
    last_seen: Timestamp,
}

#[derive(Deserialize)]
struct Sampled {
    host_id: String,
    disks: Vec<DiskUse>,
}

#[derive(Deserialize)]
struct DiskUse {
    mount: String,
    total_bytes: u64,
    used_bytes: u64,
}

#[derive(Deserialize)]
struct LimitRead {
    host_id: String,
    agent: String,
    window: String,
    used_ratio: f32,
}

#[derive(Deserialize)]
struct RunFinished {
    job_id: String,
    job_name: String,
    host_id: String,
    state: String,
}

impl Alerts {
    pub fn new(
        keyspace: Keyspace,
        bus: Bus,
        clock: Arc<dyn Clock>,
        hosts: Arc<dyn HostNames>,
    ) -> Self {
        Self {
            ks: keyspace,
            bus,
            clock,
            hosts,
        }
    }

    pub fn settings(&self, principal: &Principal) -> Result<Settings> {
        principal.require(Role::Viewer)?;
        self.current_settings()
    }

    /// Changes what raises an alert from now on. Alerts already open stay
    /// until their condition ends.
    pub fn set_settings(&self, principal: &Principal, settings: Settings) -> Result<Settings> {
        principal.require(Role::Admin)?;
        let settings = settings.validated()?;
        self.ks.put(SETTINGS, &settings)?;
        self.bus.publish(
            principal.actor.clone(),
            &SettingsChanged {
                settings: settings.clone(),
            },
        );
        Ok(settings)
    }

    /// Open alerts, oldest first.
    pub fn open(&self, principal: &Principal) -> Result<Vec<Alert>> {
        principal.require(Role::Viewer)?;
        self.open_alerts()
    }

    /// Open and resolved alerts, newest first.
    pub fn history(&self, principal: &Principal, limit: usize) -> Result<Vec<Alert>> {
        principal.require(Role::Viewer)?;
        let mut alerts = Vec::new();
        for id in self.ks.index_all(ALL)?.iter().rev().take(limit) {
            alerts.extend(self.ks.get::<Alert>(&format!("alert:{id}"))?);
        }
        Ok(alerts)
    }

    fn react(&self, envelope: &Envelope) -> Result<()> {
        let settings = self.current_settings()?;
        match envelope.topic.as_str() {
            "telemetry.host_offline" if settings.host_offline => {
                let Some(event) = envelope.view::<WentOffline>(&envelope.topic) else {
                    return Ok(());
                };
                let detail = format!(
                    "Last heard from at {}.",
                    event.last_seen.format("%Y-%m-%d %H:%M UTC")
                );
                self.raise(
                    format!("offline:{}", event.host_id),
                    Kind::HostOffline,
                    &event.host_id,
                    "is offline".into(),
                    detail,
                )
            }
            "telemetry.host_online" => {
                let Some(event) = envelope.view::<OfHost>(&envelope.topic) else {
                    return Ok(());
                };
                self.settle(&format!("offline:{}", event.host_id))
            }
            "telemetry.sample_recorded" => {
                let (Some(event), Some(threshold)) = (
                    envelope.view::<Sampled>(&envelope.topic),
                    settings.disk_usage,
                ) else {
                    return Ok(());
                };
                for disk in event.disks.iter().filter(|d| d.total_bytes > 0) {
                    let used = disk.used_bytes as f32 / disk.total_bytes as f32;
                    let key = format!("disk:{}:{}", event.host_id, disk.mount);
                    if used >= threshold {
                        let what = format!("disk {} is {} full", disk.mount, percent(used));
                        let detail = format!("The alert level is {}.", percent(threshold));
                        self.raise(key, Kind::DiskUsage, &event.host_id, what, detail)?;
                    } else {
                        self.settle(&key)?;
                    }
                }
                Ok(())
            }
            "agents.limit_read" => {
                let (Some(event), Some(threshold)) = (
                    envelope.view::<LimitRead>(&envelope.topic),
                    settings.agent_limit,
                ) else {
                    return Ok(());
                };
                let key = format!("limit:{}:{}:{}", event.host_id, event.agent, event.window);
                if event.used_ratio >= threshold {
                    let what = format!(
                        "{} has used {} of its {} limit",
                        event.agent,
                        percent(event.used_ratio),
                        event.window
                    );
                    let detail = format!("The alert level is {}.", percent(threshold));
                    self.raise(key, Kind::AgentLimit, &event.host_id, what, detail)
                } else {
                    self.settle(&key)
                }
            }
            "job.run_finished" => {
                let Some(event) = envelope.view::<RunFinished>(&envelope.topic) else {
                    return Ok(());
                };
                let key = format!("job:{}", event.job_id);
                if event.state == "succeeded" {
                    return self.settle(&key);
                }
                if !settings.job_failed {
                    return Ok(());
                }
                let how = match event.state.as_str() {
                    "timed_out" => "timed out",
                    "missed" => "was not collected by the host in time",
                    "lost" => "started but never reported back",
                    _ => "failed",
                };
                let what = format!("job {} {how}", event.job_name);
                self.raise(
                    key,
                    Kind::JobFailed,
                    &event.host_id,
                    what,
                    "It resolves when the job next succeeds.".into(),
                )
            }
            "host.removed" => {
                let Some(event) = envelope.view::<OfHost>(&envelope.topic) else {
                    return Ok(());
                };
                // the host is gone, and with it everything that was wrong
                // with it; nobody is told
                for alert in self
                    .open_alerts()?
                    .into_iter()
                    .filter(|a| a.host_id == event.host_id)
                {
                    self.close(alert)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Opens the alert for `key` unless it is open already. `what` is the
    /// sentence after the host's name.
    fn raise(
        &self,
        key: String,
        kind: Kind,
        host_id: &str,
        what: String,
        detail: String,
    ) -> Result<()> {
        if self.ks.get::<String>(&format!("open:{key}"))?.is_some() {
            return Ok(());
        }
        let now = self.clock.now();
        let name = self
            .hosts
            .name(host_id)
            .unwrap_or_else(|| host_id.to_string());
        let summary = match kind {
            Kind::HostOffline => format!("{name} {what}"),
            _ => format!("{name}: {what}"),
        };
        let alert = Alert {
            id: id::new("alr", now),
            key,
            kind,
            host_id: host_id.to_string(),
            summary,
            detail,
            opened_at: now,
            resolved_at: None,
        };
        self.ks.put(&format!("alert:{}", alert.id), &alert)?;
        self.ks.put(&format!("open:{}", alert.key), &alert.id)?;
        self.ks
            .index_put(OPEN, now.timestamp_millis(), &alert.key)?;
        self.ks.index_put(ALL, now.timestamp_millis(), &alert.id)?;
        let stale = self.ks.index_all(ALL)?;
        for old in stale.iter().take(stale.len().saturating_sub(HISTORY_KEPT)) {
            if self
                .ks
                .get::<Alert>(&format!("alert:{old}"))?
                .is_none_or(|a| a.resolved_at.is_some())
            {
                self.ks.index_remove(ALL, old)?;
                self.ks.delete(&format!("alert:{old}"))?;
            }
        }
        self.bus.publish(
            Actor::System,
            &AlertOpened {
                alert_id: alert.id,
                kind,
                host_id: alert.host_id,
                summary: alert.summary,
                detail: alert.detail,
            },
        );
        Ok(())
    }

    /// Resolves the alert for `key` if one is open, and says so.
    fn settle(&self, key: &str) -> Result<()> {
        let Some(alert_id) = self.ks.get::<String>(&format!("open:{key}"))? else {
            return Ok(());
        };
        let Some(alert) = self.ks.get::<Alert>(&format!("alert:{alert_id}"))? else {
            return Ok(());
        };
        let alert = self.close(alert)?;
        self.bus.publish(
            Actor::System,
            &AlertResolved {
                alert_id: alert.id,
                kind: alert.kind,
                host_id: alert.host_id,
                summary: alert.summary,
            },
        );
        Ok(())
    }

    fn close(&self, mut alert: Alert) -> Result<Alert> {
        alert.resolved_at = Some(self.clock.now());
        self.ks.put(&format!("alert:{}", alert.id), &alert)?;
        self.ks.delete(&format!("open:{}", alert.key))?;
        self.ks.index_remove(OPEN, &alert.key)?;
        Ok(alert)
    }

    fn open_alerts(&self) -> Result<Vec<Alert>> {
        let mut alerts = Vec::new();
        for key in self.ks.index_all(OPEN)? {
            if let Some(id) = self.ks.get::<String>(&format!("open:{key}"))? {
                alerts.extend(self.ks.get::<Alert>(&format!("alert:{id}"))?);
            }
        }
        Ok(alerts)
    }

    fn current_settings(&self) -> Result<Settings> {
        Ok(self.ks.get(SETTINGS)?.unwrap_or_default())
    }
}

impl Sink for Alerts {
    fn accept(&self, envelope: &Envelope) {
        if let Err(error) = self.react(envelope) {
            tracing::error!(%error, topic = %envelope.topic, "an event was not turned into an alert");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Error, Event, ManualClock};
    use tendhost_store::Store;

    use super::*;

    struct Names;

    impl HostNames for Names {
        fn name(&self, host_id: &str) -> Option<String> {
            (host_id == "host_db").then(|| "db-01".to_string())
        }
    }

    #[derive(Default)]
    struct Announced(Mutex<Vec<(String, String)>>);

    impl Sink for Announced {
        fn accept(&self, envelope: &Envelope) {
            if envelope.topic.starts_with("alert.") {
                let summary = envelope.payload["summary"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                self.0
                    .lock()
                    .unwrap()
                    .push((envelope.topic.clone(), summary));
            }
        }
    }

    macro_rules! event {
        ($name:ident, $topic:literal, { $($field:ident : $ty:ty),* }) => {
            #[derive(Serialize)]
            struct $name { $($field: $ty),* }
            impl Event for $name {
                const TOPIC: &'static str = $topic;
                const AUDITED: bool = false;
            }
        };
    }

    event!(Offline, "telemetry.host_offline", { host_id: String, last_seen: Timestamp });
    event!(Online, "telemetry.host_online", { host_id: String });
    event!(Sample, "telemetry.sample_recorded", { host_id: String, disks: Vec<Disk> });
    event!(Limit, "agents.limit_read", { host_id: String, agent: String, window: String, used_ratio: f32 });
    event!(Run, "job.run_finished", { job_id: String, job_name: String, host_id: String, state: String });
    event!(HostRemoved, "host.removed", { host_id: String });

    #[derive(Serialize)]
    struct Disk {
        mount: String,
        total_bytes: u64,
        used_bytes: u64,
    }

    struct Fixture {
        alerts: Arc<Alerts>,
        bus: Bus,
        clock: Arc<ManualClock>,
        announced: Arc<Announced>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let keyspace = Store::temp().unwrap().keyspace("alerts");
        let alerts = Arc::new(Alerts::new(
            keyspace,
            bus.clone(),
            clock.clone(),
            Arc::new(Names),
        ));
        let announced = Arc::new(Announced::default());
        bus.attach(alerts.clone());
        bus.attach(announced.clone());
        Fixture {
            alerts,
            bus,
            clock,
            announced,
        }
    }

    fn admin() -> Principal {
        Principal {
            actor: Actor::User("usr_a".into()),
            role: Role::Admin,
        }
    }

    fn viewer() -> Principal {
        Principal {
            actor: Actor::User("usr_v".into()),
            role: Role::Viewer,
        }
    }

    fn summaries(f: &Fixture) -> Vec<String> {
        f.alerts
            .open(&viewer())
            .unwrap()
            .into_iter()
            .map(|a| a.summary)
            .collect()
    }

    fn sample(f: &Fixture, used: u64) {
        let disk = Disk {
            mount: "/".into(),
            total_bytes: 100,
            used_bytes: used,
        };
        f.bus.publish(
            Actor::System,
            &Sample {
                host_id: "host_db".into(),
                disks: vec![disk],
            },
        );
    }

    fn run(f: &Fixture, state: &str) {
        let event = Run {
            job_id: "job_1".into(),
            job_name: "backup".into(),
            host_id: "host_db".into(),
            state: state.into(),
        };
        f.bus.publish(Actor::System, &event);
    }

    #[test]
    fn a_silent_host_has_one_alert_until_it_is_heard_from_again() {
        let f = fixture();
        let offline = || Offline {
            host_id: "host_db".into(),
            last_seen: f.clock.now(),
        };
        f.bus.publish(Actor::System, &offline());
        f.bus.publish(Actor::System, &offline());
        assert_eq!(summaries(&f), ["db-01 is offline"]);
        assert_eq!(
            f.alerts.open(&viewer()).unwrap()[0].detail,
            "Last heard from at 2026-01-01 12:00 UTC."
        );

        f.bus.publish(
            Actor::System,
            &Online {
                host_id: "host_db".into(),
            },
        );
        f.bus.publish(
            Actor::System,
            &Online {
                host_id: "host_db".into(),
            },
        );
        assert!(summaries(&f).is_empty());
        let announced = f.announced.0.lock().unwrap();
        let topics: Vec<_> = announced.iter().map(|(topic, _)| topic.as_str()).collect();
        assert_eq!(topics, ["alert.opened", "alert.resolved"]);
        assert_eq!(announced[1].1, "db-01 is offline");
    }

    #[test]
    fn a_disk_alert_opens_at_the_level_and_resolves_below_it() {
        let f = fixture();
        sample(&f, 89);
        assert!(summaries(&f).is_empty());
        sample(&f, 90);
        sample(&f, 95);
        assert_eq!(summaries(&f), ["db-01: disk / is 90% full"]);
        sample(&f, 89);
        assert!(summaries(&f).is_empty());
        assert_eq!(f.announced.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn an_agent_near_its_limit_is_told_once_per_window() {
        let f = fixture();
        let limit = |window: &str, used| Limit {
            host_id: "host_db".into(),
            agent: "codex".into(),
            window: window.into(),
            used_ratio: used,
        };
        f.bus.publish(Actor::System, &limit("5h", 0.95));
        f.bus.publish(Actor::System, &limit("5h", 0.97));
        f.bus.publish(Actor::System, &limit("7d", 0.5));
        assert_eq!(summaries(&f), ["db-01: codex has used 95% of its 5h limit"]);
        f.bus.publish(Actor::System, &limit("5h", 0.02));
        assert!(summaries(&f).is_empty());
    }

    #[test]
    fn a_failed_job_stays_alerted_until_it_succeeds() {
        let f = fixture();
        run(&f, "succeeded");
        run(&f, "timed_out");
        run(&f, "failed");
        assert_eq!(summaries(&f), ["db-01: job backup timed out"]);
        run(&f, "succeeded");
        assert!(summaries(&f).is_empty());
    }

    #[test]
    fn a_condition_turned_off_raises_nothing_and_what_is_open_can_still_end() {
        let f = fixture();
        sample(&f, 95);
        let off = Settings {
            host_offline: false,
            disk_usage: None,
            agent_limit: None,
            job_failed: false,
        };
        f.alerts.set_settings(&admin(), off).unwrap();
        f.bus.publish(
            Actor::System,
            &Offline {
                host_id: "host_db".into(),
                last_seen: f.clock.now(),
            },
        );
        run(&f, "failed");
        assert_eq!(summaries(&f).len(), 1);
        f.alerts
            .set_settings(
                &admin(),
                Settings {
                    disk_usage: Some(0.99),
                    ..Settings::default()
                },
            )
            .unwrap();
        sample(&f, 95);
        assert!(summaries(&f).is_empty());
    }

    #[test]
    fn a_removed_hosts_alerts_close_without_a_word() {
        let f = fixture();
        sample(&f, 95);
        f.bus.publish(
            Actor::System,
            &Offline {
                host_id: "host_other".into(),
                last_seen: f.clock.now(),
            },
        );
        f.bus.publish(
            Actor::System,
            &HostRemoved {
                host_id: "host_db".into(),
            },
        );
        assert_eq!(summaries(&f), ["host_other is offline"]);
        let announced = f.announced.0.lock().unwrap();
        assert!(announced.iter().all(|(topic, _)| topic == "alert.opened"));
        drop(announced);
        // the same condition on a later host of that id opens anew
        sample(&f, 95);
        assert_eq!(summaries(&f).len(), 2);
    }

    #[test]
    fn history_keeps_resolved_alerts_newest_first() {
        let f = fixture();
        sample(&f, 95);
        f.clock.advance(chrono::Duration::minutes(1));
        sample(&f, 10);
        f.clock.advance(chrono::Duration::minutes(1));
        run(&f, "failed");
        let history = f.alerts.history(&viewer(), 10).unwrap();
        let got: Vec<_> = history
            .iter()
            .map(|a| (a.kind, a.resolved_at.is_some()))
            .collect();
        assert_eq!(got, [(Kind::JobFailed, false), (Kind::DiskUsage, true)]);
    }

    #[test]
    fn settings_are_read_by_anyone_signed_in_and_changed_by_an_admin() {
        let f = fixture();
        assert_eq!(f.alerts.settings(&viewer()).unwrap(), Settings::default());
        assert!(matches!(
            f.alerts.set_settings(&viewer(), Settings::default()),
            Err(Error::Forbidden)
        ));
        let bad = Settings {
            disk_usage: Some(2.0),
            ..Settings::default()
        };
        assert!(matches!(
            f.alerts.set_settings(&admin(), bad),
            Err(Error::Invalid { .. })
        ));
    }
}
