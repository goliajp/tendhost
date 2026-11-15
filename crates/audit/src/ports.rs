use tendhost_kernel::Actor;

/// What this module needs to know about who acted: a name to show.
pub trait ActorNames: Send + Sync {
    /// `None` when the user or host no longer exists.
    fn name(&self, actor: &Actor) -> Option<String>;
}
