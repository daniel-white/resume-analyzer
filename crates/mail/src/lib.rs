//! SMTP delivery for magic links. Mailpit accepts this without TLS.

use lettre::AsyncTransport;
use lettre::Message;
use lettre::Tokio1Executor;
use lettre::message::Mailbox;

#[derive(Clone, Debug)]
pub struct Smtp {
    host: String,
    port: u16,
}

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl Smtp {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    pub async fn send_magic_link(&self, to: &str, url: &str) -> Result<(), Error> {
        let message = magic_link_message(to, url)?;
        let mailer = lettre::AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
            .port(self.port)
            .build();
        mailer.send(message).await.map_err(|err| Error {
            message: format!("smtp: {err}"),
        })?;
        Ok(())
    }
}

fn magic_link_message(to: &str, url: &str) -> Result<Message, Error> {
    let from: Mailbox = "The Other Read <login@resume.local>"
        .parse()
        .map_err(|err| Error {
            message: format!("from: {err}"),
        })?;
    let to: Mailbox = to.parse().map_err(|err| Error {
        message: format!("to: {err}"),
    })?;
    let body =
        format!("Open this link to sign in. It works once and expires in 15 minutes.\n\n{url}\n");
    Message::builder()
        .from(from)
        .to(to)
        .subject("Your sign-in link")
        .body(body)
        .map_err(|err| Error {
            message: format!("message: {err}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_link_message_contains_the_url() {
        let url = "https://app.resume.local/login/verify?token=abc";
        let message = magic_link_message("ada@example.com", url).unwrap();
        let encoded = String::from_utf8(message.formatted()).unwrap();
        assert!(encoded.contains(url));
        assert!(encoded.contains("ada@example.com"));
    }
}
