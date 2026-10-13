//! LLM-callable tools over the artifact domain.
//!
//! Four tools: `create_artifact` (mint a record with its first version),
//! `read_artifact` (fetch a record with its full version history),
//! `update_artifact` (append a revision to an existing record), and
//! `list_artifacts` (enumerate records, newest first). Each takes an
//! [`ArtifactStore`](crate::ArtifactStore) so reads and writes land in the
//! host's storage.

mod create;
mod list;
mod read;
mod update;

pub use create::CreateArtifactTool;
pub use list::ListArtifactsTool;
pub use read::ReadArtifactTool;
pub use update::UpdateArtifactTool;

use std::sync::Arc;

use tinytools::Tool;

use crate::ArtifactStore;

/// Build this crate's tools in their persisted catalogue order, bound to one
/// store.
pub fn registered_tools(store: Arc<dyn ArtifactStore>) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(CreateArtifactTool::new(store.clone())),
        Box::new(ReadArtifactTool::new(store.clone())),
        Box::new(UpdateArtifactTool::new(store.clone())),
        Box::new(ListArtifactsTool::new(store)),
    ]
}

/// Shared helper: read a required, non-empty string arg.
pub(crate) fn read_required_str(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    let raw = args
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match raw {
        Some(s) => Ok(s.to_string()),
        None => Err(anyhow::anyhow!("missing required string argument `{key}`")),
    }
}
