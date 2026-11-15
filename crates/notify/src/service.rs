use std::sync::Arc;

use serde::Deserialize;
use tendhost_kernel::{Bus, Clock, Envelope, Error, Principal, Result, Role, Sink, id};
use tendhost_store::Keyspace;
use tokio::sync::{Mutex, mpsc};

use crate::events::{ChannelCreated, ChannelRemoved};
use crate::model::{Channel, ChannelView, DELIVERIES_KEPT, Delivery, Message, NewChannel, Target};
use crate::transport;

const CHANNELS: &str = "channels";
const DELIVERIES: &str = "deliveries";

/// Keys: `channel:<id>`, `channels` (by creation), `deliveries` (by time;
/// each member is the delivery itself).
pub struct Notify {
    ks: Keyspace,
    bus: Bus,
    clock: Arc<dyn Clock>,
    http: reqwest::Client,
    queue: mpsc::UnboundedSender<Message>,
    waiting: Mutex<mpsc::UnboundedReceiver<Message>>,
}

/// This module's view of the events it passes on: `alert.opened` and
/// `alert.resolved`.
#[derive(Deserialize)]
struct Announced {
    summary: String,
    #[serde(default)]
    detail: String,
}

impl Notify {
    pub fn new(keyspace: Keyspace, bus: Bus, clock: Arc<dyn Clock>) -> Self {
        let (queue, waiting) = mpsc::unbounded_channel();
        Self {
            ks: keyspace,
            bus,
            clock,
            http: reqwest::Client::new(),
            queue,
            waiting: Mutex::new(waiting),
        }
    }

    pub fn create(&self, principal: &Principal, new: NewChannel) -> Result<ChannelView> {
        principal.require(Role::Admin)?;
        let name = new.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(Error::invalid("name", "must be 1 to 64 characters"));
        }
        let now = self.clock.now();
        let channel = Channel {
            id: id::new("chn", now),
            name,
            target: new.target.validated()?,
            created_at: now,
        };
        self.ks.put(&format!("channel:{}", channel.id), &channel)?;
        self.ks
            .index_put(CHANNELS, now.timestamp_millis(), &channel.id)?;
        let kind = match channel.target {
            Target::Webhook { .. } => "webhook",
            Target::Ntfy { .. } => "ntfy",
            Target::Email { .. } => "email",
        };
        self.bus.publish(
            principal.actor.clone(),
            &ChannelCreated {
                channel_id: channel.id.clone(),
                name: channel.name.clone(),
                kind,
            },
        );
        Ok(channel.view())
    }

    pub fn channels(&self, principal: &Principal) -> Result<Vec<ChannelView>> {
        principal.require(Role::Admin)?;
        Ok(self.all()?.iter().map(Channel::view).collect())
    }

    pub fn remove(&self, principal: &Principal, channel_id: &str) -> Result<()> {
        principal.require(Role::Admin)?;
        let channel = self.channel(channel_id)?;
        self.ks.index_remove(CHANNELS, channel_id)?;
        self.ks.delete(&format!("channel:{channel_id}"))?;
        self.bus.publish(
            principal.actor.clone(),
            &ChannelRemoved {
                channel_id: channel.id,
                name: channel.name,
            },
        );
        Ok(())
    }

    /// Sends a test message over one channel and says how it went.
    pub async fn test(&self, principal: &Principal, channel_id: &str) -> Result<Delivery> {
        principal.require(Role::Admin)?;
        let channel = self.channel(channel_id)?;
        let message = Message {
            title: "tendhost test message".into(),
            body: format!(
                "This is a test of the channel \"{}\". If you can read it, the channel works.",
                channel.name
            ),
        };
        self.send_over(&channel, &message).await
    }

    /// Newest first.
    pub fn deliveries(&self, principal: &Principal, limit: usize) -> Result<Vec<Delivery>> {
        principal.require(Role::Admin)?;
        self.ks
            .index_all(DELIVERIES)?
            .iter()
            .rev()
            .take(limit)
            .map(|member| serde_json::from_str(member).map_err(Error::internal))
            .collect()
    }

    /// Sends every message that is waiting over every channel. Returns how
    /// many messages were taken.
    pub async fn deliver_waiting(&self) -> Result<usize> {
        let mut waiting = self.waiting.lock().await;
        let mut taken = 0;
        while let Ok(message) = waiting.try_recv() {
            self.broadcast(&message).await?;
            taken += 1;
        }
        Ok(taken)
    }

    /// Sends messages as they arrive, forever.
    pub async fn work(&self) {
        loop {
            let message = self.waiting.lock().await.recv().await;
            let Some(message) = message else { return };
            if let Err(error) = self.broadcast(&message).await {
                tracing::error!(%error, "a notification was not recorded");
            }
        }
    }

    async fn broadcast(&self, message: &Message) -> Result<()> {
        for channel in self.all()? {
            self.send_over(&channel, message).await?;
        }
        Ok(())
    }

    async fn send_over(&self, channel: &Channel, message: &Message) -> Result<Delivery> {
        let outcome = transport::send(&self.http, &channel.target, message).await;
        let delivery = Delivery {
            channel_id: channel.id.clone(),
            channel_name: channel.name.clone(),
            at: self.clock.now(),
            title: message.title.clone(),
            ok: outcome.is_ok(),
            error: outcome.err(),
        };
        let member = serde_json::to_string(&delivery).map_err(Error::internal)?;
        self.ks
            .index_put(DELIVERIES, delivery.at.timestamp_millis(), &member)?;
        let excess = self
            .ks
            .index_len(DELIVERIES)?
            .saturating_sub(DELIVERIES_KEPT);
        for old in self.ks.index_all(DELIVERIES)?.iter().take(excess) {
            self.ks.index_remove(DELIVERIES, old)?;
        }
        Ok(delivery)
    }

    fn channel(&self, id: &str) -> Result<Channel> {
        self.ks
            .get(&format!("channel:{id}"))?
            .ok_or(Error::NotFound { kind: "channel" })
    }

    fn all(&self) -> Result<Vec<Channel>> {
        let mut channels = Vec::new();
        for id in self.ks.index_all(CHANNELS)? {
            channels.extend(self.ks.get::<Channel>(&format!("channel:{id}"))?);
        }
        Ok(channels)
    }
}

impl Sink for Notify {
    fn accept(&self, envelope: &Envelope) {
        let prefix = match envelope.topic.as_str() {
            "alert.opened" => "",
            "alert.resolved" => "Resolved: ",
            _ => return,
        };
        let Some(announced) = envelope.view::<Announced>(&envelope.topic) else {
            return;
        };
        // the worker holds the receiving end for the life of the process
        let _ = self.queue.send(Message {
            title: format!("{prefix}{}", announced.summary),
            body: announced.detail,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use chrono::{TimeZone, Utc};
    use serde::Serialize;
    use tendhost_kernel::{Actor, Event, ManualClock};
    use tendhost_store::Store;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    use super::*;
    use crate::model::Security;

    type Received = Arc<StdMutex<Vec<(String, String)>>>;

    /// A receiver on a real socket: records `(Title header or "", body)`;
    /// `/fail` answers 500.
    async fn receiver() -> (String, Received) {
        let received: Received = Arc::default();
        async fn take(State(received): State<Received>, headers: HeaderMap, body: String) {
            let title = headers
                .get("title")
                .map(|v| v.to_str().unwrap().to_string())
                .unwrap_or_default();
            received.lock().unwrap().push((title, body));
        }
        let app = Router::new()
            .route("/ok", post(take))
            .route(
                "/fail",
                post(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .with_state(received.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, received)
    }

    /// A mail server that accepts one message and hands back what came
    /// after DATA.
    async fn mail_server() -> (u16, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"220 test ESMTP\r\n").await.unwrap();
            let (mut in_data, mut data) = (false, String::new());
            while let Some(line) = lines.next_line().await.unwrap() {
                if in_data {
                    if line == "." {
                        in_data = false;
                        write.write_all(b"250 queued\r\n").await.unwrap();
                    } else {
                        data.push_str(&line);
                        data.push('\n');
                    }
                    continue;
                }
                let verb = line.split(' ').next().unwrap_or_default().to_uppercase();
                let reply: &[u8] = match verb.as_str() {
                    "EHLO" | "HELO" => b"250 test\r\n",
                    "DATA" => {
                        in_data = true;
                        b"354 go on\r\n"
                    }
                    "QUIT" => {
                        write.write_all(b"221 bye\r\n").await.unwrap();
                        break;
                    }
                    _ => b"250 ok\r\n",
                };
                write.write_all(reply).await.unwrap();
            }
            data
        });
        (port, served)
    }

    #[derive(Serialize)]
    struct AlertOpened {
        summary: String,
        detail: String,
        kind: String,
    }

    impl Event for AlertOpened {
        const TOPIC: &'static str = "alert.opened";
        const AUDITED: bool = false;
    }

    #[derive(Serialize)]
    struct AlertResolved {
        summary: String,
    }

    impl Event for AlertResolved {
        const TOPIC: &'static str = "alert.resolved";
        const AUDITED: bool = false;
    }

    fn fixture() -> (Arc<Notify>, Bus) {
        let clock = Arc::new(ManualClock::at(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        ));
        let bus = Bus::new(clock.clone());
        let notify = Arc::new(Notify::new(
            Store::temp().unwrap().keyspace("notify"),
            bus.clone(),
            clock,
        ));
        bus.attach(notify.clone());
        (notify, bus)
    }

    fn admin() -> Principal {
        Principal {
            actor: Actor::User("usr_a".into()),
            role: Role::Admin,
        }
    }

    fn channel(name: &str, target: Target) -> NewChannel {
        NewChannel {
            name: name.into(),
            target,
        }
    }

    #[tokio::test]
    async fn an_alert_reaches_every_channel_in_that_channels_form() {
        let (url, received) = receiver().await;
        let (notify, bus) = fixture();
        notify
            .create(
                &admin(),
                channel(
                    "hook",
                    Target::Webhook {
                        url: format!("{url}/ok"),
                    },
                ),
            )
            .unwrap();
        notify
            .create(
                &admin(),
                channel(
                    "push",
                    Target::Ntfy {
                        url: format!("{url}/ok"),
                        token: None,
                    },
                ),
            )
            .unwrap();

        bus.publish(
            Actor::System,
            &AlertOpened {
                summary: "db is offline".into(),
                detail: "last seen 12:00".into(),
                kind: "host_offline".into(),
            },
        );
        bus.publish(
            Actor::System,
            &AlertResolved {
                summary: "db is offline".into(),
            },
        );
        assert_eq!(notify.deliver_waiting().await.unwrap(), 2);

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 4);
        // the two channels were created in the same instant; either may go first
        let opened: Vec<_> = received.iter().take(2).collect();
        let hook = opened.iter().find(|(title, _)| title.is_empty()).unwrap();
        let hook: serde_json::Value = serde_json::from_str(&hook.1).unwrap();
        assert_eq!(
            hook,
            serde_json::json!({ "title": "db is offline", "body": "last seen 12:00", "source": "tendhost" })
        );
        let push = opened.iter().find(|(title, _)| !title.is_empty()).unwrap();
        assert_eq!(
            **push,
            ("db is offline".to_string(), "last seen 12:00".to_string())
        );
        assert!(
            received[2..]
                .iter()
                .any(|(title, _)| title == "Resolved: db is offline")
        );
        let log = notify.deliveries(&admin(), 10).unwrap();
        assert_eq!(log.len(), 4);
        assert!(log.iter().all(|d| d.ok));
    }

    #[tokio::test]
    async fn a_failed_delivery_is_recorded_with_its_reason_and_does_not_stop_the_others() {
        let (url, received) = receiver().await;
        let (notify, bus) = fixture();
        notify
            .create(
                &admin(),
                channel(
                    "broken",
                    Target::Webhook {
                        url: format!("{url}/fail"),
                    },
                ),
            )
            .unwrap();
        notify
            .create(
                &admin(),
                channel(
                    "gone",
                    Target::Webhook {
                        url: "http://127.0.0.1:9/x".into(),
                    },
                ),
            )
            .unwrap();
        notify
            .create(
                &admin(),
                channel(
                    "fine",
                    Target::Webhook {
                        url: format!("{url}/ok"),
                    },
                ),
            )
            .unwrap();
        bus.publish(
            Actor::System,
            &AlertResolved {
                summary: "x".into(),
            },
        );
        notify.deliver_waiting().await.unwrap();

        assert_eq!(received.lock().unwrap().len(), 1);
        let log = notify.deliveries(&admin(), 10).unwrap();
        let by_name = |name: &str| log.iter().find(|d| d.channel_name == name).unwrap();
        assert!(by_name("fine").ok);
        assert!(by_name("broken").error.as_deref().unwrap().contains("500"));
        let unreachable = by_name("gone").error.clone().unwrap();
        assert!(
            unreachable.contains("could not reach") && !unreachable.contains("127.0.0.1"),
            "{unreachable}"
        );
    }

    #[tokio::test]
    async fn an_email_channel_hands_the_message_to_its_mail_server() {
        let (port, served) = mail_server().await;
        let (notify, _) = fixture();
        let target = Target::Email {
            host: "127.0.0.1".into(),
            port,
            security: Security::None,
            username: None,
            password: None,
            from: "tend@example.com".into(),
            to: "ada@example.com".into(),
        };
        let created = notify.create(&admin(), channel("mail", target)).unwrap();
        let delivery = notify.test(&admin(), &created.id).await.unwrap();
        assert!(delivery.ok, "{:?}", delivery.error);
        let data = served.await.unwrap();
        assert!(data.contains("Subject: tendhost test message"));
        assert!(data.contains("To: ada@example.com") && data.contains("From: tend@example.com"));
        assert!(data.contains("the channel works"));
    }

    #[tokio::test]
    async fn events_that_are_not_alerts_are_not_sent() {
        #[derive(Serialize)]
        struct Other {
            summary: String,
        }
        impl Event for Other {
            const TOPIC: &'static str = "host.enrolled";
        }
        let (notify, bus) = fixture();
        bus.publish(
            Actor::System,
            &Other {
                summary: "x".into(),
            },
        );
        assert_eq!(notify.deliver_waiting().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn channels_are_an_admins_and_changes_are_published() {
        let (notify, _) = fixture();
        let viewer = Principal {
            actor: Actor::User("usr_v".into()),
            role: Role::Viewer,
        };
        let target = || Target::Webhook {
            url: "https://example.com/h".into(),
        };
        assert!(matches!(
            notify.create(&viewer, channel("x", target())),
            Err(Error::Forbidden)
        ));
        assert!(matches!(notify.channels(&viewer), Err(Error::Forbidden)));
        assert!(matches!(
            notify.create(&admin(), channel(" ", target())),
            Err(Error::Invalid { field: "name", .. })
        ));
        let created = notify.create(&admin(), channel("x", target())).unwrap();
        notify.remove(&admin(), &created.id).unwrap();
        assert!(notify.channels(&admin()).unwrap().is_empty());
        assert!(matches!(
            notify.remove(&admin(), &created.id),
            Err(Error::NotFound { .. })
        ));
    }
}
