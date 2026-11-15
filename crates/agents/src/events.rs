use serde::Serialize;
use tendhost_kernel::{Event, Timestamp};

#[derive(Debug, Serialize)]
pub struct TrackingChanged {
    pub host_id: String,
    pub tracked: Vec<String>,
}

impl Event for TrackingChanged {
    const TOPIC: &'static str = "agents.tracking_changed";
}

/// A limit window of a tracked agent, as last read on its host.
#[derive(Debug, Serialize)]
pub struct LimitRead {
    pub host_id: String,
    pub agent: String,
    pub window: String,
    pub used_ratio: f32,
    pub resets_at: Option<Timestamp>,
}

impl Event for LimitRead {
    const TOPIC: &'static str = "agents.limit_read";
    const AUDITED: bool = false;
}
