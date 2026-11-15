use tendhost_kernel::Result;

/// What this module needs to know about hosts.
pub trait KnownHosts: Send + Sync {
    fn exists(&self, host_id: &str) -> Result<bool>;
}
