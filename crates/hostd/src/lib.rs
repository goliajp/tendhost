//! `tendhostd`, the daemon on a managed host. It makes outbound requests
//! to the server and opens no port.

pub mod client;
pub mod collect;
pub mod config;
pub mod exec;
pub mod outbox;
pub mod run;
