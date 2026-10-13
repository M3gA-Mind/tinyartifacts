use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::store::ArtifactStore;
use crate::types::{ArtifactAuthor, ArtifactKind, ArtifactRecord};

use tinytools::traits::{PermissionLevel, Tool, ToolResult};

/// Create a new artifact with its first version.
pub struct CreateArtifactTool {
    store: Arc<dyn ArtifactStore>,
    /// Default author_id to stamp when not provided.
    default_author_id: String,
}

impl CreateArtifactTool {
    pub fn new(store: Arc<dyn ArtifactStore>, default_author_id: impl Into<String>) -> Self {
        Self {
            store,
            default_author_id: default_author_id.into(),
        }
    }
}

#[async_trait]
impl Tool for CreateArtifactTool {
    fn name(&self) -> &str {
        "create_artifact"
    }

    fn description(&self) -> &str {
        "Create a new artifact (a versioned deliverable) with the given body and format. \
         Use this to produce finished output — a spec, a report, markdown docs, HTML, \
         or any text-based deliverable. The artifact is stored as version 1; call \
         `update_artifact` later to revise it."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short display title for the artifact."
                },
                "body": {
                    "type": "string",
                    "description": "The content of the artifact."
                },
                "kind": {
                    "type": "string",
                    "enum": ["text", "markdown", "html", "image", "file", "presentation", "document"],
                    "description": "What kind of artifact. Inferred from the title/body when omitted."
                },
                "task_id": {
                    "type": "string",
                    "description": "Optional task or workflow id this artifact belongs to."
                }
            },
            "required": ["title", "body"],
            "additionalProperties": false
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let title = args
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let body = args
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let kind = args
            .get("kind")
            .and_then(Value::as_str)
            .map(ArtifactKind::parse)
            .unwrap_or_else(|| {
                if title.ends_with(".md") || title.contains(".md") {
                    ArtifactKind::Markdown
                } else if title.ends_with(".html") {
                    ArtifactKind::Markdown
                } else {
                    ArtifactKind::Text
                }
            });
        let task_id = args.get("task_id").and_then(Value::as_str);

        let id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut record = ArtifactRecord::new(
            &id,
            kind,
            title,
            body,
            &self.default_author_id,
            now,
        );
        record.task_id = task_id.map(|s| s.to_string());

        self.store.upsert(&record).await?;

        Ok(ToolResult::success(serde_json::to_string_pretty(&record)?))
    }
}

/// Update an existing artifact by pushing a new version.
pub struct UpdateArtifactTool {
    store: Arc<dyn ArtifactStore>,
    default_author_id: String,
}

impl UpdateArtifactTool {
    pub fn new(store: Arc<dyn ArtifactStore>, default_author_id: impl Into<String>) -> Self {
        Self {
            store,
            default_author_id: default_author_id.into(),
        }
    }
}

#[async_trait]
impl Tool for UpdateArtifactTool {
    fn name(&self) -> &str {
        "update_artifact"
    }

    fn description(&self) -> &str {
        "Update an existing artifact by adding a new version. The artifact's full history \
         is preserved — previous versions remain readable. Use this to revise a deliverable \
         based on feedback or new information."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The id of the artifact to update."
                },
                "body": {
                    "type": "string",
                    "description": "The new content for this version."
                },
                "note": {
                    "type": "string",
                    "description": "Optional one-line reason for this revision, e.g. 'incorporated feedback'."
                }
            },
            "required": ["artifact_id", "body"],
            "additionalProperties": false
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let artifact_id = args
            .get("artifact_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing required argument `artifact_id`"))?;
        let body = args
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing required argument `body`"))?;
        let note = args.get("note").and_then(Value::as_str).map(|s| s.to_string());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut record = self.store.get(artifact_id).await?;
        record.push_version(body, ArtifactAuthor::Agent, &self.default_author_id, now, note);
        self.store.upsert(&record).await?;

        Ok(ToolResult::success(serde_json::to_string_pretty(&record)?))
    }
}

/// Read an artifact's full record including all versions.
pub struct ReadArtifactTool {
    store: Arc<dyn ArtifactStore>,
}

impl ReadArtifactTool {
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
        "Read a single artifact by id, returning its full record including all versions, \
         authorship, and timestamps. Use this to see the version history of a deliverable."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The id of the artifact to read."
                }
            },
            "required": ["artifact_id"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let artifact_id = args
            .get("artifact_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing required argument `artifact_id`"))?;

        match self.store.get(artifact_id).await {
            Ok(record) => Ok(ToolResult::success(serde_json::to_string_pretty(&record)?)),
            Err(e) => Ok(ToolResult::error(format!("artifact not found: {e}"))),
        }
    }
}

/// List artifacts, optionally filtered by task_id.
pub struct ListArtifactsTool {
    store: Arc<dyn ArtifactStore>,
}

impl ListArtifactsTool {
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ListArtifactsTool {
    fn name(&self) -> &str {
        "list_artifacts"
    }

    fn description(&self) -> &str {
        "List artifacts, newest first. Optionally filter by task_id to see artifacts \
         belonging to a specific task or workflow."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Optional task or workflow id to filter by."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id = args.get("task_id").and_then(Value::as_str);

        match self.store.list(task_id).await {
            Ok(artifacts) => Ok(ToolResult::success(serde_json::to_string_pretty(&artifacts)?)),
            Err(e) => Ok(ToolResult::error(format!("failed to list artifacts: {e}"))),
        }
    }
}

/// Delete an artifact. Default-OFF (Dangerous).
pub struct DeleteArtifactTool {
    store: Arc<dyn ArtifactStore>,
}

impl DeleteArtifactTool {
    pub fn new(store: Arc<dyn ArtifactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for DeleteArtifactTool {
    fn name(&self) -> &str {
        "delete_artifact"
    }

    fn description(&self) -> &str {
        "Permanently delete an artifact and all its versions. IRREVERSIBLE."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The id of the artifact to delete."
                }
            },
            "required": ["artifact_id"],
            "additionalProperties": false
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Dangerous
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let artifact_id = args
            .get("artifact_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing required argument `artifact_id`"))?;

        match self.store.delete(artifact_id).await {
            Ok(true) => Ok(ToolResult::success(json!({ "deleted": true }).to_string())),
            Ok(false) => Ok(ToolResult::error("artifact not found")),
            Err(e) => Ok(ToolResult::error(format!("failed to delete artifact: {e}"))),
        }
    }
}