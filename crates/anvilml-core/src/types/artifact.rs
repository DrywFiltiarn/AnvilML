//! Artifact metadata types — tracking outputs produced by jobs.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ArtifactMeta — metadata about a produced artifact
// ---------------------------------------------------------------------------

/// Metadata describing an artifact produced by a job.
///
/// Artifacts are the outputs of model execution — generated images,
/// embeddings, logs, etc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct ArtifactMeta {
    /// Unique identifier for this artifact (UUID v4).
    pub id: Uuid,

    /// The job that produced this artifact.
    pub job_id: Uuid,

    /// The model used to produce this artifact.
    pub model_id: Uuid,

    /// Filesystem path to the artifact file.
    pub path: String,

    /// Human-readable type label (e.g. "image/png", "embedding").
    pub artifact_type: String,

    /// Timestamp when the artifact was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
}

impl ArtifactMeta {
    /// Create a new `ArtifactMeta` with the current timestamp.
    pub fn new(
        job_id: Uuid,
        model_id: Uuid,
        path: String,
        artifact_type: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_id,
            model_id,
            path,
            artifact_type,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // ArtifactMeta — construction and serialization
    // ------------------------------------------------------------------

    #[test]
    fn artifact_meta_new() {
        let job_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        let artifact = ArtifactMeta::new(
            job_id,
            model_id,
            "/artifacts/output.png".into(),
            "image/png".into(),
        );
        assert_eq!(artifact.job_id, job_id);
        assert_eq!(artifact.model_id, model_id);
        assert_eq!(artifact.path, "/artifacts/output.png");
        assert_eq!(artifact.artifact_type, "image/png");
        // id should be a valid UUID v4
        assert_eq!(artifact.id.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn artifact_meta_serialization_round_trip() {
        let artifact = ArtifactMeta {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            model_id: Uuid::new_v4(),
            path: "/artifacts/embedding.npy".into(),
            artifact_type: "embedding".into(),
            created_at: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&artifact).unwrap();
        let back: ArtifactMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(artifact, back);
    }

    #[test]
    fn artifact_meta_datetime_serialization() {
        let artifact = ArtifactMeta {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            model_id: Uuid::new_v4(),
            path: "/artifacts/test".into(),
            artifact_type: "test".into(),
            created_at: DateTime::parse_from_rfc3339("2025-06-15T12:30:45Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&artifact).unwrap();
        // Verify timestamp is serialized as integer (unix seconds)
        assert!(json.contains("created_at"));
        // Deserialize and verify timestamp matches
        let back: ArtifactMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(
            artifact.created_at.timestamp(),
            back.created_at.timestamp()
        );
    }
}
