//! Live hints for one resume, plus a global channel for generation tokens.
//! Postgres is the source of truth; these hubs only forward work as it happens.
//! A subscriber that arrives late sees nothing until the next event, and the
//! page reloads the authoritative detail afterward.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use domain::EventRecord;
use serde::Serialize;
use stream_protocol::ModelEvent;
use tokio::sync::broadcast;
use uuid::Uuid;

const CAPACITY: usize = 32;
const STREAM_CAPACITY: usize = 256;

/// The payload of a `GenerationStream` event. The nested `event` is one line
/// of the model's NDJSON output, so the browser uses the same parser for both.
#[derive(Clone, Debug, Serialize)]
pub struct GenerationFrame {
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub flow: &'static str,
    pub section_id: Option<String>,
    pub seq: u64,
    pub event: ModelEvent,
}

#[derive(Clone)]
pub struct Bus {
    inner: Arc<Mutex<HashMap<Uuid, broadcast::Sender<EventRecord>>>>,
    stream: Arc<broadcast::Sender<EventRecord>>,
}

impl Default for Bus {
    fn default() -> Self {
        let (stream, _) = broadcast::channel(STREAM_CAPACITY);
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            stream: Arc::new(stream),
        }
    }
}

impl Bus {
    /// A completed outbox event. Owner pages listen on their resume channel.
    pub fn publish(&self, event: EventRecord) {
        let mut slots = self.inner.lock().expect("bus");
        let sender = slots.entry(event.resume_id).or_insert_with(|| {
            let (sender, _) = broadcast::channel(CAPACITY);
            sender
        });
        // Nobody listening is fine. The stream is a hint, not a record.
        let _ = sender.send(event);
    }

    /// A token/stage frame emitted while a model is still working. It goes to
    /// the owner's resume channel and to the admin-only global stream.
    pub fn publish_stream(&self, event: EventRecord) {
        self.publish(event.clone());
        let _ = self.stream.send(event);
    }

    #[must_use]
    pub fn subscribe(&self, resume_id: Uuid) -> broadcast::Receiver<EventRecord> {
        let mut slots = self.inner.lock().expect("bus");
        let sender = slots.entry(resume_id).or_insert_with(|| {
            let (sender, _) = broadcast::channel(CAPACITY);
            sender
        });
        sender.subscribe()
    }

    /// Global stream for admins, who do not own every diagnosis they review.
    #[must_use]
    pub fn subscribe_stream(&self) -> broadcast::Receiver<EventRecord> {
        self.stream.subscribe()
    }
}
