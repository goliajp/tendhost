//! People who sign in: first-run setup, users, sessions, api tokens.
//!
//! Owns nothing about hosts; a host's credential belongs to the module that
//! owns hosts.

mod events;
mod http;
mod model;
mod password;
mod repo;
mod service;

pub use events::{OwnerCreated, SignedIn, TokenCreated, TokenRevoked, UserCreated, UserRemoved};
pub use http::router;
pub use model::{NewToken, Token, User};
pub use service::{Identity, Settings};
