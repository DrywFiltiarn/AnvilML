use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;
use uuid::Uuid;

/// Metadata for a generated, content-addressed PNG artifact.
///
/// This struct captures the identity, generation parameters, and
/// storage location of a single output artifact produced by a worker.
/// The `hash` field is the SHA-256 hex content address used by
/// `anvilml-artifacts` as its primary key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ArtifactMeta {
    /// SHA-256 hex content address — primary key for artifact storage.
    pub hash: String,
    /// UUID of the job that produced this artifact.
    pub job_id: Uuid,
    /// Generated image width in pixels.
    pub width: u32,
    /// Generated image height in pixels.
    pub height: u32,
    /// Random seed used for generation (i64 to support negative seeds).
    pub seed: i64,
    /// Number of diffusion steps used in generation.
    pub steps: u32,
    /// Timestamp when the artifact was created.
    pub created_at: DateTime<Utc>,
    /// Filesystem path to the saved PNG file.
    #[schema(value_type = String)]
    pub file_path: PathBuf,
}
