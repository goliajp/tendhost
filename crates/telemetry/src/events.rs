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
