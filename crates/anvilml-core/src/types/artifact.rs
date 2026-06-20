//! Artifact metadata type for the AnvilML job system.
//!
//! Defines `ArtifactMeta`, which records information about files produced
//! by a job execution (e.g. generated images, intermediate outputs).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Metadata for a file artifact produced by a job.
///
/// Created by the job system when a worker finishes producing output files.
/// The `hash` field contains a SHA-256 hex digest of the artifact contents,
/// used for deduplication and integrity verification.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ArtifactMeta {
    /// Unique artifact identifier, assigned by the job system.
    pub id: String,
    /// ID of the job that produced this artifact.
    pub job_id: Uuid,
    /// SHA-256 hex digest of the artifact contents (64 lowercase hex chars).
    pub hash: String,
    /// Image width in pixels, if the artifact is a PNG image. Zero when unknown.
    pub width: u32,
    /// Image height in pixels, if the artifact is a PNG image. Zero when unknown.
    pub height: u32,
    /// Filesystem path to the artifact file.
    pub path: String,
    /// Size of the artifact file on disk in bytes.
    pub size_bytes: u64,
    /// Timestamp when this artifact was created.
    pub created_at: DateTime<Utc>,
}
