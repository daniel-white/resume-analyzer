//! Object storage behind the same trait the tests use in memory.

use async_trait::async_trait;
use s3::Client as S3Client;
use storage::Objects;

pub struct S3Objects {
    client: S3Client,
    bucket: String,
}

impl S3Objects {
    pub fn new(client: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }
}

#[async_trait]
impl Objects for S3Objects {
    async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        self.client
            .objects()
            .put(&self.bucket, key)
            .body_bytes(bytes.to_vec())
            .send()
            .await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let object = self.client.objects().get(&self.bucket, key).send().await?;
        let bytes = object.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.client
            .objects()
            .delete(&self.bucket, key)
            .send()
            .await?;
        Ok(())
    }
}
