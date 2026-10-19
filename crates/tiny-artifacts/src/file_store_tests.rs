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
        .put("artifacts/files/a/v1.png", b"png bytes", None)
        .await
        .unwrap();

    assert_eq!(metadata.size_bytes, 9);
    assert_eq!(metadata.media_type.as_deref(), Some("image/png"));
    assert_eq!(
        store.read("artifacts/files/a/v1.png").await.unwrap(),
        b"png bytes"
    );
    assert_eq!(
        store.metadata("artifacts/files/a/v1.png").await.unwrap(),
        Some(metadata)
    );
    store.delete_prefix("artifacts/files/a/").await.unwrap();
    assert_eq!(
        store.metadata("artifacts/files/a/v1.png").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn blob_adapters_share_objects_across_instances() {
    let backend = MemoryStorage::new();
    let scope = Scope::local();
    let first_storage = backend.for_scope(&scope).unwrap();
    let second_storage = backend.for_scope(&scope).unwrap();
    let first = BlobFileStore::new(&first_storage);
    let second = BlobFileStore::new(&second_storage);

    first
        .put("artifacts/files/a/v1.txt", b"notes", None)
        .await
        .unwrap();

    assert_eq!(
        second.read("artifacts/files/a/v1.txt").await.unwrap(),
        b"notes"
    );
    second.delete_prefix("artifacts/files/a/").await.unwrap();
    assert_eq!(
        first.metadata("artifacts/files/a/v1.txt").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn blob_store_reports_missing_objects_and_rejects_invalid_keys_and_prefixes() {
    let backend = MemoryStorage::new();
    let storage = backend.for_scope(&Scope::local()).unwrap();
    let store = BlobFileStore::new(&storage);

    assert!(store.read("artifacts/files/missing.txt").await.is_err());
    assert_eq!(
        store.metadata("artifacts/files/missing.txt").await.unwrap(),
        None
    );
    assert!(store.put("../outside", b"bad", None).await.is_err());
    assert!(store
        .put("artifacts/files/a//v1.txt", b"bad", None)
        .await
        .is_err());
    assert!(store
        .put("artifacts/files/a/./v1.txt", b"bad", None)
        .await
        .is_err());
    assert!(store.metadata("/absolute").await.is_err());
    assert!(store.read("bad\\key").await.is_err());
    assert!(store.delete_prefix("").await.is_err());
    store
        .put("artifacts/files/sibling/v1.txt", b"keep", None)
        .await
        .unwrap();
    assert!(store.delete_prefix("../").await.is_err());
    assert_eq!(
        store.read("artifacts/files/sibling/v1.txt").await.unwrap(),
        b"keep"
    );
    store
        .delete_prefix("artifacts/files/missing/")
        .await
        .unwrap();
}

#[tokio::test]
async fn prefix_deletion_requires_a_directory_boundary_in_both_stores() {
    let backend = MemoryStorage::new();
    let storage = backend.for_scope(&Scope::local()).unwrap();
    let blobs = BlobFileStore::new(&storage);
    let fs = FsFileStore::new(temp_dir());

    for store in [&blobs as &dyn FileStore, &fs as &dyn FileStore] {
        store
            .put("artifacts/files/a/v1.txt", b"a", None)
            .await
            .unwrap();
        store
            .put("artifacts/files/ab/v1.txt", b"ab", None)
            .await
            .unwrap();

        assert!(store.delete_prefix("artifacts/files/a").await.is_err());
        assert_eq!(store.read("artifacts/files/a/v1.txt").await.unwrap(), b"a");
        assert_eq!(
            store.read("artifacts/files/ab/v1.txt").await.unwrap(),
            b"ab"
        );
        store.delete_prefix("artifacts/files/a/").await.unwrap();
        assert!(store
            .metadata("artifacts/files/a/v1.txt")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store.read("artifacts/files/ab/v1.txt").await.unwrap(),
            b"ab"
        );
    }
}

#[tokio::test]
async fn prefix_deletion_rejects_noncanonical_segments_without_touching_siblings() {
    let backend = MemoryStorage::new();
    let storage = backend.for_scope(&Scope::local()).unwrap();
    let blobs = BlobFileStore::new(&storage);
    let fs = FsFileStore::new(temp_dir());

    for store in [&blobs as &dyn FileStore, &fs as &dyn FileStore] {
        store
            .put("artifacts/files/a/v1.txt", b"a", None)
            .await
            .unwrap();
        store
            .put("artifacts/files/ab/v1.txt", b"ab", None)
            .await
            .unwrap();

        for malformed in ["artifacts/files/a//", "artifacts/files/a/./"] {
            assert!(store.delete_prefix(malformed).await.is_err());
            assert_eq!(store.read("artifacts/files/a/v1.txt").await.unwrap(), b"a");
            assert_eq!(
                store.read("artifacts/files/ab/v1.txt").await.unwrap(),
                b"ab"
            );
        }
    }
}

#[tokio::test]
async fn filesystem_store_refuses_traversal_keys() {
    let store = FsFileStore::new(temp_dir());
    assert!(store.put("../outside", b"bad", None).await.is_err());
    assert!(store.read("/absolute").await.is_err());
    store
        .put("artifacts/files/sibling/v1.txt", b"keep", None)
        .await
        .unwrap();
    assert!(store.delete_prefix("../").await.is_err());
    assert_eq!(
        store.read("artifacts/files/sibling/v1.txt").await.unwrap(),
        b"keep"
    );
}

#[tokio::test]
async fn filesystem_store_reports_missing_paths_and_storage_errors() {
    let root = temp_dir();
    let store = FsFileStore::new(&root);

    assert!(store.read("artifacts/files/missing.txt").await.is_err());
    assert_eq!(
        store.metadata("artifacts/files/missing.txt").await.unwrap(),
        None
    );
    store
        .delete_prefix("artifacts/files/missing/")
        .await
        .unwrap();
    assert!(store.put("", b"bad", None).await.is_err());
    assert!(store
        .put("artifacts/files/a//v1.txt", b"bad", None)
        .await
        .is_err());
    assert!(store
        .put("artifacts/files/a/./v1.txt", b"bad", None)
        .await
        .is_err());
    assert!(store.put("bad\\key", b"bad", None).await.is_err());
    assert!(store.put("bad\0key", b"bad", None).await.is_err());

    let file_root = root.join("not-a-directory");
    std::fs::write(&file_root, b"file").unwrap();
    let blocked = FsFileStore::new(file_root);
    assert!(blocked
        .put("artifacts/files/a/v1.txt", b"bad", None)
        .await
        .is_err());

    let stored = "artifacts/files/a/v1.txt";
    store.put(stored, b"data", None).await.unwrap();
    assert!(store.delete_prefix(&format!("{stored}/")).await.is_err());
}

#[test]
fn file_key_validation_rejects_unsafe_keys_and_prefixes() {
    assert!(validate_key("").is_err());
    assert!(validate_key("/absolute").is_err());
    assert!(validate_key("../outside").is_err());
    assert!(validate_key("a//b").is_err());
    assert!(validate_key("a/./b").is_err());
    assert!(validate_key("nested\\file").is_err());
    assert!(validate_key("nul\0key").is_err());
    assert!(validate_key("artifacts/files/a/v1.txt").is_ok());
    assert!(validate_prefix("").is_err());
    assert!(validate_prefix("../").is_err());
    assert!(validate_prefix("a//").is_err());
    assert!(validate_prefix("a/./").is_err());
    assert!(validate_prefix("artifacts/files/a/").is_ok());
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
    assert!(store.metadata("escape/not-created/file.txt").await.is_err());
    assert!(store.delete_prefix("escape/not-created/").await.is_err());
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
        "artifacts/files/a/v1.pdf",
    )
    .await
    .unwrap();
    assert_eq!(imported.key, "artifacts/files/a/v1.pdf");
    assert_eq!(store.read(&imported.key).await.unwrap(), b"legacy");
    assert!(import_legacy_file(
        &store,
        &FileRoots::new([temp_dir()]),
        &legacy,
        "artifacts/files/b/v1.pdf",
    )
    .await
    .is_err());

    let backend = MemoryStorage::new();
    let storage = backend.for_scope(&Scope::local()).unwrap();
    let blob_store = BlobFileStore::new(&storage);
    let imported_blob = import_legacy_file(
        &blob_store,
        &FileRoots::new([old_root]),
        &legacy,
        "artifacts/files/cloud/v1.pdf",
    )
    .await
    .unwrap();
    assert_eq!(
        blob_store.read(&imported_blob.key).await.unwrap(),
        b"legacy"
    );
}

#[tokio::test]
async fn legacy_import_respects_host_guards_and_infers_missing_media_type() {
    let root = temp_dir();
    let old_path = root.join("legacy.txt");
    std::fs::write(&old_path, b"legacy text").unwrap();
    let legacy: FileRef = serde_json::from_value(serde_json::json!({
        "path": old_path,
        "sizeBytes": 11
    }))
    .unwrap();
    let store = FsFileStore::new(temp_dir());
    let guarded_roots = FileRoots::new([root]).with_guard(|_| Some("blocked by test".into()));
    store
        .put("artifacts/files/a/v1.txt", b"preserve me", None)
        .await
        .unwrap();

    assert!(
        import_legacy_file(&store, &guarded_roots, &legacy, "artifacts/files/a/v1.txt")
            .await
            .is_err()
    );
    assert_eq!(
        store.read("artifacts/files/a/v1.txt").await.unwrap(),
        b"preserve me"
    );
    let allowed = FileRoots::new([Path::new(&legacy.key).parent().unwrap().to_path_buf()]);
    let imported = import_legacy_file(&store, &allowed, &legacy, "artifacts/files/a/v1.txt")
        .await
        .unwrap();
    assert_eq!(imported.media_type.as_deref(), Some("text/plain"));
    assert_eq!(imported.size_bytes, 11);
}
