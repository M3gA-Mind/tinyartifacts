//! Durable bytes behind [`FileRef`] keys.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tinystoragedrivers_core::{BlobStore as TsdBlobStore, ScopedStorage};

use crate::file_ref::{FileRef, FileRoots};
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
    ///
    /// Returns an error when the key is invalid or storage fails.
    async fn put(&self, key: &str, bytes: &[u8], media_type: Option<&str>) -> Result<FileMetadata>;
    /// Read the complete object at `key`.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is invalid, the object is missing, or
    /// storage fails.
    async fn read(&self, key: &str) -> Result<Vec<u8>>;
    /// Return metadata for `key`, or `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is invalid or storage fails.
    async fn metadata(&self, key: &str) -> Result<Option<FileMetadata>>;
    /// Remove every object whose key starts with `prefix`.
    ///
    /// # Errors
    ///
    /// Returns an error when the prefix is invalid or storage fails.
    async fn delete_prefix(&self, prefix: &str) -> Result<()>;
}

/// [`FileStore`] adapter for a host-provided scoped storage handle.
pub struct BlobFileStore {
    blobs: std::sync::Arc<dyn TsdBlobStore>,
}

impl BlobFileStore {
    /// Adapt the blobs in `storage` to the artifact file-store port.
    #[must_use]
    pub fn new(storage: &ScopedStorage) -> Self {
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

/// Files kept below `root` using path-safe storage keys.
#[derive(Clone, Debug)]
pub struct FsFileStore {
    root: PathBuf,
}

impl FsFileStore {
    /// Store files under `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory containing stored file objects.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    async fn path(&self, key: &str, create_parent: bool) -> Result<PathBuf> {
        validate_key(key)?;
        tokio::fs::create_dir_all(&self.root).await?;
        let root = tokio::fs::canonicalize(&self.root).await?;
        let path = root.join(key);
        let parent = path.parent().context("file key has no parent")?;
        if create_parent {
            tokio::fs::create_dir_all(parent).await?;
        }
        let canonical_parent = match tokio::fs::canonicalize(parent).await {
            Ok(parent) => parent,
            Err(error)
                if !create_parent && error.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(root.join(key));
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("cannot resolve parent of {}", path.display())
                });
            }
        };
        if !canonical_parent.starts_with(&root) {
            bail!("file key escapes the configured store root: {key:?}");
        }
        Ok(canonical_parent.join(path.file_name().context("file key has no name")?))
    }
}

#[async_trait]
impl FileStore for FsFileStore {
    async fn put(&self, key: &str, bytes: &[u8], media_type: Option<&str>) -> Result<FileMetadata> {
        let path = self.path(key, true).await?;
        let parent = path.parent().context("file key has no parent")?;
        let staging = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&staging, bytes)
            .await
            .with_context(|| format!("cannot write {}", staging.display()))?;
        if let Err(error) = tokio::fs::rename(&staging, &path).await {
            let _ = tokio::fs::remove_file(&staging).await;
            return Err(error).with_context(|| format!("cannot save {}", path.display()));
        }
        Ok(FileMetadata {
            size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            media_type: media_type
                .map(str::to_owned)
                .or_else(|| crate::media_type_for(Path::new(key)).map(str::to_owned)),
        })
    }

    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.path(key, false).await?;
        if let Ok(canonical) = tokio::fs::canonicalize(&path).await {
            let root = tokio::fs::canonicalize(&self.root).await?;
            if !canonical.starts_with(root) {
                bail!("file key escapes the configured store root: {key:?}");
            }
        }
        tokio::fs::read(&path)
            .await
            .with_context(|| format!("cannot read {}", path.display()))
    }

    async fn metadata(&self, key: &str) -> Result<Option<FileMetadata>> {
        let path = self.path(key, false).await?;
        if let Ok(canonical) = tokio::fs::canonicalize(&path).await {
            let root = tokio::fs::canonicalize(&self.root).await?;
            if !canonical.starts_with(root) {
                bail!("file key escapes the configured store root: {key:?}");
            }
        }
        match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_file() => Ok(Some(FileMetadata {
                size_bytes: metadata.len(),
                media_type: crate::media_type_for(&path).map(str::to_owned),
            })),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
        }
    }

    async fn delete_prefix(&self, prefix: &str) -> Result<()> {
        validate_prefix(prefix)?;
        let path = self.path(prefix.trim_end_matches('/'), false).await?;
        if let Ok(canonical) = tokio::fs::canonicalize(&path).await {
            let root = tokio::fs::canonicalize(&self.root).await?;
            if !canonical.starts_with(root) {
                bail!("file key prefix escapes the configured store root: {prefix:?}");
            }
        }
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("cannot remove {}", path.display())),
        }
    }
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() || key.starts_with('/') || key.contains('\\') || key.contains('\0') {
        bail!("invalid file key {key:?}");
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
    validate_key(prefix.trim_end_matches('/'))
}

/// Copy a legacy path-based reference into any configured file store.
///
/// The source path is resolved against the supplied roots before it is read;
/// this also applies the host's guard to desktop data being imported.
///
/// # Errors
///
/// Returns an error if the old path is refused, unreadable, or the new object
/// cannot be written.
pub async fn import_legacy_file(
    store: &dyn FileStore,
    roots: &FileRoots,
    legacy: &FileRef,
    key: &str,
) -> Result<FileRef> {
    let source = roots.resolve(&legacy.key).await?;
    let bytes = tokio::fs::read(&source.path).await?;
    let media_type = legacy
        .media_type
        .as_deref()
        .or(source.media_type.as_deref());
    let metadata = store.put(key, &bytes, media_type).await?;
    Ok(FileRef {
        key: key.to_owned(),
        size_bytes: metadata.size_bytes,
        media_type: metadata.media_type,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "file_store_tests.rs"]
mod tests;
