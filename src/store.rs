use async_trait::async_trait;

use crate::types::{ArtifactError, ArtifactRecord};
use crate::types::ArtifactStatus;

/// Durable per-company artifact storage.
///
/// Each implementation is responsible for its own backend (filesystem, SQLite,
/// MongoDB, etc.). The trait covers the full lifecycle: create, read, update,
/// delete, and list.
#[async_trait]
pub trait ArtifactStore: Send + Sync {
    /// List artifacts, most-recently-updated first, optionally narrowed to one
    /// task.
    async fn list(&self, task_id: Option<&str>) -> Result<Vec<ArtifactRecord>, ArtifactError>;

    /// Fetches one artifact by id, with its full version history.
    async fn get(&self, id: &str) -> Result<ArtifactRecord, ArtifactError>;

    /// Inserts or replaces an artifact by id.
    async fn upsert(&self, artifact: &ArtifactRecord) -> Result<(), ArtifactError>;

    /// Deletes an artifact by id; returns whether one was removed.
    async fn delete(&self, id: &str) -> Result<bool, ArtifactError>;

    /// Marks an artifact as Pending (being produced).
    async fn mark_pending(&self, id: &str) -> Result<(), ArtifactError> {
        let mut record = self.get(id).await?;
        record.status = ArtifactStatus::Pending;
        self.upsert(&record).await
    }

    /// Marks an artifact as Ready (production complete).
    async fn mark_ready(&self, id: &str, size_bytes: u64) -> Result<(), ArtifactError> {
        let mut record = self.get(id).await?;
        record.status = ArtifactStatus::Ready;
        self.upsert(&record).await
    }

    /// Marks an artifact as Failed.
    async fn mark_failed(&self, id: &str, reason: &str) -> Result<(), ArtifactError> {
        let mut record = self.get(id).await?;
        record.status = ArtifactStatus::Failed;
        self.upsert(&record).await
    }
}