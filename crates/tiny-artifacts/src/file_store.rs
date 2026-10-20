//! Durable artifact bytes over a host-bound blob handle.

use std::path::{Component, Path};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tinystoragedrivers_api::{BlobStore as TsdBlobStore, Storage};

/// Storage-independent metadata returned when bytes are written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    /// Number of bytes stored.
    pub size_bytes: u64,
    /// Media type guessed from the file name, when known.
    pub media_type: Option<String>,
}

/// Durable file bytes addressed by opaque keys.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Store bytes at `key`, replacing any existing object.
    ///
    /// # Errors
    /// Returns an error when the key is invalid or storage fails.
    async fn put(&self, key: &str, bytes: &[u8], media_type: Option<&str>) -> Result<FileMetadata>;
    /// Read the complete object at `key`.
    ///
    /// # Errors
    /// Returns an error when the key is invalid, the object is missing, or
    /// storage fails.
    async fn read(&self, key: &str) -> Result<Vec<u8>>;
    /// Return metadata for `key`, or `None` when it does not exist.
    ///
    /// # Errors
    /// Returns an error when the key is invalid or storage fails.
    async fn metadata(&self, key: &str) -> Result<Option<FileMetadata>>;
    /// Remove every object below the directory `prefix`, which must end in `/`.
    ///
    /// # Errors
    /// Returns an error when the prefix is invalid or storage fails.
    async fn delete_prefix(&self, prefix: &str) -> Result<()>;
}

/// [`FileStore`] adapter for a host-provided blob handle.
pub struct BlobFileStore {
    blobs: std::sync::Arc<dyn TsdBlobStore>,
}

impl BlobFileStore {
    /// Adapt the blobs in the host-bound `storage` bundle to the artifact
    /// file-store port.
    #[must_use]
    pub fn new(storage: &Storage) -> Self {
        Self {
            blobs: storage.blobs().clone(),
        }
    }
}

impl std::fmt::Debug for BlobFileStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobFileStore")
            .field("blobs", &self.blobs)
            .finish()
    }
}

#[async_trait]
impl FileStore for BlobFileStore {
    async fn put(&self, key: &str, bytes: &[u8], media_type: Option<&str>) -> Result<FileMetadata> {
        validate_key(key)?;
        let content_type = media_type.or_else(|| crate::media_type_for(Path::new(key)));
        let meta = self.blobs.put(key, bytes.to_vec(), content_type).await?;
        Ok(FileMetadata {
            size_bytes: meta.len,
            media_type: meta.content_type,
        })
    }

    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        validate_key(key)?;
        self.blobs
            .get(key)
            .await?
            .map(|blob| blob.bytes)
            .with_context(|| format!("file `{key}` does not exist"))
    }

    async fn metadata(&self, key: &str) -> Result<Option<FileMetadata>> {
        validate_key(key)?;
        Ok(self.blobs.head(key).await?.map(|meta| FileMetadata {
            size_bytes: meta.len,
            media_type: meta.content_type,
        }))
    }

    async fn delete_prefix(&self, prefix: &str) -> Result<()> {
        validate_prefix(prefix)?;
        for blob in self.blobs.list(prefix).await? {
            self.blobs.delete(&blob.key).await?;
        }
        Ok(())
    }
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() || key.starts_with('/') || key.contains('\\') || key.contains('\0') {
        bail!("invalid file key {key:?}");
    }
    if key
        .split('/')
        .any(|segment| segment.is_empty() || segment == ".")
    {
        bail!("file key must use canonical path segments: {key:?}");
    }
    if Path::new(key).components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!("file key must stay inside the store: {key:?}");
    }
    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<()> {
    if prefix.is_empty() {
        bail!("file key prefix is empty");
    }
    if !prefix.ends_with('/') {
        bail!("file key directory prefix must end with `/`: {prefix:?}");
    }
    validate_key(&prefix[..prefix.len() - 1])
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "file_store_tests.rs"]
mod tests;
