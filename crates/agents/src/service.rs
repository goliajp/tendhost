use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use tendhost_kernel::{
    Actor, Bus, Clock, Envelope, Error, Principal, Result, Role, Sink, Timestamp,
};
use tendhost_store::Keyspace;

use crate::events::{LimitRead, TrackingChanged};
use crate::model::{
    AgentView, Detail, Found, Installed, KINDS, USAGE_KEPT_DAYS, UsageDay, normalize_tracked,
};
use crate::ports::KnownHosts;
use crate::repo::Repo;

pub struct Agents {
    repo: Repo,
    bus: Bus,
    clock: Arc<dyn Clock>,
    hosts: Arc<dyn KnownHosts>,
}

/// This module's view of `host.removed`.
#[derive(Deserialize)]
struct Removed {
    host_id: String,
}

impl Agents {
    pub fn new(
        keyspace: Keyspace,
        bus: Bus,
        clock: Arc<dyn Clock>,
        hosts: Arc<dyn KnownHosts>,
    ) -> Self {
        Self {
            repo: Repo::new(keyspace),
            bus,
            clock,
            hosts,
        }
    }

    /// The host reports the agents it has. Detail is kept only for tracked
    /// agents, whatever the host sent.
    pub fn record(&self, host_id: &str, at: Timestamp, found: Vec<Found>) -> Result<()> {
        let tracked = self.repo.tracked(host_id)?;
        let mut installed = Vec::new();
        for found in found
            .into_iter()
            .filter(|f| KINDS.contains(&f.agent.as_str()))
        {
            installed.push(Installed {
                agent: found.agent.clone(),
                account_count: found.account_count,
                supports_usage: found.supports_usage,
                supports_limits: found.supports_limits,
                seen_at: at,
            });
            let Some(detail) = found.detail.filter(|_| tracked.contains(&found.agent)) else {
                continue;
            };

            let mut days: BTreeMap<NaiveDate, Vec<UsageDay>> = BTreeMap::new();
            detail
                .usage
                .into_iter()
                .for_each(|row| days.entry(row.day).or_default().push(row));
            for (day, rows) in &days {
                self.repo.put_usage_day(host_id, &found.agent, *day, rows)?;
            }
            let keep_from = self.clock.now().date_naive() - Duration::days(USAGE_KEPT_DAYS);
            self.repo
                .drop_usage_before(host_id, &found.agent, keep_from)?;

            let earlier = self.repo.detail(host_id, &found.agent)?;
            let is_newer = |reading: &crate::model::LimitReading| {
                earlier
                    .limits
                    .as_ref()
                    .is_none_or(|e| reading.observed_at > e.observed_at)
            };
            let fresh = detail.limits.filter(is_newer);
            for window in fresh.iter().flat_map(|reading| &reading.windows) {
                self.bus.publish(
                    Actor::Host(host_id.to_string()),
                    &LimitRead {
                        host_id: host_id.to_string(),
                        agent: found.agent.clone(),
                        window: window.name.clone(),
                        used_ratio: window.used_ratio,
                        resets_at: window.resets_at,
                    },
                );
            }
            let limits = fresh.or(earlier.limits);
            self.repo.put_detail(
                host_id,
                &found.agent,
                &Detail {
                    accounts: detail.accounts,
                    limits,
                },
            )?;
        }
        self.repo.put_installed(host_id, &installed)
    }

    /// The agents whose detail the host should send.
    pub fn tracked(&self, host_id: &str) -> Result<Vec<String>> {
        self.repo.tracked(host_id)
    }

    /// Chooses the agents to track on a host. What was collected about an
    /// agent that is no longer tracked is deleted.
    pub fn set_tracked(
        &self,
        principal: &Principal,
        host_id: &str,
        agents: &[String],
    ) -> Result<Vec<String>> {
        principal.require(Role::Admin)?;
        if !self.hosts.exists(host_id)? {
            return Err(Error::NotFound { kind: "host" });
        }
        let agents = normalize_tracked(agents)?;
        for dropped in self
            .repo
            .tracked(host_id)?
            .iter()
            .filter(|a| !agents.contains(a))
        {
            self.repo.forget_agent(host_id, dropped)?;
        }
        self.repo.put_tracked(host_id, &agents)?;
        self.bus.publish(
            principal.actor.clone(),
            &TrackingChanged {
                host_id: host_id.to_string(),
                tracked: agents.clone(),
            },
        );
        Ok(agents)
    }

    /// Every agent on every host, grouped by host.
    pub fn overview(&self, principal: &Principal) -> Result<Vec<AgentView>> {
        principal.require(Role::Viewer)?;
        let mut views = Vec::new();
        for host_id in self.repo.hosts()? {
            let tracked = self.repo.tracked(&host_id)?;
            for installed in self.repo.installed(&host_id)? {
                let is_tracked = tracked.contains(&installed.agent);
                let detail = if is_tracked {
                    self.repo.detail(&host_id, &installed.agent)?
                } else {
                    Detail::default()
                };
                views.push(AgentView {
                    host_id: host_id.clone(),
                    installed,
                    tracked: is_tracked,
                    accounts: detail.accounts,
                    limits: detail.limits,
                });
            }
        }
        Ok(views)
    }

    /// Token totals per day and model for the last `days` days, oldest
    /// first.
    pub fn usage(
        &self,
        principal: &Principal,
        host_id: &str,
        agent: &str,
        days: u32,
    ) -> Result<Vec<UsageDay>> {
        principal.require(Role::Viewer)?;
        let since =
            self.clock.now().date_naive() - Duration::days(i64::from(days.clamp(1, 90)) - 1);
        self.repo.usage_since(host_id, agent, since)
    }
}

impl Sink for Agents {
    fn accept(&self, envelope: &Envelope) {
        if let Some(removed) = envelope.view::<Removed>("host.removed")
            && let Err(error) = self.repo.forget_host(&removed.host_id, KINDS)
        {
            tracing::error!(%error, host = %removed.host_id, "agent data of a removed host was not deleted");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Event, ManualClock};
    use tendhost_store::Store;

    use super::*;
    use crate::model::{Account, FoundDetail, LimitReading, LimitWindow};

    struct Known;

    impl KnownHosts for Known {
        fn exists(&self, host_id: &str) -> Result<bool> {
            Ok(host_id != "host_missing")
        }
    }

    #[derive(Default)]
    struct Seen(Mutex<Vec<Envelope>>);

    impl Sink for Seen {
        fn accept(&self, envelope: &Envelope) {
            self.0.lock().unwrap().push(envelope.clone());
        }
    }

    #[derive(Serialize)]
    struct HostRemoved {
        host_id: String,
    }

    impl Event for HostRemoved {
        const TOPIC: &'static str = "host.removed";
    }

    struct Fixture {
        agents: Arc<Agents>,
        bus: Bus,
        clock: Arc<ManualClock>,
        seen: Arc<Seen>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let keyspace = Store::temp().unwrap().keyspace("agents");
        let agents = Arc::new(Agents::new(
            keyspace,
            bus.clone(),
            clock.clone(),
            Arc::new(Known),
        ));
        let seen = Arc::new(Seen::default());
        bus.attach(agents.clone());
        bus.attach(seen.clone());
        Fixture {
            agents,
            bus,
            clock,
            seen,
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

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, d).unwrap()
    }

    fn row(d: u32, model: &str, output: u64) -> UsageDay {
        UsageDay {
            day: day(d),
            model: model.into(),
            input_tokens: 10,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }
    }

    fn reading(at: Timestamp, used: f32) -> LimitReading {
        let window = LimitWindow {
            name: "5h".into(),
            used_ratio: used,
            resets_at: None,
        };
        LimitReading {
            observed_at: at,
            plan: Some("pro".into()),
            windows: vec![window],
        }
    }

    fn found(agent: &str, usage: Vec<UsageDay>, limits: Option<LimitReading>) -> Found {
        let account = Account {
            dir: format!("/home/ada/.{agent}"),
            label: "ada@example.com".into(),
            plan: None,
        };
        Found {
            agent: agent.into(),
            account_count: 1,
            supports_usage: true,
            supports_limits: limits.is_some(),
            detail: Some(FoundDetail {
                accounts: vec![account],
                usage,
                limits,
            }),
        }
    }

    #[test]
    fn an_untracked_agent_is_listed_as_installed_and_nothing_else_is_kept() {
        let f = fixture();
        let now = f.clock.now();
        f.agents
            .record(
                "host_a",
                now,
                vec![found(
                    "codex",
                    vec![row(10, "gpt", 5)],
                    Some(reading(now, 0.5)),
                )],
            )
            .unwrap();
        let views = f.agents.overview(&viewer()).unwrap();
        assert_eq!(views.len(), 1);
        assert_eq!(
            (views[0].installed.agent.as_str(), views[0].tracked),
            ("codex", false)
        );
        assert!(views[0].accounts.is_empty() && views[0].limits.is_none());
        assert!(
            f.agents
                .usage(&viewer(), "host_a", "codex", 30)
                .unwrap()
                .is_empty()
        );
        assert!(f.seen.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_tracked_agent_shows_accounts_limits_and_usage() {
        let f = fixture();
        let now = f.clock.now();
        assert_eq!(
            f.agents
                .set_tracked(&admin(), "host_a", &["codex".into()])
                .unwrap(),
            ["codex"]
        );
        assert_eq!(f.agents.tracked("host_a").unwrap(), ["codex"]);
        let report = vec![
            found(
                "codex",
                vec![row(9, "gpt", 5), row(10, "gpt", 7), row(10, "mini", 1)],
                Some(reading(now, 0.5)),
            ),
            found("claude", vec![row(10, "opus", 9)], None),
        ];
        f.agents.record("host_a", now, report).unwrap();

        let views = f.agents.overview(&viewer()).unwrap();
        let codex = views.iter().find(|v| v.installed.agent == "codex").unwrap();
        assert_eq!(codex.accounts[0].label, "ada@example.com");
        assert_eq!(codex.limits.as_ref().unwrap().windows[0].used_ratio, 0.5);
        let claude = views
            .iter()
            .find(|v| v.installed.agent == "claude")
            .unwrap();
        assert!(!claude.tracked && claude.accounts.is_empty());

        let usage = f.agents.usage(&viewer(), "host_a", "codex", 30).unwrap();
        assert_eq!(
            usage,
            [row(9, "gpt", 5), row(10, "gpt", 7), row(10, "mini", 1)]
        );
        assert_eq!(
            f.agents
                .usage(&viewer(), "host_a", "codex", 1)
                .unwrap()
                .len(),
            2
        );
        assert!(
            f.agents
                .usage(&viewer(), "host_a", "claude", 30)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_later_report_replaces_its_days_and_leaves_other_days() {
        let f = fixture();
        f.agents
            .set_tracked(&admin(), "host_a", &["codex".into()])
            .unwrap();
        f.agents
            .record(
                "host_a",
                f.clock.now(),
                vec![found(
                    "codex",
                    vec![row(9, "gpt", 5), row(10, "gpt", 7)],
                    None,
                )],
            )
            .unwrap();
        f.agents
            .record(
                "host_a",
                f.clock.now(),
                vec![found("codex", vec![row(10, "gpt", 8)], None)],
            )
            .unwrap();
        let usage = f.agents.usage(&viewer(), "host_a", "codex", 30).unwrap();
        assert_eq!(usage, [row(9, "gpt", 5), row(10, "gpt", 8)]);
    }

    #[test]
    fn only_a_newer_limit_reading_replaces_the_stored_one_and_is_announced() {
        let f = fixture();
        let now = f.clock.now();
        f.agents
            .set_tracked(&admin(), "host_a", &["codex".into()])
            .unwrap();
        f.agents
            .record(
                "host_a",
                now,
                vec![found("codex", vec![], Some(reading(now, 0.5)))],
            )
            .unwrap();
        f.agents
            .record(
                "host_a",
                now,
                vec![found(
                    "codex",
                    vec![],
                    Some(reading(now - Duration::minutes(5), 0.1)),
                )],
            )
            .unwrap();
        f.agents
            .record(
                "host_a",
                now,
                vec![found("codex", vec![], Some(reading(now, 0.5)))],
            )
            .unwrap();
        f.agents
            .record("host_a", now, vec![found("codex", vec![], None)])
            .unwrap();
        let views = f.agents.overview(&viewer()).unwrap();
        assert_eq!(views[0].limits.as_ref().unwrap().windows[0].used_ratio, 0.5);

        let seen = f.seen.0.lock().unwrap();
        let reads: Vec<_> = seen
            .iter()
            .filter(|e| e.topic == "agents.limit_read")
            .collect();
        assert_eq!(reads.len(), 1);
        assert!(!reads[0].audited);
        assert_eq!(
            (
                reads[0].payload["agent"].as_str(),
                reads[0].payload["window"].as_str()
            ),
            (Some("codex"), Some("5h"))
        );
    }

    #[test]
    fn ending_tracking_deletes_what_was_collected() {
        let f = fixture();
        let now = f.clock.now();
        f.agents
            .set_tracked(&admin(), "host_a", &["codex".into(), "claude".into()])
            .unwrap();
        let report = || {
            vec![
                found("codex", vec![row(10, "gpt", 7)], Some(reading(now, 0.5))),
                found("claude", vec![row(10, "opus", 9)], None),
            ]
        };
        f.agents.record("host_a", now, report()).unwrap();

        f.agents
            .set_tracked(&admin(), "host_a", &["claude".into()])
            .unwrap();
        assert!(
            f.agents
                .usage(&viewer(), "host_a", "codex", 30)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            f.agents
                .usage(&viewer(), "host_a", "claude", 30)
                .unwrap()
                .len(),
            1
        );
        // tracking again starts from nothing
        f.agents
            .set_tracked(&admin(), "host_a", &["claude".into(), "codex".into()])
            .unwrap();
        let views = f.agents.overview(&viewer()).unwrap();
        let codex = views.iter().find(|v| v.installed.agent == "codex").unwrap();
        assert!(codex.tracked && codex.accounts.is_empty() && codex.limits.is_none());
    }

    #[test]
    fn usage_older_than_the_retention_is_dropped() {
        let f = fixture();
        f.agents
            .set_tracked(&admin(), "host_a", &["codex".into()])
            .unwrap();
        let old = UsageDay {
            day: f.clock.now().date_naive() - Duration::days(USAGE_KEPT_DAYS + 1),
            ..row(10, "gpt", 1)
        };
        let kept = UsageDay {
            day: f.clock.now().date_naive() - Duration::days(USAGE_KEPT_DAYS),
            ..row(10, "gpt", 2)
        };
        f.agents
            .record(
                "host_a",
                f.clock.now(),
                vec![found("codex", vec![old, kept.clone()], None)],
            )
            .unwrap();
        assert_eq!(f.repo_usage_all("host_a", "codex"), [kept]);
    }

    #[test]
    fn tracking_needs_an_admin_a_real_host_and_known_agents() {
        let f = fixture();
        assert!(matches!(
            f.agents.set_tracked(&viewer(), "host_a", &[]),
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            f.agents.set_tracked(&admin(), "host_missing", &[]),
            Err(Error::NotFound { .. })
        ));
        assert!(matches!(
            f.agents.set_tracked(&admin(), "host_a", &["cursor".into()]),
            Err(Error::Invalid { .. })
        ));
        f.agents
            .set_tracked(&admin(), "host_a", &["kimi".into()])
            .unwrap();
        let seen = f.seen.0.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            (seen[0].topic.as_str(), seen[0].audited),
            ("agents.tracking_changed", true)
        );
    }

    #[test]
    fn an_agent_kind_this_server_does_not_know_is_ignored() {
        let f = fixture();
        f.agents
            .record(
                "host_a",
                f.clock.now(),
                vec![found("cursor", vec![], None), found("kimi", vec![], None)],
            )
            .unwrap();
        let agents: Vec<_> = f
            .agents
            .overview(&viewer())
            .unwrap()
            .into_iter()
            .map(|v| v.installed.agent)
            .collect();
        assert_eq!(agents, ["kimi"]);
    }

    #[test]
    fn a_removed_host_leaves_nothing_behind() {
        let f = fixture();
        f.agents
            .set_tracked(&admin(), "host_a", &["codex".into()])
            .unwrap();
        f.agents
            .record(
                "host_a",
                f.clock.now(),
                vec![found("codex", vec![row(10, "gpt", 7)], None)],
            )
            .unwrap();
        f.bus.publish(
            Actor::System,
            &HostRemoved {
                host_id: "host_a".into(),
            },
        );
        assert!(f.agents.overview(&viewer()).unwrap().is_empty());
        assert!(f.agents.tracked("host_a").unwrap().is_empty());
        assert!(
            f.agents
                .usage(&viewer(), "host_a", "codex", 90)
                .unwrap()
                .is_empty()
        );
    }

    impl Fixture {
        fn repo_usage_all(&self, host_id: &str, agent: &str) -> Vec<UsageDay> {
            self.agents
                .repo
                .usage_since(host_id, agent, NaiveDate::MIN)
                .unwrap()
        }
    }
}
