//! Create a new artifact with its first version.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tinytools::{PermissionLevel, Tool, ToolResult};
use uuid::Uuid;

use crate::store::ArtifactStore;
use crate::tools::read_required_str;
use crate::types::{ArtifactKind, ArtifactRecord};

/// Create a new artifact with its first version. Use `update_artifact`
/// later to append revisions.
pub struct CreateArtifactTool {
    store: Arc<dyn ArtifactStore>,
}

impl std::fmt::Debug for CreateArtifactTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateArtifactTool").finish_non_exhaustive()
    }
}

impl CreateArtifactTool {
    /// Constructs the tool bound to one store.
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CreateArtifactTool {
    fn name(&self) -> &str {
        "create_artifact"
    }

    fn description(&self) -> &str {
        "Create a new artifact with its first version. Supported kinds: \
         `html` (HTML document), `markdown` (Markdown file), `text` (plain \
         text), `image` (image reference), `file` (file reference). Returns \
         the artifact id, title, and version number — use the id in later \
         calls to `read_artifact` or `update_artifact`."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["html", "markdown", "text", "image", "file"],
                    "description": "Artifact kind."
                },
                "title": {
                    "type": "string",
                    "description": "A short display title for the artifact."
                },
                "body": {
                    "type": "string",
                    "description": "The artifact content. For `html` kind, HTML source; for `markdown`, Markdown source; for other kinds, free-form text."
                }
            },
            "required": ["kind", "title", "body"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let kind_raw = read_required_str(&args, "kind")?;
        let kind = ArtifactKind::parse(&kind_raw)
            .ok_or_else(|| anyhow::anyhow!("invalid kind `{kind_raw}`; expected one of html, markdown, text, image, file"))?;
        let title = read_required_str(&args, "title")?;
        let body = read_required_str(&args, "body")?;

        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64;

        let record = ArtifactRecord::new(&id, &title, kind, &body, "agent", now);
        self.store.upsert(&record).await?;

        Ok(ToolResult::success(json!({
            "artifact_id": id,
            "title": title,
            "kind": kind.as_str(),
            "version": 1,
        }).to_string()))
    }

    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        false
    }
}
