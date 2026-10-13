//! Versioned artifact types, store trait, and tools.
//!
//! An artifact is a record with an ordered version list. Nothing is ever
//! overwritten: a revision appends a new version, which is what makes
//! `human_edit_diff` answerable at any later point.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use tiny_artifacts::{ArtifactStore, ArtifactKind, ArtifactRecord, registered_tools};
//!
//! // A no-op store for demonstration (replace with a real backend).
//! # fn main() {}  // make this integration-test only
//! ```

mod diff;
pub mod store;
pub mod tools;
pub mod types;

pub use diff::{ArtifactDiff, DiffLine, DiffOp, MAX_DIFF_LINES};
pub use store::ArtifactStore;
pub use tools::{registered_tools, CreateArtifactTool, ListArtifactsTool, ReadArtifactTool, UpdateArtifactTool};
pub use types::{ArtifactAuthor, ArtifactKind, ArtifactRecord, ArtifactVersion};

/// Helper: current unix epoch millis.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis() as u64
}
