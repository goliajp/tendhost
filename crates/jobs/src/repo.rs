use tendhost_kernel::Result;
use tendhost_store::Keyspace;

use crate::model::{Job, RUNS_KEPT_PER_JOB, Run};

const JOBS: &str = "jobs";
const DUE: &str = "due";
const OPEN: &str = "open";

/// Keys: `job:<id>`, `jobs` (by creation), `due` (jobs by next run),
/// `run:<id>`, `runs:<job>` (by due time), `pending:<host>` (runs the host
/// has not collected, by due time), `open` (runs without a final state, by
/// due time).
pub(crate) struct Repo {
    ks: Keyspace,
}

impl Repo {
    pub fn new(ks: Keyspace) -> Self {
        Self { ks }
    }

    /// Writes the job and files it under its next run time, if it has one.
    pub fn put_job(&self, job: &Job) -> Result<()> {
        self.ks.put(&format!("job:{}", job.id), job)?;
        self.ks
            .index_put(JOBS, job.created_at.timestamp_millis(), &job.id)?;
        match job.next_run_at {
            Some(at) => self.ks.index_put(DUE, at.timestamp_millis(), &job.id),
            None => self.ks.index_remove(DUE, &job.id),
        }
    }

    pub fn job(&self, id: &str) -> Result<Option<Job>> {
        self.ks.get(&format!("job:{id}"))
    }

    pub fn jobs(&self) -> Result<Vec<Job>> {
        let mut jobs = Vec::new();
        for id in self.ks.index_all(JOBS)? {
            jobs.extend(self.job(&id)?);
        }
        Ok(jobs)
    }

    pub fn due_jobs(&self, now_ms: i64) -> Result<Vec<Job>> {
        let mut jobs = Vec::new();
        for id in self.ks.index_range(DUE, i64::MIN, now_ms)? {
            jobs.extend(self.job(&id)?);
        }
        Ok(jobs)
    }

    /// Removes the job and every run of it.
    pub fn remove_job(&self, job: &Job) -> Result<()> {
        for run_id in self.ks.index_all(&runs_key(&job.id))? {
            self.forget_run(&job.host_id, &run_id)?;
        }
        self.ks.index_trim(&runs_key(&job.id), i64::MAX)?;
        self.ks.index_remove(DUE, &job.id)?;
        self.ks.index_remove(JOBS, &job.id)?;
        self.ks.delete(&format!("job:{}", job.id)).map(|_| ())
    }

    /// Writes the run and keeps the indexes in step with its state.
    pub fn put_run(&self, run: &Run) -> Result<()> {
        use crate::model::RunState::Pending;
        let due = run.due_at.timestamp_millis();
        self.ks.put(&format!("run:{}", run.id), run)?;
        self.ks.index_put(&runs_key(&run.job_id), due, &run.id)?;
        if run.state == Pending {
            self.ks
                .index_put(&pending_key(&run.host_id), due, &run.id)?;
        } else {
            self.ks.index_remove(&pending_key(&run.host_id), &run.id)?;
        }
        if run.state.is_over() {
            self.ks.index_remove(OPEN, &run.id)?;
            self.trim_runs(run)
        } else {
            self.ks.index_put(OPEN, due, &run.id)
        }
    }

    pub fn run(&self, id: &str) -> Result<Option<Run>> {
        self.ks.get(&format!("run:{id}"))
    }

    /// Newest first.
    pub fn runs_of(&self, job_id: &str, limit: usize) -> Result<Vec<Run>> {
        let mut runs = Vec::new();
        for id in self
            .ks
            .index_all(&runs_key(job_id))?
            .iter()
            .rev()
            .take(limit)
        {
            runs.extend(self.run(id)?);
        }
        Ok(runs)
    }

    pub fn pending_for(&self, host_id: &str) -> Result<Vec<Run>> {
        let mut runs = Vec::new();
        for id in self.ks.index_all(&pending_key(host_id))? {
            runs.extend(self.run(&id)?);
        }
        Ok(runs)
    }

    pub fn open_runs(&self) -> Result<Vec<Run>> {
        let mut runs = Vec::new();
        for id in self.ks.index_all(OPEN)? {
            runs.extend(self.run(&id)?);
        }
        Ok(runs)
    }

    fn trim_runs(&self, run: &Run) -> Result<()> {
        let ids = self.ks.index_all(&runs_key(&run.job_id))?;
        let excess = ids.len().saturating_sub(RUNS_KEPT_PER_JOB);
        for old in ids.iter().take(excess) {
            // only finished runs leave; an open one is still being tracked
            if self.run(old)?.is_some_and(|r| r.state.is_over()) {
                self.ks.index_remove(&runs_key(&run.job_id), old)?;
                self.ks.delete(&format!("run:{old}"))?;
            }
        }
        Ok(())
    }

    fn forget_run(&self, host_id: &str, run_id: &str) -> Result<()> {
        self.ks.index_remove(&pending_key(host_id), run_id)?;
        self.ks.index_remove(OPEN, run_id)?;
        self.ks.delete(&format!("run:{run_id}")).map(|_| ())
    }
}

fn runs_key(job_id: &str) -> String {
    format!("runs:{job_id}")
}

fn pending_key(host_id: &str) -> String {
    format!("pending:{host_id}")
}
