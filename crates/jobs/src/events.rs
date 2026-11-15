use serde::Serialize;
use tendhost_kernel::Event;

use crate::model::RunState;

#[derive(Debug, Serialize)]
pub struct JobCreated {
    pub job_id: String,
    pub name: String,
    pub host_id: String,
    pub command: String,
}

impl Event for JobCreated {
    const TOPIC: &'static str = "job.created";
}

/// Enabled or disabled.
#[derive(Debug, Serialize)]
pub struct JobChanged {
    pub job_id: String,
    pub name: String,
    pub enabled: bool,
}

impl Event for JobChanged {
    const TOPIC: &'static str = "job.changed";
}

#[derive(Debug, Serialize)]
pub struct JobRemoved {
    pub job_id: String,
    pub name: String,
}

impl Event for JobRemoved {
    const TOPIC: &'static str = "job.removed";
}

/// A person asked for a run outside the schedule.
#[derive(Debug, Serialize)]
pub struct RunRequested {
    pub job_id: String,
    pub run_id: String,
    pub host_id: String,
}

impl Event for RunRequested {
    const TOPIC: &'static str = "job.run_requested";
}

/// A run reached a final state. The runs themselves are the record, so
/// this is not repeated in the audit log.
#[derive(Debug, Serialize)]
pub struct RunFinished {
    pub job_id: String,
    pub job_name: String,
    pub run_id: String,
    pub host_id: String,
    pub state: RunState,
}

impl Event for RunFinished {
    const TOPIC: &'static str = "job.run_finished";
    const AUDITED: bool = false;
}
