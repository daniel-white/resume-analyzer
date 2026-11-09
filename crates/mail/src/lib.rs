//! SMTP delivery for sign-in links and analysis notifications.

use lettre::AsyncTransport;
use lettre::Message;
use lettre::Tokio1Executor;
use lettre::message::Mailbox;

#[derive(Clone, Debug)]
pub struct Smtp {
    host: String,
    port: u16,
}

pub struct AnalysisResult {
    pub provider: String,
    pub score: u8,
    pub url: String,
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
        self.send(magic_link_message(to, url)?).await
    }

    pub async fn send_analysis_done(
        &self,
        to: &str,
        filename: &str,
        version: i32,
        results: &[AnalysisResult],
    ) -> Result<(), Error> {
        self.send(analysis_message(to, filename, version, results)?)
            .await
    }

    async fn send(&self, message: Message) -> Result<(), Error> {
        let mailer = lettre::AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host)
            .port(self.port)
            .timeout(Some(std::time::Duration::from_secs(15)))
            .build();
        mailer.send(message).await.map_err(|err| Error {
            message: format!("smtp: {err}"),
        })?;
        Ok(())
    }
}

fn magic_link_message(to: &str, url: &str) -> Result<Message, Error> {
    let body =
        format!("Open this link to sign in. It works once and expires in 15 minutes.\n\n{url}\n");
    message(to, "Your sign-in link", body)
}

fn analysis_message(
    to: &str,
    filename: &str,
    version: i32,
    results: &[AnalysisResult],
) -> Result<Message, Error> {
    let details = results
        .iter()
        .map(|result| {
            format!(
                "{} - Score: {}/100\n{}\n",
                result.provider, result.score, result.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    message(
        to,
        "Your résumé analysis results are ready",
        format!(
            "New results are ready for {filename} (version {version}).\n\n{details}\nSign in if prompted. Other providers may still be analyzing your résumé.\n\nThese are simulated ATS results; scores do not predict hiring outcomes.\n"
        ),
    )
}

fn message(to: &str, subject: &str, body: String) -> Result<Message, Error> {
    let from: Mailbox = "The Other Read <login@resume.local>"
        .parse()
        .map_err(|err| Error {
            message: format!("from: {err}"),
        })?;
    let to: Mailbox = to.parse().map_err(|err| Error {
        message: format!("to: {err}"),
    })?;
    Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
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

    #[test]
    fn analysis_message_names_the_provider_and_exact_version() {
        let url = "https://app.resume.local/resumes/abc?version=def&provider=greenline";
        let results = [
            AnalysisResult {
                provider: "Greenline".into(),
                score: 85,
                url: url.into(),
            },
            AnalysisResult {
                provider: "Levr".into(),
                score: 90,
                url: "https://app.resume.local/resumes/abc?version=def&provider=levr".into(),
            },
        ];
        let message = analysis_message("owner@example.com", "resume.pdf", 2, &results).unwrap();
        let encoded = String::from_utf8(message.formatted()).unwrap();
        assert!(encoded.contains("To: owner@example.com"));
        assert!(encoded.contains("Your résumé analysis results are ready"));
        assert!(encoded.contains("Greenline"));
        assert!(encoded.contains("Levr"));
        assert!(encoded.contains("provider=levr"));
        assert!(encoded.contains("resume.pdf (version 2)"));
        assert!(encoded.contains("85/100"));
        assert!(encoded.contains(url));
        assert!(encoded.contains("simulated ATS"));
    }
}
