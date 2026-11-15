//! Coding agents (Claude Code, Codex, Gemini CLI, Kimi Code) on managed
//! hosts: which are installed where, and, for the ones an admin chose to
//! track, their accounts, token usage and limit windows.
//!
//! Nothing here reads an agent's files; the host reports what it found.
//! Tracking is a choice per host and agent, and withdrawing it deletes
//! what was collected.

mod events;
mod http;
mod model;
mod ports;
mod repo;
mod service;

pub use events::{LimitRead, TrackingChanged};
pub use http::router;
pub use model::{
    Account, AgentView, Found, FoundDetail, Installed, KINDS, LimitReading, LimitWindow, UsageDay,
};
pub use ports::KnownHosts;
pub use service::Agents;
