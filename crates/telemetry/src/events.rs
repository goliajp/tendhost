use serde::Serialize;
use tendhost_kernel::{Event, Timestamp};

/// A host was heard from after being offline, or for the first time.
#[derive(Debug, Serialize)]
pub struct HostOnline {
    pub host_id: String,
}

impl Event for HostOnline {
    const TOPIC: &'static str = "telemetry.host_online";
    const AUDITED: bool = false;
}

#[derive(Debug, Serialize)]
pub struct HostOffline {
    pub host_id: String,
    pub last_seen: Timestamp,
}

impl Event for HostOffline {
    const TOPIC: &'static str = "telemetry.host_offline";
    const AUDITED: bool = false;
}

/// A measurement was stored. Carries what others act on: how full each
/// disk is.
#[derive(Debug, Serialize)]
pub struct SampleRecorded {
    pub host_id: String,
    pub at: Timestamp,
    pub disks: Vec<DiskUse>,
}

#[derive(Debug, Serialize)]
pub struct DiskUse {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
}

impl Event for SampleRecorded {
    const TOPIC: &'static str = "telemetry.sample_recorded";
    const AUDITED: bool = false;
}
