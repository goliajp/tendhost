//! The three ways a message leaves.

use std::time::Duration;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

use crate::model::{Message, Security, Target};

const TIMEOUT: Duration = Duration::from_secs(15);

/// Sends `message` to `target`; the error is a sentence for the delivery
/// record.
pub(crate) async fn send(
    http: &reqwest::Client,
    target: &Target,
    message: &Message,
) -> Result<(), String> {
    match target {
        Target::Webhook { url } => {
            let body = serde_json::json!({ "title": message.title, "body": message.body, "source": "tendhost" });
            answered(http.post(url).timeout(TIMEOUT).json(&body).send().await)
        }
        Target::Ntfy { url, token } => {
            let mut request = http
                .post(url)
                .timeout(TIMEOUT)
                .header("Title", header_text(&message.title));
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            answered(request.body(message.body.clone()).send().await)
        }
        Target::Email {
            host,
            port,
            security,
            username,
            password,
            from,
            to,
        } => {
            let mail = lettre::Message::builder()
                .from(
                    from.parse::<Mailbox>()
                        .map_err(|e| format!("from address: {e}"))?,
                )
                .to(to
                    .parse::<Mailbox>()
                    .map_err(|e| format!("to address: {e}"))?)
                .subject(&message.title)
                .body(message.body.clone())
                .map_err(|e| format!("building the mail: {e}"))?;
            let builder = match security {
                Security::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(host),
                Security::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host),
                Security::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                    host,
                )),
            }
            .map_err(|e| format!("mail server {host}: {e}"))?;
            let mut builder = builder.port(*port).timeout(Some(TIMEOUT));
            if let (Some(username), Some(password)) = (username, password) {
                builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
            }
            builder
                .build()
                .send(mail)
                .await
                .map(|_| ())
                .map_err(|e| format!("mail server {host}: {e}"))
        }
    }
}

fn answered(response: reqwest::Result<reqwest::Response>) -> Result<(), String> {
    match response {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => Err(format!("the receiver answered {}", response.status())),
        Err(error) => Err(format!(
            "could not reach the receiver: {}",
            error.without_url()
        )),
    }
}

/// A header value cannot carry line breaks or non-ascii text.
fn header_text(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_made_safe_for_a_header() {
        assert_eq!(
            header_text("disk 91% on db\r\nX-Evil: 1"),
            "disk 91% on db??X-Evil: 1"
        );
        assert_eq!(header_text("磁盘 full"), "?? full");
    }
}
