use super::*;

use tinystoragedrivers_api::Storage;
use tinystoragedrivers_testkit::memory_storage;
use tinystoragedrivers_types::Scope;

fn storage() -> Storage {
    memory_storage(&Scope::new("artifact-files-test").unwrap())
}

#[tokio::test]
async fn blob_adapter_reads_metadata_and_deletes_only_the_requested_prefix() {
    let storage = storage();
    let first = BlobFileStore::new(&storage);
    let second = BlobFileStore::new(&storage);
    let metadata = first
        .put("artifacts/files/a/v1.png", b"png bytes", None)
        .await
        .unwrap();

    assert_eq!(metadata.size_bytes, 9);
    assert_eq!(metadata.media_type.as_deref(), Some("image/png"));
    assert_eq!(
        second.read("artifacts/files/a/v1.png").await.unwrap(),
        b"png bytes"
    );
    assert_eq!(
        second.metadata("artifacts/files/a/v1.png").await.unwrap(),
        Some(metadata)
    );
    second
        .put("artifacts/files/ab/v1.png", b"sibling", None)
        .await
        .unwrap();
    second.delete_prefix("artifacts/files/a/").await.unwrap();
    assert_eq!(
        first.metadata("artifacts/files/a/v1.png").await.unwrap(),
        None
    );
    assert_eq!(
        first.read("artifacts/files/ab/v1.png").await.unwrap(),
        b"sibling"
    );
}

#[tokio::test]
async fn blob_adapter_rejects_invalid_keys_and_prefixes_without_touching_siblings() {
    let storage = storage();
    let store = BlobFileStore::new(&storage);
    assert!(store.read("artifacts/files/missing.txt").await.is_err());
    assert_eq!(
        store.metadata("artifacts/files/missing.txt").await.unwrap(),
        None
    );
    for key in [
        "../outside",
        "artifacts/files/a//v1",
        "artifacts/files/a/./v1",
        "/absolute",
        "bad\\key",
    ] {
        assert!(store.put(key, b"bad", None).await.is_err(), "{key}");
        assert!(store.read(key).await.is_err(), "{key}");
        assert!(store.metadata(key).await.is_err(), "{key}");
    }
    store
        .put("artifacts/files/a/v1.txt", b"a", None)
        .await
        .unwrap();
    store
        .put("artifacts/files/ab/v1.txt", b"ab", None)
        .await
        .unwrap();

    for prefix in [
        "",
        "artifacts/files/a",
        "artifacts/files/a//",
        "artifacts/files/a/./",
        "../",
    ] {
        assert!(store.delete_prefix(prefix).await.is_err(), "{prefix}");
        assert_eq!(store.read("artifacts/files/a/v1.txt").await.unwrap(), b"a");
        assert_eq!(
            store.read("artifacts/files/ab/v1.txt").await.unwrap(),
            b"ab"
        );
    }
    store
        .delete_prefix("artifacts/files/missing/")
        .await
        .unwrap();
}
