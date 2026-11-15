use serde::Serialize;
use tendhost_kernel::{Event, Role};

#[derive(Debug, Serialize)]
pub struct OwnerCreated {
    pub user_id: String,
    pub name: String,
}

impl Event for OwnerCreated {
    const TOPIC: &'static str = "identity.owner_created";
}

#[derive(Debug, Serialize)]
pub struct UserCreated {
    pub user_id: String,
    pub name: String,
    pub role: Role,
}

impl Event for UserCreated {
    const TOPIC: &'static str = "identity.user_created";
}

#[derive(Debug, Serialize)]
pub struct UserRemoved {
    pub user_id: String,
    pub name: String,
}

impl Event for UserRemoved {
    const TOPIC: &'static str = "identity.user_removed";
}

#[derive(Debug, Serialize)]
pub struct SignedIn {
    pub user_id: String,
}

impl Event for SignedIn {
    const TOPIC: &'static str = "identity.signed_in";
}

#[derive(Debug, Serialize)]
pub struct TokenCreated {
    pub token_id: String,
    pub name: String,
    pub role: Role,
}

impl Event for TokenCreated {
    const TOPIC: &'static str = "identity.token_created";
}

#[derive(Debug, Serialize)]
pub struct TokenRevoked {
    pub token_id: String,
    pub name: String,
}

impl Event for TokenRevoked {
    const TOPIC: &'static str = "identity.token_revoked";
}
