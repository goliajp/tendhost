use std::sync::Arc;

use serde::Deserialize;
use tendhost_kernel::{Actor, Bus, Clock, Envelope, Error, Principal, Result, Role, Sink, id};
use tendhost_store::Keyspace;

use crate::events::{JobChanged, JobCreated, JobRemoved, RunFinished, RunRequested};
use crate::model::{
    Assignment, COLLECT_WITHIN, Job, LastRun, NewJob, Outcome, REPORT_GRACE, Run, RunState, tail,
};
use crate::ports::HostDirectory;
use crate::repo::Repo;

pub struct Jobs {
    repo: Repo,
    bus: Bus,
    clock: Arc<dyn Clock>,
    hosts: Arc<dyn HostDirectory>,
}

/// This module's view of `host.removed`.
#[derive(Deserialize)]
struct Removed {
    host_id: String,
}

impl Jobs {
    pub fn new(
        keyspace: Keyspace,
        bus: Bus,
        clock: Arc<dyn Clock>,
        hosts: Arc<dyn HostDirectory>,
    ) -> Self {
        Self {
            repo: Repo::new(keyspace),
            bus,
            clock,
            hosts,
        }
    }

    pub fn create(&self, principal: &Principal, new: NewJob) -> Result<Job> {
        principal.require(Role::Admin)?;
        let now = self.clock.now();
        let new = new.validated(now)?;
        self.require_commands_allowed(&new.host_id)?;
        let job = Job {
            id: id::new("job", now),
            next_run_at: new.schedule.next_after(now)?,
            name: new.name,
            host_id: new.host_id,
            command: new.command,
            timeout_secs: new.timeout_secs,
            schedule: new.schedule,
            enabled: true,
            created_by: principal.actor.clone(),
            created_at: now,
            last_run: None,
        };
        self.repo.put_job(&job)?;
        self.bus.publish(
            principal.actor.clone(),
            &JobCreated {
                job_id: job.id.clone(),
                name: job.name.clone(),
                host_id: job.host_id.clone(),
                command: job.command.clone(),
            },
        );
        Ok(job)
    }

    /// Every job, oldest first; only those of `host_id` when given.
    pub fn jobs(&self, principal: &Principal, host_id: Option<&str>) -> Result<Vec<Job>> {
        principal.require(Role::Viewer)?;
        let mut jobs = self.repo.jobs()?;
        jobs.retain(|job| host_id.is_none_or(|host| job.host_id == host));
        Ok(jobs)
    }

    pub fn job(&self, principal: &Principal, job_id: &str) -> Result<Job> {
        principal.require(Role::Viewer)?;
        self.repo
            .job(job_id)?
            .ok_or(Error::NotFound { kind: "job" })
    }

    /// A disabled job keeps its runs but is never due, and cannot be run
    /// by hand either.
    pub fn set_enabled(&self, principal: &Principal, job_id: &str, enabled: bool) -> Result<Job> {
        principal.require(Role::Admin)?;
        let mut job = self
            .repo
            .job(job_id)?
            .ok_or(Error::NotFound { kind: "job" })?;
        job.enabled = enabled;
        job.next_run_at = if enabled {
            job.schedule.next_after(self.clock.now())?
        } else {
            None
        };
        self.repo.put_job(&job)?;
        self.bus.publish(
            principal.actor.clone(),
            &JobChanged {
                job_id: job.id.clone(),
                name: job.name.clone(),
                enabled,
            },
        );
        Ok(job)
    }

    pub fn remove(&self, principal: &Principal, job_id: &str) -> Result<()> {
        principal.require(Role::Admin)?;
        let job = self
            .repo
            .job(job_id)?
            .ok_or(Error::NotFound { kind: "job" })?;
        self.repo.remove_job(&job)?;
        self.bus.publish(
            principal.actor.clone(),
            &JobRemoved {
                job_id: job.id,
                name: job.name,
            },
        );
        Ok(())
    }

    /// Asks for a run now, outside the schedule.
    pub fn request_run(&self, principal: &Principal, job_id: &str) -> Result<Run> {
        principal.require(Role::Admin)?;
        let job = self
            .repo
            .job(job_id)?
            .ok_or(Error::NotFound { kind: "job" })?;
        if !job.enabled {
            return Err(Error::Conflict("the job is disabled".into()));
        }
        self.require_commands_allowed(&job.host_id)?;
        let run = self.new_run(&job, principal.actor.clone())?;
        self.bus.publish(
            principal.actor.clone(),
            &RunRequested {
                job_id: job.id,
                run_id: run.id.clone(),
                host_id: run.host_id.clone(),
            },
        );
        Ok(run)
    }

    /// A job's runs, newest first.
    pub fn runs(&self, principal: &Principal, job_id: &str, limit: usize) -> Result<Vec<Run>> {
        principal.require(Role::Viewer)?;
        if self.repo.job(job_id)?.is_none() {
            return Err(Error::NotFound { kind: "job" });
        }
        self.repo.runs_of(job_id, limit)
    }

    /// Run on a timer. Queues a run for every job whose time has come, and
    /// closes runs that can no longer end by themselves. A job whose time
    /// passed while the server was down gets one run, not one per missed
    /// occurrence.
    pub fn tick(&self) -> Result<()> {
        let now = self.clock.now();
        for mut job in self.repo.due_jobs(now.timestamp_millis())? {
            self.new_run(&job, Actor::System)?;
            job.next_run_at = job.schedule.next_after(now)?;
            self.repo.put_job(&job)?;
        }
        for run in self.repo.open_runs()? {
            let Some(job) = self.repo.job(&run.job_id)? else {
                continue;
            };
            let limit = chrono::Duration::seconds(i64::from(job.timeout_secs)) + REPORT_GRACE;
            let closed = match (run.state, run.started_at) {
                (RunState::Pending, _) if run.due_at + COLLECT_WITHIN <= now => RunState::Missed,
                (RunState::Running, Some(started)) if started + limit <= now => RunState::Lost,
                _ => continue,
            };
            self.close(
                &job,
                Run {
                    state: closed,
                    finished_at: Some(now),
                    ..run
                },
            )?;
        }
        Ok(())
    }

    /// The runs `host_id` should start now. Each is handed out once.
    pub fn collect(&self, host_id: &str) -> Result<Vec<Assignment>> {
        let now = self.clock.now();
        let mut assignments = Vec::new();
        for run in self.repo.pending_for(host_id)? {
            let Some(job) = self.repo.job(&run.job_id)? else {
                continue;
            };
            self.repo.put_run(&Run {
                state: RunState::Running,
                started_at: Some(now),
                ..run.clone()
            })?;
            assignments.push(Assignment {
                run_id: run.id,
                command: job.command,
                timeout_secs: job.timeout_secs,
            });
        }
        Ok(assignments)
    }

    /// `host_id` reports how a run ended. Reporting the same end again, or
    /// the end of a run that no longer exists, changes nothing.
    pub fn finish(&self, host_id: &str, outcome: Outcome) -> Result<()> {
        let Some(run) = self.repo.run(&outcome.run_id)? else {
            return Ok(());
        };
        if run.host_id != host_id {
            return Err(Error::Forbidden);
        }
        if run.state.is_over() {
            return Ok(());
        }
        let Some(job) = self.repo.job(&run.job_id)? else {
            return Ok(());
        };
        self.close(
            &job,
            Run {
                state: outcome.state(),
                started_at: Some(outcome.started_at),
                finished_at: Some(outcome.finished_at),
                exit_code: outcome.exit_code,
                output: tail(&outcome.output),
                ..run
            },
        )
    }

    fn new_run(&self, job: &Job, requested_by: Actor) -> Result<Run> {
        let now = self.clock.now();
        let run = Run {
            id: id::new("run", now),
            job_id: job.id.clone(),
            host_id: job.host_id.clone(),
            state: RunState::Pending,
            due_at: now,
            requested_by,
            started_at: None,
            finished_at: None,
            exit_code: None,
            output: String::new(),
        };
        self.repo.put_run(&run)?;
        Ok(run)
    }

    fn close(&self, job: &Job, run: Run) -> Result<()> {
        self.repo.put_run(&run)?;
        // read again: the tick may have moved the job's next run meanwhile
        if let (Some(mut job), Some(finished_at)) = (self.repo.job(&job.id)?, run.finished_at) {
            job.last_run = Some(LastRun {
                state: run.state,
                finished_at,
            });
            self.repo.put_job(&job)?;
        }
        self.bus.publish(
            Actor::Host(run.host_id.clone()),
            &RunFinished {
                job_id: job.id.clone(),
                job_name: job.name.clone(),
                run_id: run.id,
                host_id: run.host_id,
                state: run.state,
            },
        );
        Ok(())
    }

    fn require_commands_allowed(&self, host_id: &str) -> Result<()> {
        if self.hosts.accepts_commands(host_id)? {
            return Ok(());
        }
        Err(Error::invalid(
            "host_id",
            "this host only reports; to run commands on it, enroll it again with --manage",
        ))
    }

    fn forget_host(&self, host_id: &str) -> Result<()> {
        for job in self
            .repo
            .jobs()?
            .into_iter()
            .filter(|job| job.host_id == host_id)
        {
            self.repo.remove_job(&job)?;
        }
        Ok(())
    }
}

impl Sink for Jobs {
    fn accept(&self, envelope: &Envelope) {
        if let Some(removed) = envelope.view::<Removed>("host.removed")
            && let Err(error) = self.forget_host(&removed.host_id)
        {
            tracing::error!(%error, host = %removed.host_id, "jobs of a removed host were not deleted");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use chrono::{Duration, TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Event, ManualClock, Timestamp};
    use tendhost_store::Store;

    use super::*;
    use crate::model::{RUNS_KEPT_PER_JOB, Schedule};

    /// hosts by id → whether they accept commands
    struct Directory(HashMap<&'static str, bool>);

    impl HostDirectory for Directory {
        fn accepts_commands(&self, host_id: &str) -> Result<bool> {
            self.0
                .get(host_id)
                .copied()
                .ok_or(Error::NotFound { kind: "host" })
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
        jobs: Arc<Jobs>,
        bus: Bus,
        clock: Arc<ManualClock>,
        seen: Arc<Seen>,
    }

    fn fixture() -> Fixture {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let directory = Directory(HashMap::from([
            ("host_m", true),
            ("host_m2", true),
            ("host_o", false),
        ]));
        let keyspace = Store::temp().unwrap().keyspace("jobs");
        let jobs = Arc::new(Jobs::new(
            keyspace,
            bus.clone(),
            clock.clone(),
            Arc::new(directory),
        ));
        let seen = Arc::new(Seen::default());
        bus.attach(jobs.clone());
        bus.attach(seen.clone());
        Fixture {
            jobs,
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

    fn new_job(host_id: &str, schedule: Schedule) -> NewJob {
        NewJob {
            name: "backup".into(),
            host_id: host_id.into(),
            command: "echo hi".into(),
            timeout_secs: 60,
            schedule,
        }
    }

    fn outcome(run_id: &str, exit_code: Option<i32>, at: Timestamp) -> Outcome {
        Outcome {
            run_id: run_id.into(),
            exit_code,
            timed_out: false,
            output: "hi\n".into(),
            started_at: at,
            finished_at: at + Duration::seconds(1),
        }
    }

    fn states(f: &Fixture, job_id: &str) -> Vec<RunState> {
        f.jobs
            .runs(&viewer(), job_id, 100)
            .unwrap()
            .iter()
            .map(|r| r.state)
            .collect()
    }

    #[test]
    fn a_job_needs_a_host_that_accepts_commands() {
        let f = fixture();
        let refused = f
            .jobs
            .create(&admin(), new_job("host_o", Schedule::Manual))
            .unwrap_err();
        assert!(refused.to_string().contains("--manage"));
        assert!(matches!(
            f.jobs.create(&admin(), new_job("host_x", Schedule::Manual)),
            Err(Error::NotFound { kind: "host" })
        ));
        assert!(matches!(
            f.jobs
                .create(&viewer(), new_job("host_m", Schedule::Manual)),
            Err(Error::Forbidden)
        ));
        assert!(
            f.jobs
                .create(&admin(), new_job("host_m", Schedule::Manual))
                .is_ok()
        );
    }

    #[test]
    fn a_requested_run_goes_to_its_host_once_and_ends_as_reported() {
        let f = fixture();
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        let run = f.jobs.request_run(&admin(), &job.id).unwrap();
        assert_eq!(run.state, RunState::Pending);
        assert!(f.jobs.collect("host_m2").unwrap().is_empty());

        let handed = f.jobs.collect("host_m").unwrap();
        assert_eq!(
            handed,
            [Assignment {
                run_id: run.id.clone(),
                command: "echo hi".into(),
                timeout_secs: 60
            }]
        );
        assert!(f.jobs.collect("host_m").unwrap().is_empty());
        assert_eq!(states(&f, &job.id), [RunState::Running]);

        f.jobs
            .finish("host_m", outcome(&run.id, Some(0), f.clock.now()))
            .unwrap();
        let stored = &f.jobs.runs(&viewer(), &job.id, 10).unwrap()[0];
        assert_eq!(
            (stored.state, stored.exit_code, stored.output.as_str()),
            (RunState::Succeeded, Some(0), "hi\n")
        );
        assert_eq!(stored.requested_by, admin().actor);
        assert_eq!(
            f.jobs
                .job(&viewer(), &job.id)
                .unwrap()
                .last_run
                .unwrap()
                .state,
            RunState::Succeeded
        );
    }

    #[test]
    fn a_result_is_taken_once_and_only_from_the_runs_own_host() {
        let f = fixture();
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        let run = f.jobs.request_run(&admin(), &job.id).unwrap();
        f.jobs.collect("host_m").unwrap();
        assert!(matches!(
            f.jobs
                .finish("host_m2", outcome(&run.id, Some(0), f.clock.now())),
            Err(Error::Forbidden)
        ));
        f.jobs
            .finish("host_m", outcome(&run.id, Some(2), f.clock.now()))
            .unwrap();
        f.jobs
            .finish("host_m", outcome(&run.id, Some(0), f.clock.now()))
            .unwrap();
        f.jobs
            .finish("host_m", outcome("run_unknown", Some(0), f.clock.now()))
            .unwrap();
        assert_eq!(states(&f, &job.id), [RunState::Failed]);
    }

    #[test]
    fn a_cron_job_gets_a_run_each_time_it_is_due_and_one_after_downtime() {
        let f = fixture();
        let job = f
            .jobs
            .create(
                &admin(),
                new_job(
                    "host_m",
                    Schedule::Cron {
                        expr: "*/10 * * * *".into(),
                    },
                ),
            )
            .unwrap();
        assert_eq!(job.next_run_at, Some(f.clock.now() + Duration::minutes(10)));
        f.clock.advance(Duration::minutes(9));
        f.jobs.tick().unwrap();
        assert!(states(&f, &job.id).is_empty());
        f.clock.advance(Duration::minutes(1));
        f.jobs.tick().unwrap();
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id).len(), 1);
        // the server was down across five occurrences
        f.clock.advance(Duration::minutes(55));
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id).len(), 2);
        let next = f.jobs.job(&viewer(), &job.id).unwrap().next_run_at.unwrap();
        assert_eq!(next, f.clock.now() + Duration::minutes(5));
    }

    #[test]
    fn a_one_time_job_runs_once() {
        let f = fixture();
        let at = f.clock.now() + Duration::hours(1);
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Once { at }))
            .unwrap();
        f.clock.advance(Duration::hours(2));
        f.jobs.tick().unwrap();
        f.clock.advance(Duration::hours(2));
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id).len(), 1);
        assert_eq!(f.jobs.job(&viewer(), &job.id).unwrap().next_run_at, None);
    }

    #[test]
    fn a_disabled_job_is_never_due_and_cannot_be_run_by_hand() {
        let f = fixture();
        let job = f
            .jobs
            .create(
                &admin(),
                new_job(
                    "host_m",
                    Schedule::Cron {
                        expr: "* * * * *".into(),
                    },
                ),
            )
            .unwrap();
        assert_eq!(
            f.jobs
                .set_enabled(&admin(), &job.id, false)
                .unwrap()
                .next_run_at,
            None
        );
        f.clock.advance(Duration::minutes(5));
        f.jobs.tick().unwrap();
        assert!(states(&f, &job.id).is_empty());
        assert!(matches!(
            f.jobs.request_run(&admin(), &job.id),
            Err(Error::Conflict(_))
        ));

        let enabled = f.jobs.set_enabled(&admin(), &job.id, true).unwrap();
        assert_eq!(
            enabled.next_run_at,
            Some(f.clock.now() + Duration::minutes(1))
        );
    }

    #[test]
    fn runs_nobody_collects_or_reports_are_closed() {
        let f = fixture();
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        f.jobs.request_run(&admin(), &job.id).unwrap();
        f.clock.advance(COLLECT_WITHIN - Duration::seconds(1));
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id), [RunState::Pending]);
        f.clock.advance(Duration::seconds(1));
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id), [RunState::Missed]);
        assert!(f.jobs.collect("host_m").unwrap().is_empty());

        let run = f.jobs.request_run(&admin(), &job.id).unwrap();
        f.jobs.collect("host_m").unwrap();
        f.clock.advance(Duration::seconds(60) + REPORT_GRACE);
        f.jobs.tick().unwrap();
        assert_eq!(states(&f, &job.id), [RunState::Lost, RunState::Missed]);
        // a late report does not rewrite the record
        f.jobs
            .finish("host_m", outcome(&run.id, Some(0), f.clock.now()))
            .unwrap();
        assert_eq!(states(&f, &job.id), [RunState::Lost, RunState::Missed]);
    }

    #[test]
    fn only_the_newest_runs_of_a_job_are_kept() {
        let f = fixture();
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        for _ in 0..RUNS_KEPT_PER_JOB + 3 {
            f.clock.advance(Duration::seconds(1));
            let run = f.jobs.request_run(&admin(), &job.id).unwrap();
            f.jobs.collect("host_m").unwrap();
            f.jobs
                .finish("host_m", outcome(&run.id, Some(0), f.clock.now()))
                .unwrap();
        }
        let runs = f.jobs.runs(&viewer(), &job.id, 1000).unwrap();
        assert_eq!(runs.len(), RUNS_KEPT_PER_JOB);
        assert_eq!(runs[0].due_at, f.clock.now());
    }

    #[test]
    fn removing_a_job_or_its_host_removes_its_runs() {
        let f = fixture();
        let a = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        let b = f
            .jobs
            .create(&admin(), new_job("host_m2", Schedule::Manual))
            .unwrap();
        f.jobs.request_run(&admin(), &a.id).unwrap();
        f.jobs.request_run(&admin(), &b.id).unwrap();

        f.jobs.remove(&admin(), &a.id).unwrap();
        assert!(f.jobs.collect("host_m").unwrap().is_empty());
        assert!(matches!(
            f.jobs.runs(&viewer(), &a.id, 10),
            Err(Error::NotFound { .. })
        ));

        f.bus.publish(
            Actor::System,
            &HostRemoved {
                host_id: "host_m2".into(),
            },
        );
        assert!(f.jobs.jobs(&viewer(), None).unwrap().is_empty());
        assert!(f.jobs.collect("host_m2").unwrap().is_empty());
        f.clock.advance(COLLECT_WITHIN);
        f.jobs.tick().unwrap();
    }

    #[test]
    fn jobs_list_by_host() {
        let f = fixture();
        f.jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        f.jobs
            .create(&admin(), new_job("host_m2", Schedule::Manual))
            .unwrap();
        assert_eq!(f.jobs.jobs(&viewer(), None).unwrap().len(), 2);
        let of_m2 = f.jobs.jobs(&viewer(), Some("host_m2")).unwrap();
        assert_eq!(
            of_m2.iter().map(|j| j.host_id.as_str()).collect::<Vec<_>>(),
            ["host_m2"]
        );
    }

    #[test]
    fn what_people_do_is_audited_and_run_results_are_published_unaudited() {
        let f = fixture();
        let job = f
            .jobs
            .create(&admin(), new_job("host_m", Schedule::Manual))
            .unwrap();
        let run = f.jobs.request_run(&admin(), &job.id).unwrap();
        f.jobs.collect("host_m").unwrap();
        f.jobs
            .finish("host_m", outcome(&run.id, Some(1), f.clock.now()))
            .unwrap();
        f.jobs.set_enabled(&admin(), &job.id, false).unwrap();
        f.jobs.remove(&admin(), &job.id).unwrap();
        let seen = f.seen.0.lock().unwrap();
        let summary: Vec<_> = seen.iter().map(|e| (e.topic.as_str(), e.audited)).collect();
        assert_eq!(
            summary,
            [
                ("job.created", true),
                ("job.run_requested", true),
                ("job.run_finished", false),
                ("job.changed", true),
                ("job.removed", true),
            ]
        );
        assert_eq!(seen[2].payload["state"], "failed");
        assert_eq!(seen[2].payload["job_name"], "backup");
    }
}
