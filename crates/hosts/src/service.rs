use std::sync::Arc;

use tendhost_kernel::{Actor, Bus, Clock, Error, Principal, Result, Role, Timestamp, id, secret};
use tendhost_store::Keyspace;

use crate::events::{EnrollmentOpened, HostEnrolled, HostRemoved, HostUpdated};
use crate::model::{
    ENROLLMENT_LIFETIME, Enrollment, Facts, Host, Mode, NewEnrollment, normalize_labels,
    normalize_name,
};
use crate::repo::Repo;

pub struct Hosts {
    repo: Repo,
    bus: Bus,
    clock: Arc<dyn Clock>,
}

impl Hosts {
    pub fn new(keyspace: Keyspace, bus: Bus, clock: Arc<dyn Clock>) -> Self {
        Self {
            repo: Repo::new(keyspace),
            bus,
            clock,
        }
    }

    /// Issues a token that lets one machine enroll within the next
    /// [`ENROLLMENT_LIFETIME`].
    pub fn open_enrollment(&self, principal: &Principal) -> Result<NewEnrollment> {
        principal.require(Role::Admin)?;
        let now = self.clock.now();
        let token = secret::generate("the");
        let enrollment = Enrollment {
            id: id::new("enr", now),
            opened_by: principal.actor.clone(),
            expires_at: now + ENROLLMENT_LIFETIME,
        };
        self.repo
            .put_enrollment(&secret::fingerprint(&token), &enrollment)?;
        self.bus.publish(
            principal.actor.clone(),
            &EnrollmentOpened {
                enrollment_id: enrollment.id,
            },
        );
        Ok(NewEnrollment {
            token,
            expires_at: enrollment.expires_at,
        })
    }

    /// Admits the machine holding `token`. Returns the host and the
    /// credential it presents from now on. A token admits one machine.
    pub fn enroll(
        &self,
        token: &str,
        name: &str,
        mode: Mode,
        facts: Facts,
    ) -> Result<(Host, String)> {
        let name = normalize_name(name)?;
        let fingerprint = secret::fingerprint(token.trim());
        let now = self.clock.now();
        let refused = || Error::invalid("token", "is not a valid enrollment token; open a new one");
        let enrollment = self.repo.enrollment(&fingerprint)?.ok_or_else(refused)?;
        if enrollment.expires_at <= now {
            self.repo.claim_enrollment(&fingerprint)?;
            return Err(Error::invalid("token", "has expired; open a new one"));
        }
        if !self.repo.claim_enrollment(&fingerprint)? {
            return Err(refused());
        }

        let host_id = id::new("host", now);
        let name = self.claim_free_name(&name, &host_id)?;
        let host = Host {
            id: host_id,
            name,
            mode,
            labels: Vec::new(),
            enrolled_at: now,
            facts,
            facts_at: now,
        };
        self.repo.put_host(&host)?;
        let credential = secret::generate("thh");
        self.repo
            .put_credential(&secret::fingerprint(&credential), &host.id)?;
        self.bus.publish(
            Actor::Host(host.id.clone()),
            &HostEnrolled {
                host_id: host.id.clone(),
                name: host.name.clone(),
                mode,
                enrollment_id: enrollment.id,
            },
        );
        Ok((host, credential))
    }

    /// The host a credential belongs to.
    pub fn authenticate(&self, credential: &str) -> Result<String> {
        self.repo
            .host_id_of_credential(&secret::fingerprint(credential))?
            .ok_or(Error::Unauthenticated)
    }

    /// Replaces what the host says about itself.
    pub fn record_facts(&self, host_id: &str, facts: Facts, at: Timestamp) -> Result<()> {
        let mut host = self
            .repo
            .host(host_id)?
            .ok_or(Error::NotFound { kind: "host" })?;
        if at < host.facts_at {
            return Ok(());
        }
        host.facts = facts;
        host.facts_at = at;
        self.repo.put_host(&host)
    }

    /// What the host lets the server do. For other parts of the server,
    /// not for a caller of the api.
    pub fn mode(&self, host_id: &str) -> Result<Mode> {
        Ok(self
            .repo
            .host(host_id)?
            .ok_or(Error::NotFound { kind: "host" })?
            .mode)
    }

    pub fn hosts(&self, principal: &Principal) -> Result<Vec<Host>> {
        principal.require(Role::Viewer)?;
        self.repo.hosts()
    }

    pub fn host(&self, principal: &Principal, host_id: &str) -> Result<Host> {
        principal.require(Role::Viewer)?;
        self.repo
            .host(host_id)?
            .ok_or(Error::NotFound { kind: "host" })
    }

    /// Renames and/or relabels; an absent argument leaves that part as is.
    pub fn update(
        &self,
        principal: &Principal,
        host_id: &str,
        name: Option<&str>,
        labels: Option<&[String]>,
    ) -> Result<Host> {
        principal.require(Role::Admin)?;
        let mut host = self
            .repo
            .host(host_id)?
            .ok_or(Error::NotFound { kind: "host" })?;
        let labels = labels.map(normalize_labels).transpose()?;
        if let Some(name) = name.map(normalize_name).transpose()?
            && name != host.name
        {
            if !self.repo.claim_name(&name, &host.id)? {
                return Err(Error::Conflict(format!("another host is called {name}")));
            }
            self.repo.release_name(&host.name)?;
            host.name = name;
        }
        if let Some(labels) = labels {
            host.labels = labels;
        }
        self.repo.put_host(&host)?;
        self.bus.publish(
            principal.actor.clone(),
            &HostUpdated {
                host_id: host.id.clone(),
                name: host.name.clone(),
                labels: host.labels.clone(),
            },
        );
        Ok(host)
    }

    /// Forgets the host. Its credential stops working at once.
    pub fn remove(&self, principal: &Principal, host_id: &str) -> Result<()> {
        principal.require(Role::Admin)?;
        let host = self
            .repo
            .host(host_id)?
            .ok_or(Error::NotFound { kind: "host" })?;
        self.repo.remove_host(&host)?;
        self.bus.publish(
            principal.actor.clone(),
            &HostRemoved {
                host_id: host.id,
                name: host.name,
            },
        );
        Ok(())
    }

    /// `name`, or `name-2`, `name-3`, … when taken: two machines may well
    /// report the same hostname.
    fn claim_free_name(&self, name: &str, host_id: &str) -> Result<String> {
        if self.repo.claim_name(name, host_id)? {
            return Ok(name.to_string());
        }
        for n in 2..1000 {
            let candidate = format!("{name}-{n}");
            if self.repo.claim_name(&candidate, host_id)? {
                return Ok(candidate);
            }
        }
        Err(Error::Conflict(format!("too many hosts are called {name}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{Duration, TimeZone, Utc};
    use tendhost_kernel::{Envelope, ManualClock, Sink};
    use tendhost_store::Store;

    use super::*;

    #[derive(Default)]
    struct Seen(Mutex<Vec<Envelope>>);

    impl Sink for Seen {
        fn accept(&self, envelope: &Envelope) {
            self.0.lock().unwrap().push(envelope.clone());
        }
    }

    struct Fixture {
        hosts: Hosts,
        clock: Arc<ManualClock>,
        seen: Arc<Seen>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let seen = Arc::new(Seen::default());
        bus.attach(seen.clone());
        let hosts = Hosts::new(Store::temp().unwrap().keyspace("hosts"), bus, clock.clone());
        Fixture { hosts, clock, seen }
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

    fn facts(hostname: &str) -> Facts {
        Facts {
            hostname: hostname.into(),
            os: "linux".into(),
            os_version: "6.12".into(),
            arch: "x86_64".into(),
            cpu_count: 8,
            memory_bytes: 16 << 30,
            daemon_version: "0.1.0".into(),
        }
    }

    fn enroll(f: &Fixture, name: &str) -> (Host, String) {
        let token = f.hosts.open_enrollment(&admin()).unwrap().token;
        f.hosts
            .enroll(&token, name, Mode::Observe, facts(name))
            .unwrap()
    }

    #[test]
    fn a_token_admits_one_machine_and_its_credential_identifies_it() {
        let f = fixture();
        let opened = f.hosts.open_enrollment(&admin()).unwrap();
        let (host, credential) = f
            .hosts
            .enroll(&opened.token, "Web 01", Mode::Manage, facts("web01"))
            .unwrap();
        assert_eq!((host.name.as_str(), host.mode), ("web-01", Mode::Manage));
        assert_eq!(f.hosts.authenticate(&credential).unwrap(), host.id);
        assert!(matches!(
            f.hosts.authenticate("thh_guess"),
            Err(Error::Unauthenticated)
        ));
        assert!(matches!(
            f.hosts
                .enroll(&opened.token, "web02", Mode::Observe, facts("web02")),
            Err(Error::Invalid { .. })
        ));
        assert_eq!(f.hosts.hosts(&viewer()).unwrap(), [host]);
    }

    #[test]
    fn an_enrollment_token_expires() {
        let f = fixture();
        let opened = f.hosts.open_enrollment(&admin()).unwrap();
        f.clock.advance(ENROLLMENT_LIFETIME);
        let refused = f
            .hosts
            .enroll(&opened.token, "a", Mode::Observe, facts("a"))
            .unwrap_err();
        assert!(refused.to_string().contains("expired"));
        assert!(f.hosts.hosts(&viewer()).unwrap().is_empty());

        let opened = f.hosts.open_enrollment(&admin()).unwrap();
        f.clock.advance(ENROLLMENT_LIFETIME - Duration::seconds(1));
        assert!(
            f.hosts
                .enroll(&opened.token, "a", Mode::Observe, facts("a"))
                .is_ok()
        );
    }

    #[test]
    fn a_refused_name_does_not_use_up_the_token() {
        let f = fixture();
        let opened = f.hosts.open_enrollment(&admin()).unwrap();
        assert!(
            f.hosts
                .enroll(&opened.token, "!!!", Mode::Observe, facts("x"))
                .is_err()
        );
        assert!(
            f.hosts
                .enroll(&opened.token, "x", Mode::Observe, facts("x"))
                .is_ok()
        );
    }

    #[test]
    fn machines_with_the_same_hostname_get_distinct_names() {
        let f = fixture();
        let names: Vec<_> = (0..3).map(|_| enroll(&f, "macbook").0.name).collect();
        assert_eq!(names, ["macbook", "macbook-2", "macbook-3"]);
    }

    #[test]
    fn only_an_admin_opens_enrollment_or_changes_hosts() {
        let f = fixture();
        let (host, _) = enroll(&f, "a");
        assert!(matches!(
            f.hosts.open_enrollment(&viewer()),
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            f.hosts.update(&viewer(), &host.id, Some("b"), None),
            Err(Error::Forbidden)
        ));
        assert!(matches!(
            f.hosts.remove(&viewer(), &host.id),
            Err(Error::Forbidden)
        ));
        assert!(f.hosts.host(&viewer(), &host.id).is_ok());
    }

    #[test]
    fn renaming_frees_the_old_name_and_refuses_a_taken_one() {
        let f = fixture();
        let (a, _) = enroll(&f, "a");
        let (b, _) = enroll(&f, "b");
        assert!(matches!(
            f.hosts.update(&admin(), &b.id, Some("A"), None),
            Err(Error::Conflict(_))
        ));
        assert_eq!(
            f.hosts
                .update(&admin(), &a.id, Some("c"), None)
                .unwrap()
                .name,
            "c"
        );
        assert_eq!(
            f.hosts
                .update(&admin(), &b.id, Some("a"), None)
                .unwrap()
                .name,
            "a"
        );
        assert_eq!(
            f.hosts
                .update(&admin(), &b.id, Some("a"), None)
                .unwrap()
                .name,
            "a"
        );
    }

    #[test]
    fn labels_change_without_touching_the_name() {
        let f = fixture();
        let (host, _) = enroll(&f, "a");
        let labels = ["web".to_string(), " db ".to_string()];
        let updated = f
            .hosts
            .update(&admin(), &host.id, None, Some(&labels))
            .unwrap();
        assert_eq!(
            (updated.name.as_str(), updated.labels.as_slice()),
            ("a", &["db".to_string(), "web".to_string()][..])
        );
        assert!(
            f.hosts
                .update(&admin(), &host.id, Some("b"), Some(&["".to_string()]))
                .is_err()
        );
        assert_eq!(f.hosts.host(&viewer(), &host.id).unwrap().name, "a");
    }

    #[test]
    fn a_removed_host_is_gone_with_its_credential_and_its_name() {
        let f = fixture();
        let (host, credential) = enroll(&f, "a");
        f.hosts.remove(&admin(), &host.id).unwrap();
        assert!(matches!(
            f.hosts.authenticate(&credential),
            Err(Error::Unauthenticated)
        ));
        assert!(matches!(
            f.hosts.host(&viewer(), &host.id),
            Err(Error::NotFound { .. })
        ));
        assert!(matches!(
            f.hosts.remove(&admin(), &host.id),
            Err(Error::NotFound { .. })
        ));
        assert_eq!(enroll(&f, "a").0.name, "a");
    }

    #[test]
    fn newer_facts_replace_older_ones_and_stale_reports_are_ignored() {
        let f = fixture();
        let (host, _) = enroll(&f, "a");
        let later = host.facts_at + Duration::minutes(5);
        let mut upgraded = facts("a");
        upgraded.os_version = "6.13".into();
        f.hosts
            .record_facts(&host.id, upgraded.clone(), later)
            .unwrap();
        f.hosts
            .record_facts(&host.id, facts("a"), later - Duration::minutes(1))
            .unwrap();
        let stored = f.hosts.host(&viewer(), &host.id).unwrap();
        assert_eq!((stored.facts, stored.facts_at), (upgraded, later));
        assert!(matches!(
            f.hosts.record_facts("host_missing", facts("a"), later),
            Err(Error::NotFound { .. })
        ));
    }

    #[test]
    fn changes_are_published_with_who_made_them() {
        let f = fixture();
        let (host, _) = enroll(&f, "a");
        f.hosts.update(&admin(), &host.id, Some("b"), None).unwrap();
        f.hosts.remove(&admin(), &host.id).unwrap();
        let seen = f.seen.0.lock().unwrap();
        let topics: Vec<_> = seen.iter().map(|e| e.topic.as_str()).collect();
        assert_eq!(
            topics,
            [
                "host.enrollment_opened",
                "host.enrolled",
                "host.updated",
                "host.removed"
            ]
        );
        assert_eq!(seen[1].actor, Actor::Host(host.id.clone()));
        assert_eq!(seen[3].actor, admin().actor);
        assert_eq!(seen[3].payload["name"], "b");
    }
}
