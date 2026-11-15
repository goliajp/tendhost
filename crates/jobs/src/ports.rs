use tendhost_kernel::Result;

/// What this module needs to know about hosts. Implemented where the
/// server is assembled, by whichever module owns hosts.
pub trait HostDirectory: Send + Sync {
    /// Whether the host lets the server run commands on it; `NotFound`
    /// when there is no such host.
    fn accepts_commands(&self, host_id: &str) -> Result<bool>;
}
