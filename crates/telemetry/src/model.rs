use chrono::Duration;
use serde::{Deserialize, Serialize};
use tendhost_kernel::Timestamp;

/// A host not heard from for this long is offline. Three missed syncs at
/// the default interval.
pub const OFFLINE_AFTER: Duration = Duration::seconds(90);
pub const RETENTION: Duration = Duration::days(7);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub at: Timestamp,
    pub load1: f32,
    pub memory_used_bytes: u64,
    pub uptime_secs: u64,
    pub disks: Vec<Disk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disk {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    pub host_id: String,
    pub last_seen: Timestamp,
    pub online: bool,
}
