//! The machines under management: how one is enrolled, what proves it is
//! the same machine afterwards, what it is called and what it says about
//! itself.
//!
//! Owns nothing about measurements or whether a host is online.

mod events;
mod http;
mod model;
mod repo;
mod service;

pub use events::{EnrollmentOpened, HostEnrolled, HostRemoved, HostUpdated};
pub use http::router;
pub use model::{Facts, Host, Mode, NewEnrollment};
pub use service::Hosts;
