//! Measurements over time, and presence: when each host was last heard
//! from and whether that still counts as online.
//!
//! Knows hosts only by id. What a host is belongs to another module; this
//! one learns that a host is gone from the `host.removed` event.

mod events;
mod http;
mod model;
mod service;

pub use events::{HostOffline, HostOnline};
pub use http::router;
pub use model::{Disk, Presence, Sample};
pub use service::Telemetry;
