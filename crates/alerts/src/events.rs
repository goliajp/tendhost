use serde::Serialize;
use tendhost_kernel::Event;

use crate::model::{Kind, Settings};

#[derive(Debug, Serialize)]
pub struct AlertOpened {
    pub alert_id: String,
    pub kind: Kind,
    pub host_id: String,
    pub summary: String,
    pub detail: String,
}

impl Event for AlertOpened {
    const TOPIC: &'static str = "alert.opened";
    const AUDITED: bool = false;
}

#[derive(Debug, Serialize)]
pub struct AlertResolved {
    pub alert_id: String,
    pub kind: Kind,
    pub host_id: String,
    pub summary: String,
}

impl Event for AlertResolved {
    const TOPIC: &'static str = "alert.resolved";
    const AUDITED: bool = false;
}

#[derive(Debug, Serialize)]
pub struct SettingsChanged {
    pub settings: Settings,
}

impl Event for SettingsChanged {
    const TOPIC: &'static str = "alert.settings_changed";
}
