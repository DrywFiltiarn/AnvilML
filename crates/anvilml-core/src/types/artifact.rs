//! Artifact domain types per ANVILML_DESIGN §4.2.
//!
//! Defines `ArtifactMeta` — a pure, serializable data structure with zero
//! I/O or async logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Metadata about an artifact produced by a generation job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArtifactMeta {
    /// Unique identifier for this artifact (UUID v4).
    #[serde(default)]
    #[schema(required)]
    pub id: Uuid,
    /// The model that produced this artifact.
    pub model_id: String,
    /// The job that produced this artifact.
    #[serde(default)]
    #[schema(required)]
    pub job_id: Uuid,
    /// Filesystem path to the artifact file.
    #[schema(value_type = String)]
    pub path: std::path::PathBuf,
    /// MIME type hint (e.g. "image/png").
    #[serde(default)]
    pub mime_hint: Option<String>,
    /// Size of the artifact file in bytes.
    #[serde(default)]
    pub size_bytes: u64,
    /// When the artifact was created.
    pub created_at: DateTime<Utc>,
}

impl Default for ArtifactMeta {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            model_id: String::new(),
            job_id: Uuid::nil(),
            path: std::path::PathBuf::new(),
            mime_hint: None,
            size_bytes: 0,
            created_at: Utc::now(),
        }
    }
}

/// Input metadata carried into [`ArtifactSave::save`].
#[derive(Debug, Clone)]
pub struct ArtifactSaveInput {
    /// Image width in pixels.
    pub width: i64,
    /// Image height in pixels.
    pub height: i64,
    /// Generation seed.
    pub seed: i64,
    /// Number of diffusion steps.
    pub steps: i64,
    /// Generation prompt text.
    pub prompt: String,
}

/// Trait for saving an artifact produced by a generation job.
///
/// Implemented by `ArtifactStore` in `anvilml-server`. Placed here to avoid
/// a circular dependency: `anvilml-scheduler` → `anvilml-server` would create
/// a cycle since `anvilml-server` already depends on `anvilml-scheduler`.
#[async_trait::async_trait]
pub trait ArtifactSave: Send + Sync {
    /// Decode, hash, persist, and record a single artifact.
    ///
    /// Returns the artifact hash on success.
    async fn save(
        &self,
        job_id: &str,
        image_b64: &str,
        meta: ArtifactSaveInput,
    ) -> Result<String, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_meta_roundtrip() {
        let now = Utc::now();
        let meta = ArtifactMeta {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            model_id: "model-001".to_string(),
            job_id: Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap(),
            path: std::path::PathBuf::from("/artifacts/output.png"),
            mime_hint: Some("image/png".to_string()),
            size_bytes: 2_400_000,
            created_at: now,
        };

        let json = serde_json::to_string(&meta).expect("serialize ArtifactMeta");
        let restored: ArtifactMeta = serde_json::from_str(&json).expect("deserialize ArtifactMeta");

        assert_eq!(restored.id, meta.id);
        assert_eq!(restored.model_id, meta.model_id);
        assert_eq!(restored.job_id, meta.job_id);
        assert_eq!(restored.path, meta.path);
        assert_eq!(restored.mime_hint, meta.mime_hint);
        assert_eq!(restored.size_bytes, meta.size_bytes);
        assert_eq!(restored.created_at, meta.created_at);
    }

    #[test]
    fn artifact_meta_defaults() {
        let minimal = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "model_id": "model-001",
            "job_id": "660e8400-e29b-41d4-a716-446655440001",
            "path": "/artifacts/output.png",
            "created_at": "2024-01-01T00:00:00Z"
        });

        let meta: ArtifactMeta =
            serde_json::from_value(minimal).expect("minimal ArtifactMeta parses");

        assert_eq!(meta.model_id, "model-001");
        assert!(
            meta.mime_hint.is_none(),
            "mime_hint must be None when absent"
        );
        assert_eq!(meta.size_bytes, 0, "size_bytes must default to 0");
    }

    #[test]
    fn artifact_meta_default_impl() {
        let meta = ArtifactMeta::default();
        assert!(meta.id.is_nil());
        assert!(meta.model_id.is_empty());
        assert!(meta.job_id.is_nil());
        assert!(meta.path.as_os_str().is_empty());
        assert!(meta.mime_hint.is_none());
        assert_eq!(meta.size_bytes, 0);
    }

    #[test]
    fn artifact_meta_json_preserves_fields() {
        let meta = ArtifactMeta {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            model_id: "model-001".to_string(),
            job_id: Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap(),
            path: std::path::PathBuf::from("/artifacts/output.png"),
            mime_hint: Some("image/png".to_string()),
            size_bytes: 2_400_000,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string_pretty(&meta).expect("serialize ArtifactMeta");
        assert!(json.contains("\"id\": \"550e8400-e29b-41d4-a716-446655440000\""));
        assert!(json.contains("\"model_id\": \"model-001\""));
        assert!(json.contains("\"job_id\": \"660e8400-e29b-41d4-a716-446655440001\""));
        assert!(json.contains("\"mime_hint\": \"image/png\""));
    }

    #[test]
    fn artifact_meta_optional_uuid_nil() {
        let minimal = serde_json::json!({
            "model_id": "m1",
            "path": "/artifacts/x",
            "created_at": "2024-01-01T00:00:00Z"
        });

        let meta: ArtifactMeta = serde_json::from_value(minimal).expect("parses without id/job_id");
        assert!(meta.id.is_nil(), "id must be nil UUID when absent");
        assert!(meta.job_id.is_nil(), "job_id must be nil UUID when absent");
    }
}
