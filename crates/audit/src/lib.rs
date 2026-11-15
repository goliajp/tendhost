//! The record of who did what. No module writes to it: it keeps every
//! audited event that passes over the bus, so the only way to leave
//! something out of the record is not to publish it.

mod http;
mod service;

pub use http::router;
pub use service::{Audit, Entry};
