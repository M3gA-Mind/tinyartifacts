use super::*;

use tinystoragedrivers_core::{MemoryStorage, Scope, StorageBackend};

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("tiny-artifacts-files-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[tokio::test]
async fn filesystem_store_reads_metadata_and_deletes_by_artifact_prefix() {
    let root = temp_dir();
    let store = FsFileStore::new(&root);
    let metadata = store
        .put("artifacts/a/v1.png", b"png bytes", None)
        .await
        .unwrap();

    assert_eq!(metadata.size_bytes, 9);
    assert_eq!(metadata.media_type.as_deref(), Some("image/png"));
    assert_eq!(
        store.read("artifacts/a/v1.png").await.unwrap(),
        b"png bytes"
    );
    assert_eq!(
        store.metadata("artifacts/a/v1.png").await.unwrap(),
        Some(metadata)
    );
    store.delete_prefix("artifacts/a/").await.unwrap();
    assert_eq!(store.metadata("artifacts/a/v1.png").await.unwrap(), None);
}

#[tokio::test]
async fn blob_adapters_share_objects_across_instances() {
    let backend = MemoryStorage::new();
    let first = BlobFileStore::new(backend.for_scope(&Scope::local()).unwrap());
    let second = BlobFileStore::new(backend.for_scope(&Scope::local()).unwrap());

    first
        .put("artifacts/a/v1.txt", b"notes", None)
        .await
        .unwrap();

    assert_eq!(second.read("artifacts/a/v1.txt").await.unwrap(), b"notes");
    second.delete_prefix("artifacts/a/").await.unwrap();
    assert_eq!(first.metadata("artifacts/a/v1.txt").await.unwrap(), None);
}

#[tokio::test]
async fn filesystem_store_refuses_traversal_keys() {
    let store = FsFileStore::new(temp_dir());
    assert!(store.put("../outside", b"bad", None).await.is_err());
    assert!(store.read("/absolute").await.is_err());
    assert!(store.delete_prefix("../").await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn filesystem_store_refuses_symlinked_directories_outside_root() {
    use std::os::unix::fs::symlink;

    let root = temp_dir();
    let outside = temp_dir();
    std::fs::write(outside.join("keep.txt"), b"safe").unwrap();
    symlink(&outside, root.join("escape")).unwrap();
    let store = FsFileStore::new(&root);

    assert!(store.put("escape/new.txt", b"bad", None).await.is_err());
    assert!(store.read("escape/keep.txt").await.is_err());
    assert!(store.delete_prefix("escape/").await.is_err());
    assert_eq!(std::fs::read(outside.join("keep.txt")).unwrap(), b"safe");
    assert!(!outside.join("new.txt").exists());
}

#[tokio::test]
async fn legacy_path_references_import_only_from_allowed_roots() {
    let old_root = temp_dir();
    let old_path = old_root.join("v1.pdf");
    std::fs::write(&old_path, b"legacy").unwrap();
    let legacy: FileRef = serde_json::from_value(serde_json::json!({
        "path": old_path,
        "sizeBytes": 6,
        "mediaType": "application/pdf"
    }))
    .unwrap();
    let store = FsFileStore::new(temp_dir());

    let imported = import_legacy_file(
        &store,
        &FileRoots::new([old_root.clone()]),
        &legacy,
        "artifacts/a/v1.pdf",
    )
    .await
    .unwrap();
    assert_eq!(imported.key, "artifacts/a/v1.pdf");
    assert_eq!(store.read(&imported.key).await.unwrap(), b"legacy");
    assert!(import_legacy_file(
        &store,
        &FileRoots::new([temp_dir()]),
        &legacy,
        "artifacts/b/v1.pdf",
    )
    .await
    .is_err());

    let backend = MemoryStorage::new();
    let blob_store = BlobFileStore::new(backend.for_scope(&Scope::local()).unwrap());
    let imported_blob = import_legacy_file(
        &blob_store,
        &FileRoots::new([old_root]),
        &legacy,
        "artifacts/cloud/v1.pdf",
    )
    .await
    .unwrap();
    assert_eq!(
        blob_store.read(&imported_blob.key).await.unwrap(),
        b"legacy"
    );
}
