//! Live hint for one resume. Postgres is the source of truth; this hub only
//! forwards events the dispatcher has just finished. A subscriber that arrives
//! late sees nothing until the next event, and the page reloads the detail.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use domain::EventRecord;
use tokio::sync::broadcast;
use uuid::Uuid;

const CAPACITY: usize = 32;

#[derive(Clone, Default)]
pub struct Bus {
    inner: Arc<Mutex<HashMap<Uuid, broadcast::Sender<EventRecord>>>>,
}

impl Bus {
    pub fn publish(&self, event: EventRecord) {
        let mut slots = self.inner.lock().expect("bus");
        let sender = slots.entry(event.resume_id).or_insert_with(|| {
            let (sender, _) = broadcast::channel(CAPACITY);
            sender
        });
        // Nobody listening is fine. The stream is a hint, not a record.
        let _ = sender.send(event);
    }

    pub fn subscribe(&self, resume_id: Uuid) -> broadcast::Receiver<EventRecord> {
        let mut slots = self.inner.lock().expect("bus");
        let sender = slots.entry(resume_id).or_insert_with(|| {
            let (sender, _) = broadcast::channel(CAPACITY);
            sender
        });
        sender.subscribe()
    }
}
