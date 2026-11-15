/// What this module needs to know about hosts: what to call one in a
/// sentence.
pub trait HostNames: Send + Sync {
    /// `None` when there is no such host.
    fn name(&self, host_id: &str) -> Option<String>;
}
