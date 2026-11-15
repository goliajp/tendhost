use serde::Serialize;
use tendhost_kernel::Event;

use crate::model::Mode;

#[derive(Debug, Serialize)]
pub struct EnrollmentOpened {
    pub enrollment_id: String,
}

impl Event for EnrollmentOpened {
    const TOPIC: &'static str = "host.enrollment_opened";
}

#[derive(Debug, Serialize)]
pub struct HostEnrolled {
    pub host_id: String,
    pub name: String,
    pub mode: Mode,
    pub enrollment_id: String,
}

impl Event for HostEnrolled {
    const TOPIC: &'static str = "host.enrolled";
}

#[derive(Debug, Serialize)]
pub struct HostUpdated {
    pub host_id: String,
    pub name: String,
    pub labels: Vec<String>,
}

impl Event for HostUpdated {
    const TOPIC: &'static str = "host.updated";
}

#[derive(Debug, Serialize)]
pub struct HostRemoved {
    pub host_id: String,
    pub name: String,
}

impl Event for HostRemoved {
    const TOPIC: &'static str = "host.removed";
}
