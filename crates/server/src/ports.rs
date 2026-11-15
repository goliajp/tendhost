//! The connections between modules. Each adapter implements a port one
//! module declared, using another module, so that neither depends on the
//! other.

use std::sync::Arc;

use tendhost_agents::KnownHosts;
use tendhost_hosts::{Hosts, Mode};
use tendhost_jobs::HostDirectory;
use tendhost_kernel::{Error, Result};

/// What jobs needs to know about hosts, answered by the hosts module.
pub(crate) struct HostsDirectory(pub Arc<Hosts>);

impl HostDirectory for HostsDirectory {
    fn accepts_commands(&self, host_id: &str) -> Result<bool> {
        Ok(self.0.mode(host_id)? == Mode::Manage)
    }
}

/// What agents needs to know about hosts, answered by the hosts module.
pub(crate) struct HostsKnown(pub Arc<Hosts>);

impl KnownHosts for HostsKnown {
    fn exists(&self, host_id: &str) -> Result<bool> {
        match self.0.mode(host_id) {
            Ok(_) => Ok(true),
            Err(Error::NotFound { .. }) => Ok(false),
            Err(other) => Err(other),
        }
    }
}
