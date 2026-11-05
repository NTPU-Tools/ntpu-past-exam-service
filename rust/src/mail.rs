use aws_sdk_ses::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_ses::types::{Body, Content, Destination, Message};
use aws_sdk_ses::Client;

use crate::config::Config;

const TEMPLATE: &str = include_str!("../assets/notification-email.html");
const CTA_TEMPLATE: &str = include_str!("../assets/notification-email-cta.html");

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Single-pass placeholder substitution so escaped values are never re-scanned.
fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    loop {
        let mut earliest: Option<(usize, &str, &str)> = None;
        for (key, value) in pairs {
            if let Some(position) = rest.find(key) {
                if earliest.map(|(best, _, _)| position < best).unwrap_or(true) {
                    earliest = Some((position, key, value));
                }
            }
        }
        match earliest {
            Some((position, key, value)) => {
                output.push_str(&rest[..position]);
                output.push_str(value);
                rest = &rest[position + key.len()..];
            }
            None => {
                output.push_str(rest);
                break;
            }
        }
    }
    output
}

pub fn render(title: &str, content: &str, cta: Option<(&str, &str)>) -> String {
    let cta_block = match cta {
        Some((text, link)) => fill(
            CTA_TEMPLATE,
            &[
                ("{{CTA_LINK}}", &escape(link)),
                ("{{CTA_TEXT}}", &escape(text)),
            ],
        ),
        None => String::new(),
    };
    fill(
        TEMPLATE,
        &[
            ("{{TITLE}}", &escape(title)),
            ("{{CONTENT}}", &escape(content)),
            ("{{CTA_BLOCK}}", &cta_block),
        ],
    )
}

pub(crate) fn subject(service_name: &str, title: &str) -> String {
    let prefix = if service_name == "PROD" {
        ""
    } else {
        "測試環境 - "
    };
    format!("{prefix}{title} - 北大考古題 NTPU Past Exam")
}

pub struct Mailer {
    client: Option<Client>,
    sender: Option<String>,
    service_name: String,
}

impl Mailer {
    pub fn new(config: &Config) -> Mailer {
        let client = config.mail.as_ref().map(|mail| {
            let credentials = Credentials::new(
                &mail.access_key,
                &mail.secret,
                None,
                None,
                "past-exam-service",
            );
            let mut builder = aws_sdk_ses::config::Builder::new()
                .behavior_version(BehaviorVersion::latest())
                .region(Region::new("ap-southeast-1"))
                .credentials_provider(credentials);
            if let Some(endpoint) = &mail.endpoint {
                builder = builder.endpoint_url(endpoint);
            }
            Client::from_conf(builder.build())
        });
        Mailer {
            client,
            sender: config.mail.as_ref().map(|mail| mail.sender.clone()),
            service_name: config.service_name.clone(),
        }
    }

    /// Fire-and-forget: mail failures never affect the API response.
    pub fn notify(
        &self,
        title: &str,
        content: &str,
        recipients: &[String],
        cta: Option<(String, String)>,
    ) {
        let Some(client) = self.client.clone() else {
            tracing::info!("Skipping email notification: AWS SES is not configured.");
            return;
        };
        if recipients.is_empty() {
            return;
        }
        let sender = self.sender.clone().unwrap_or_default();
        let subject = subject(&self.service_name, title);
        let html = render(
            title,
            content,
            cta.as_ref()
                .map(|(text, link)| (text.as_str(), link.as_str())),
        );
        let recipients = recipients.to_vec();
        tokio::spawn(async move {
            let Ok(subject) = Content::builder().data(subject).charset("UTF-8").build() else {
                return;
            };
            let Ok(body) = Content::builder().data(html).charset("UTF-8").build() else {
                return;
            };
            let message = Message::builder()
                .subject(subject)
                .body(Body::builder().html(body).build())
                .build();
            let result = client
                .send_email()
                .source(format!("NTPU Tools <{sender}>"))
                .destination(
                    Destination::builder()
                        .set_to_addresses(Some(recipients))
                        .build(),
                )
                .message(message)
                .send()
                .await;
            if let Err(error) = result {
                tracing::warn!(error = %error, "failed to send notification email");
            }
        });
    }
}
