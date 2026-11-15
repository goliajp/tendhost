//! `tendhostd`, the daemon on a managed host. It makes outbound requests
//! to the server and opens no port.

pub mod agents;
pub mod client;
pub mod collect;
pub mod config;
pub mod exec;
pub mod outbox;
pub mod run;
pub mod service;
