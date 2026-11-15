use serde::Serialize;
use tendhost_kernel::Event;

#[derive(Debug, Serialize)]
pub struct ChannelCreated {
    pub channel_id: String,
    pub name: String,
    /// `webhook`, `ntfy` or `email`
    pub kind: &'static str,
}

impl Event for ChannelCreated {
    const TOPIC: &'static str = "notify.channel_created";
}

#[derive(Debug, Serialize)]
pub struct ChannelRemoved {
    pub channel_id: String,
    pub name: String,
}

impl Event for ChannelRemoved {
    const TOPIC: &'static str = "notify.channel_removed";
}
