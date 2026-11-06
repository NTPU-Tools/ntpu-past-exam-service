use std::io::{self, Write};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing_subscriber::fmt::writer::MakeWriter;

/// A `MakeWriter` that always writes to stdout and optionally tees each JSON
/// line into a channel consumed by the Better Stack forwarder.
#[derive(Clone)]
pub struct TeeWriter {
    sender: Option<UnboundedSender<String>>,
}

impl TeeWriter {
    pub fn new(sender: Option<UnboundedSender<String>>) -> Self {
        TeeWriter { sender }
    }
}

pub struct Tee {
    sender: Option<UnboundedSender<String>>,
    buffer: Vec<u8>,
}

impl<'a> MakeWriter<'a> for TeeWriter {
    type Writer = Tee;

    fn make_writer(&'a self) -> Self::Writer {
        Tee {
            sender: self.sender.clone(),
            buffer: Vec::new(),
        }
    }
}

impl Write for Tee {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        io::stdout().write_all(bytes)?;
        if self.sender.is_some() {
            self.buffer.extend_from_slice(bytes);
            while let Some(position) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=position).collect();
                if let Some(sender) = &self.sender {
                    if let Ok(text) = std::str::from_utf8(&line) {
                        let text = text.trim_end();
                        if !text.is_empty() {
                            let _ = sender.send(text.to_string());
                        }
                    }
                }
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

pub struct Forwarder {
    pub sender: UnboundedSender<String>,
    pub receiver: UnboundedReceiver<String>,
}

pub fn channel() -> Forwarder {
    let (sender, receiver) = unbounded_channel();
    Forwarder { sender, receiver }
}

/// Batches formatted log lines and POSTs them to Better Stack. Failures are
/// swallowed: logging must never affect request handling.
pub async fn forward_logs(
    mut receiver: UnboundedReceiver<String>,
    host: String,
    source_key: String,
    service_name: String,
    client: reqwest::Client,
) {
    let mut batch: Vec<String> = Vec::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            line = receiver.recv() => {
                match line {
                    Some(line) => {
                        batch.push(line);
                        if batch.len() >= 200 {
                            flush(&mut batch, &host, &source_key, &service_name, &client).await;
                        }
                    }
                    None => break,
                }
            }
            _ = ticker.tick() => {
                flush(&mut batch, &host, &source_key, &service_name, &client).await;
            }
        }
    }
    flush(&mut batch, &host, &source_key, &service_name, &client).await;
}

async fn flush(
    batch: &mut Vec<String>,
    host: &str,
    source_key: &str,
    service_name: &str,
    client: &reqwest::Client,
) {
    if batch.is_empty() {
        return;
    }
    let events: Vec<Value> = batch
        .drain(..)
        .map(|line| match serde_json::from_str::<Value>(&line) {
            Ok(value) => json!({
                "dt": value.get("timestamp").cloned().unwrap_or(Value::Null),
                "level": value.get("level").cloned().unwrap_or(Value::Null),
                "message": value
                    .pointer("/fields/message")
                    .cloned()
                    .unwrap_or_else(|| Value::String(line.clone())),
                "target": value.get("target").cloned().unwrap_or(Value::Null),
                "fields": value.get("fields").cloned().unwrap_or(Value::Null),
                "env": service_name,
            }),
            Err(_) => json!({ "message": line, "env": service_name }),
        })
        .collect();
    let result = client
        .post(host)
        .bearer_auth(source_key)
        .json(&events)
        .send()
        .await;
    if let Err(error) = result {
        // Never log through tracing here: it would feed back into the forwarder.
        eprintln!("failed to ship logs to Better Stack: {error}");
    }
}
