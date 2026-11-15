//! Channels that carry word out of tendhost (a webhook, an ntfy topic, an
//! email address) and the record of what was sent over them.
//!
//! Does not decide what is worth telling: it sends the alerts another
//! module announces on the bus, as it finds them there.

mod events;
mod http;
mod model;
mod service;
mod transport;

pub use events::{ChannelCreated, ChannelRemoved};
pub use http::router;
pub use model::{Channel, ChannelView, Delivery, Message, NewChannel, Security, Target};
pub use service::Notify;
