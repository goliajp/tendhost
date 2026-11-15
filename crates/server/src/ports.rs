//! The connections between modules. Each adapter implements a port one
//! module declared, using another module, so that neither depends on the
//! other.

use std::sync::Arc;

use tendhost_agents::KnownHosts;
use tendhost_alerts::HostNames;
use tendhost_audit::ActorNames;
use tendhost_hosts::{Hosts, Mode};
use tendhost_identity::Identity;
use tendhost_jobs::HostDirectory;
use tendhost_kernel::{Actor, Error, Result};

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

/// What alerts needs to know about hosts, answered by the hosts module.
pub(crate) struct HostsNames(pub Arc<Hosts>);

impl HostNames for HostsNames {
    fn name(&self, host_id: &str) -> Option<String> {
        self.0.name(host_id).ok().flatten()
    }
}

/// What the audit log needs to know about who acted, answered by the
/// modules that own users and hosts.
pub(crate) struct Names {
    pub identity: Arc<Identity>,
    pub hosts: Arc<Hosts>,
}

impl ActorNames for Names {
    fn name(&self, actor: &Actor) -> Option<String> {
        match actor {
            Actor::User(id) => self.identity.user_name(id).ok().flatten(),
            Actor::Host(id) => self.hosts.name(id).ok().flatten(),
            Actor::System => None,
        }
    }
}
