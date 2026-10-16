//! File references: helpers, media types, and paths that cannot be read.

// Unwrapping is fine in tests (AGENTS.md); the workspace lints keep it out of
// library code.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use tiny_artifacts::{media_type_for, FileRef, FileRefError, FileRoots};

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tiny-artifacts-ref-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn a_resolved_reference_knows_its_name_extension_and_type() {
    let root = dir();
    std::fs::write(root.join("Report.PDF"), b"%PDF").unwrap();
    let roots = FileRoots::new([root.clone()]);
    assert_eq!(roots.roots(), [root]);

    let file = roots.resolve("Report.PDF").await.unwrap();
    assert_eq!(file.file_name(), Some("Report.PDF"));
    assert_eq!(file.extension().as_deref(), Some("pdf"));
    assert_eq!(file.media_type.as_deref(), Some("application/pdf"));
    assert_eq!(file.size_bytes, 4);

    let bare = FileRef {
        path: "/tmp/README".into(),
        size_bytes: 0,
        media_type: None,
    };
    assert_eq!(bare.extension(), None);
}

#[test]
fn media_types_cover_the_common_artifact_formats() {
    let cases = [
        ("a.png", Some("image/png")),
        ("a.jpeg", Some("image/jpeg")),
        ("a.gif", Some("image/gif")),
        ("a.webp", Some("image/webp")),
        ("a.svg", Some("image/svg+xml")),
        ("a.mp4", Some("video/mp4")),
        ("a.webm", Some("video/webm")),
        ("a.mov", Some("video/quicktime")),
        (
            "a.docx",
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        ),
        (
            "a.pptx",
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        ),
        (
            "a.xlsx",
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        ),
        ("a.csv", Some("text/csv")),
        ("a.json", Some("application/json")),
        ("a.zip", Some("application/zip")),
        ("a.md", Some("text/markdown")),
        ("a.htm", Some("text/html")),
        ("a.txt", Some("text/plain")),
        ("a.unknown", None),
        ("no-extension", None),
    ];
    for (name, expected) in cases {
        assert_eq!(media_type_for(Path::new(name)), expected, "{name}");
    }
}

#[tokio::test]
async fn an_empty_path_is_refused() {
    let roots = FileRoots::new([dir()]);
    assert!(matches!(
        roots.resolve("   ").await,
        Err(FileRefError::Empty)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn a_path_that_cannot_be_read_is_an_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let root = dir();
    let locked = root.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("f.txt"), b"x").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let result = FileRoots::new([root]).resolve("locked/f.txt").await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Root can read anything; the refusal only shows for an ordinary user.
    if let Err(err) = result {
        assert!(matches!(err, FileRefError::Io { .. }), "{err}");
        assert!(err.to_string().contains("locked/f.txt"));
    }
}

#[tokio::test]
async fn the_host_guard_refuses_a_path_inside_the_roots() {
    let root = dir();
    std::fs::create_dir_all(root.join(".ssh")).unwrap();
    std::fs::write(root.join(".ssh").join("id_ed25519"), b"key").unwrap();
    std::fs::write(root.join("notes.txt"), b"fine").unwrap();
    let roots = FileRoots::new([root.clone()]).with_guard(|path| {
        path.components()
            .any(|c| c.as_os_str() == ".ssh")
            .then(|| "a credential store".to_string())
    });

    // Checked on the canonical path, whatever the spelling.
    for raw in [".ssh/id_ed25519", "./.ssh/id_ed25519"] {
        let result = roots.resolve(raw).await;
        assert!(
            matches!(
                &result,
                Err(FileRefError::Refused { path, reason })
                    if path == raw && reason == "a credential store"
            ),
            "{raw}: {result:?}"
        );
    }
    let err = roots.resolve(".ssh/id_ed25519").await.unwrap_err();
    assert!(err
        .to_string()
        .contains("cannot be referenced: a credential store"));
    assert!(roots.resolve("notes.txt").await.is_ok());
}

#[test]
fn a_guard_shows_in_debug_and_equality() {
    let plain = FileRoots::new([PathBuf::from("/a")]);
    let guarded = plain.clone().with_guard(|_| None);
    assert!(format!("{guarded:?}").contains("guard: true"));
    assert!(format!("{plain:?}").contains("guard: false"));
    assert_ne!(plain, guarded);
    assert_eq!(guarded, guarded.clone());
    assert_eq!(plain, FileRoots::new([PathBuf::from("/a")]));
    assert_ne!(guarded, plain.clone().with_guard(|_| None));
}
