//! The connections between modules. Each adapter implements a port one
//! module declared, using another module, so that neither depends on the
//! other.

use std::sync::Arc;

use tendhost_hosts::{Hosts, Mode};
use tendhost_jobs::HostDirectory;
use tendhost_kernel::Result;

/// What jobs needs to know about hosts, answered by the hosts module.
pub(crate) struct HostsDirectory(pub Arc<Hosts>);

impl HostDirectory for HostsDirectory {
    fn accepts_commands(&self, host_id: &str) -> Result<bool> {
        Ok(self.0.mode(host_id)? == Mode::Manage)
    }
}
