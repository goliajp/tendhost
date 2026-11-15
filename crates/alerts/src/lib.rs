//! Conditions worth telling someone about: a host gone silent, a disk
//! nearly full, an agent near its limit, a job that failed.
//!
//! Listens to what the other modules announce on the bus and keeps one
//! alert per condition, open from the moment the condition begins until it
//! ends. It announces both moments and sends nothing itself.

mod events;
mod http;
mod model;
mod ports;
mod service;

pub use events::{AlertOpened, AlertResolved, SettingsChanged};
pub use http::router;
pub use model::{Alert, Kind, Settings};
pub use ports::HostNames;
pub use service::Alerts;
