//! How modules tell each other what happened without knowing each other.
//!
//! A module publishes typed events. A consumer never imports the publisher's
//! type: it matches on the topic and decodes the payload into a struct of
//! its own that names only the fields it needs.

use std::sync::{Arc, RwLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{Actor, Clock, Timestamp, id};

/// Something that happened, as its publisher describes it.
pub trait Event: Serialize {
    /// `<module>.<what happened>`, e.g. `host.enrolled`
    const TOPIC: &'static str;
    /// Whether the event belongs in the audit log. Off for the ones that
    /// only report routine measurements.
    const AUDITED: bool = true;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub id: String,
    pub topic: String,
    pub at: Timestamp,
    pub actor: Actor,
    pub audited: bool,
    pub payload: serde_json::Value,
}

impl Envelope {
    /// The payload as the consumer's own view of it; `None` when the topic
    /// is another one or the payload lacks a field the view requires.
    pub fn view<V: DeserializeOwned>(&self, topic: &str) -> Option<V> {
        if self.topic != topic {
            return None;
        }
        serde_json::from_value(self.payload.clone()).ok()
    }
}

/// Receives every event before `publish` returns. For consumers that must
/// not miss one (the audit log).
pub trait Sink: Send + Sync {
    fn accept(&self, envelope: &Envelope);
}

#[derive(Clone)]
pub struct Bus {
    clock: Arc<dyn Clock>,
    sinks: Arc<RwLock<Vec<Arc<dyn Sink>>>>,
    live: broadcast::Sender<Arc<Envelope>>,
}

impl Bus {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            sinks: Arc::default(),
            live: broadcast::channel(1024).0,
        }
    }

    pub fn attach(&self, sink: Arc<dyn Sink>) {
        self.sinks.write().expect("bus sinks lock").push(sink);
    }

    /// A live feed for consumers that may fall behind and skip (pages that
    /// update as things happen).
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.live.subscribe()
    }

    pub fn publish<E: Event>(&self, actor: Actor, event: &E) {
        let at = self.clock.now();
        let envelope = Arc::new(Envelope {
            id: id::new("evt", at),
            topic: E::TOPIC.to_string(),
            at,
            actor,
            audited: E::AUDITED,
            payload: serde_json::to_value(event).expect("an event serializes to json"),
        });
        for sink in self.sinks.read().expect("bus sinks lock").iter() {
            sink.accept(&envelope);
        }
        // no live subscriber is not an error
        let _ = self.live.send(envelope);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::SystemClock;

    #[derive(Serialize)]
    struct Enrolled {
        host_id: String,
        name: String,
    }

    impl Event for Enrolled {
        const TOPIC: &'static str = "host.enrolled";
    }

    #[derive(Deserialize)]
    struct OnlyTheId {
        host_id: String,
    }

    #[derive(Deserialize)]
    struct WantsMore {
        #[allow(dead_code)]
        serial: String,
    }

    #[derive(Default)]
    struct Collect(Mutex<Vec<Envelope>>);

    impl Sink for Collect {
        fn accept(&self, envelope: &Envelope) {
            self.0.lock().unwrap().push(envelope.clone());
        }
    }

    #[test]
    fn a_sink_sees_the_event_before_publish_returns() {
        let bus = Bus::new(Arc::new(SystemClock));
        let sink = Arc::new(Collect::default());
        bus.attach(sink.clone());
        bus.publish(
            Actor::System,
            &Enrolled {
                host_id: "host_1".into(),
                name: "a".into(),
            },
        );
        let seen = sink.0.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].topic, "host.enrolled");
        assert_eq!(seen[0].actor, Actor::System);
    }

    #[test]
    fn a_consumer_decodes_its_own_view_of_the_payload() {
        let bus = Bus::new(Arc::new(SystemClock));
        let sink = Arc::new(Collect::default());
        bus.attach(sink.clone());
        bus.publish(
            Actor::System,
            &Enrolled {
                host_id: "host_1".into(),
                name: "a".into(),
            },
        );
        let seen = sink.0.lock().unwrap();
        let view: OnlyTheId = seen[0].view("host.enrolled").unwrap();
        assert_eq!(view.host_id, "host_1");
        assert!(seen[0].view::<OnlyTheId>("host.removed").is_none());
        assert!(seen[0].view::<WantsMore>("host.enrolled").is_none());
    }

    #[tokio::test]
    async fn a_live_subscriber_receives_events_published_after_it_subscribed() {
        let bus = Bus::new(Arc::new(SystemClock));
        let mut feed = bus.subscribe();
        bus.publish(
            Actor::System,
            &Enrolled {
                host_id: "host_1".into(),
                name: "a".into(),
            },
        );
        assert_eq!(feed.recv().await.unwrap().topic, "host.enrolled");
    }
}
