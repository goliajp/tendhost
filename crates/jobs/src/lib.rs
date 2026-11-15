//! Commands to run on hosts: what to run, where and when, and the record
//! of every run.
//!
//! Decides when a run is due and keeps its outcome. How a command reaches
//! a host and is executed there is not its business: it hands out the runs
//! a host should start and is told how each ended.

mod events;
mod http;
mod model;
mod ports;
mod repo;
mod service;

pub use events::{JobChanged, JobCreated, JobRemoved, RunFinished, RunRequested};
pub use http::router;
pub use model::{Assignment, Job, LastRun, NewJob, Outcome, Run, RunState, Schedule};
pub use ports::HostDirectory;
pub use service::Jobs;
