use serde::{Deserialize, Serialize};
use tendhost_kernel::{Error, Result, Timestamp};

pub const DELIVERIES_KEPT: usize = 200;

/// Where a channel sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// `POST`s `{"title", "body", "source": "tendhost"}` as json
    Webhook { url: String },
    /// `POST`s the body to an ntfy topic address, the title in a header
    Ntfy { url: String, token: Option<String> },
    Email {
        host: String,
        port: u16,
        security: Security,
        username: Option<String>,
        password: Option<String>,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Security {
    /// TLS from the first byte (usually port 465)
    Tls,
    /// plain, then upgraded (usually port 587)
    Starttls,
    /// no encryption; for a relay on the same machine
    None,
}

impl Target {
    pub(crate) fn validated(self) -> Result<Self> {
        let web = |field, url: &str| {
            if url.starts_with("https://") || url.starts_with("http://") {
                Ok(())
            } else {
                Err(Error::invalid(field, "must start with http:// or https://"))
            }
        };
        let address = |field, value: &str| {
            let ok = value
                .split_once('@')
                .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'));
            if ok {
                Ok(())
            } else {
                Err(Error::invalid(field, "is not an email address"))
            }
        };
        match &self {
            Target::Webhook { url } | Target::Ntfy { url, .. } => web("url", url)?,
            Target::Email {
                host,
                port,
                from,
                to,
                ..
            } => {
                if host.trim().is_empty() {
                    return Err(Error::invalid("host", "is empty"));
                }
                if *port == 0 {
                    return Err(Error::invalid("port", "is 0"));
                }
                address("from", from)?;
                address("to", to)?;
            }
        }
        Ok(self)
    }

    /// The target with its secrets replaced by whether they are set.
    fn view(&self) -> TargetView {
        match self.clone() {
            Target::Webhook { url } => TargetView::Webhook { url },
            Target::Ntfy { url, token } => TargetView::Ntfy {
                url,
                has_token: token.is_some(),
            },
            Target::Email {
                host,
                port,
                security,
                username,
                password,
                from,
                to,
            } => TargetView::Email {
                host,
                port,
                security,
                username,
                has_password: password.is_some(),
                from,
                to,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetView {
    Webhook {
        url: String,
    },
    Ntfy {
        url: String,
        has_token: bool,
    },
    Email {
        host: String,
        port: u16,
        security: Security,
        username: Option<String>,
        has_password: bool,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewChannel {
    pub name: String,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub target: Target,
    pub created_at: Timestamp,
}

impl Channel {
    pub fn view(&self) -> ChannelView {
        ChannelView {
            id: self.id.clone(),
            name: self.name.clone(),
            target: self.target.view(),
            created_at: self.created_at,
        }
    }
}

/// A channel as the api shows it: tokens and passwords never leave the
/// server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelView {
    pub id: String,
    pub name: String,
    pub target: TargetView,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub title: String,
    pub body: String,
}

/// One attempt to send one message over one channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    pub channel_id: String,
    pub channel_name: String,
    pub at: Timestamp,
    pub title: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn email(from: &str, to: &str) -> Target {
        Target::Email {
            host: "smtp.example.com".into(),
            port: 587,
            security: Security::Starttls,
            username: Some("ada".into()),
            password: Some("hunter2".into()),
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn targets_are_checked_before_they_are_stored() {
        assert!(
            Target::Webhook {
                url: "https://example.com/hook".into()
            }
            .validated()
            .is_ok()
        );
        assert!(
            Target::Webhook {
                url: "example.com/hook".into()
            }
            .validated()
            .is_err()
        );
        assert!(
            Target::Ntfy {
                url: "ftp://ntfy.sh/x".into(),
                token: None
            }
            .validated()
            .is_err()
        );
        assert!(
            email("tend@example.com", "ada@example.com")
                .validated()
                .is_ok()
        );
        assert!(email("tend", "ada@example.com").validated().is_err());
        assert!(
            email("tend@example.com", "ada@localhost")
                .validated()
                .is_err()
        );
    }

    #[test]
    fn a_view_says_whether_a_secret_is_set_without_showing_it() {
        let channel = Channel {
            id: "chn_1".into(),
            name: "mail".into(),
            target: email("tend@example.com", "ada@example.com"),
            created_at: Default::default(),
        };
        let json = serde_json::to_string(&channel.view()).unwrap();
        assert!(json.contains(r#""has_password":true"#) && !json.contains("hunter2"));
        let ntfy = Target::Ntfy {
            url: "https://ntfy.sh/x".into(),
            token: Some("tk_secret".into()),
        };
        let json = serde_json::to_string(&ntfy.view()).unwrap();
        assert!(json.contains(r#""has_token":true"#) && !json.contains("tk_secret"));
    }
}
