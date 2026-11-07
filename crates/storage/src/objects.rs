use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

#[async_trait]
pub trait Objects: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>>;
    async fn delete(&self, key: &str) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct MemoryObjects {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[async_trait]
impl Objects for MemoryObjects {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        self.inner
            .lock()
            .expect("objects")
            .insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        self.inner
            .lock()
            .expect("objects")
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing object {key}"))
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.inner.lock().expect("objects").remove(key);
        Ok(())
    }
}
