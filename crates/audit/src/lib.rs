//! The record of who did what. No module writes to it: it keeps every
//! audited event that passes over the bus, so the only way to leave
//! something out of the record is not to publish it.

mod http;
mod ports;
mod service;

pub use http::router;
pub use ports::ActorNames;
pub use service::{Audit, Entry};
