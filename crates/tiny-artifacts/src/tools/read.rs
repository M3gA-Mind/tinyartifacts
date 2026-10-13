//! Read a single artifact by id with its full version history.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tinytools::{Tool, ToolResult};

use crate::store::ArtifactStore;
use crate::tools::read_required_str;

/// Fetch one artifact with its full version history.
pub struct ReadArtifactTool {
    store: Arc<dyn ArtifactStore>,
}

impl std::fmt::Debug for ReadArtifactTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadArtifactTool").finish_non_exhaustive()
    }
}

impl ReadArtifactTool {
    /// Constructs the tool bound to one store.
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ReadArtifactTool {
    fn name(&self) -> &str {
        "read_artifact"
    }

    fn description(&self) -> &str {
        "Fetch one artifact by its `artifact_id`, returning its full record \
         with all versions, timestamps and authorship. Use `list_artifacts` \
         first to discover artifact ids."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The artifact id (UUID) to fetch."
                }
            },
            "required": ["artifact_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let id = read_required_str(&args, "artifact_id")?;
        match self.store.get(&id).await? {
            Some(record) => Ok(ToolResult::success(serde_json::to_string(&record)?)),
            None => Ok(ToolResult::error(format!("artifact `{id}` not found"))),
        }
    }

    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        true
    }
}
