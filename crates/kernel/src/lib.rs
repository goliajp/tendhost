//! What every tendhost crate agrees on, and nothing else: no storage, no
//! network, no business concept.

pub mod bus;
pub mod error;
pub mod id;
pub mod principal;
pub mod secret;
pub mod time;

pub use bus::{Bus, Envelope, Event, Sink};
pub use error::{Error, Result};
pub use principal::{Actor, Principal, Role};
pub use time::{Clock, ManualClock, SystemClock, Timestamp};
