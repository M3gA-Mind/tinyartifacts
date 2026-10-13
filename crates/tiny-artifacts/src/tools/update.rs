//! Update an existing artifact by appending a new version.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tinytools::{PermissionLevel, Tool, ToolResult};

use crate::store::ArtifactStore;
use crate::tools::read_required_str;
use crate::types::{ArtifactAuthor, ArtifactVersion};

/// Append a new revision to an existing artifact.
pub struct UpdateArtifactTool {
    store: Arc<dyn ArtifactStore>,
}

impl std::fmt::Debug for UpdateArtifactTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateArtifactTool").finish_non_exhaustive()
    }
}

impl UpdateArtifactTool {
    /// Constructs the tool bound to one store.
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for UpdateArtifactTool {
    fn name(&self) -> &str {
        "update_artifact"
    }

    fn description(&self) -> &str {
        "Append a new revision to an existing artifact. The body you pass \
         becomes a new version with `author: agent`. Use `read_artifact` \
         first to see the full version history and get the `artifact_id`. \
         Returns the new version number."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The artifact id (UUID) to update."
                },
                "body": {
                    "type": "string",
                    "description": "The new revision content."
                },
                "note": {
                    "type": "string",
                    "description": "Optional note about this revision (e.g. why it was changed)."
                }
            },
            "required": ["artifact_id", "body"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let id = read_required_str(&args, "artifact_id")?;
        let body = read_required_str(&args, "body")?;
        let note = args.get("note").and_then(|v| v.as_str()).map(|s| s.to_string());

        let mut record = match self.store.get(&id).await? {
            Some(r) => r,
            None => return Ok(ToolResult::error(format!("artifact `{id}` not found"))),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64;

        let v = ArtifactVersion::new(0, &body, ArtifactAuthor::Agent, "agent", now);
        let next = record.push_version(v);
        // Stamp the note onto the version we just pushed
        if let Some(n) = note {
            if let Some(latest) = record.versions.last_mut() {
                latest.note = Some(n);
            }
        }

        self.store.upsert(&record).await?;

        Ok(ToolResult::success(json!({
            "artifact_id": id,
            "version": next,
            "title": record.title,
        }).to_string()))
    }

    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        false
    }
}
