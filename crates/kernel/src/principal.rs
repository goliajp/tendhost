use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Who did something. Recorded on every event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Actor {
    User(String),
    Host(String),
    System,
}

/// Ordered: a later variant may do everything an earlier one may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Admin,
    Owner,
}

/// The identity behind one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub actor: Actor,
    pub role: Role,
}

impl Principal {
    pub fn require(&self, role: Role) -> Result<()> {
        if self.role >= role {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_grants_itself_and_everything_below() {
        let admin = Principal {
            actor: Actor::User("u".into()),
            role: Role::Admin,
        };
        assert!(admin.require(Role::Viewer).is_ok());
        assert!(admin.require(Role::Admin).is_ok());
        assert!(matches!(admin.require(Role::Owner), Err(Error::Forbidden)));
    }
}
