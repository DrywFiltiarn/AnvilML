//! WebSocket event types for the job lifecycle — the variants broadcast to `/v1/events`
//! subscribers as a job moves through the pipeline.
//!
//! Events are serialised with a `"type"` tag field (via `#[serde(tag = "type")]`) so that
//! consumers can dispatch on the variant without knowing the full payload schema in advance.
//! The tag value is the snake_case form of the variant name (e.g. `"job_queued"`).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A WebSocket event emitted during a generation job's lifecycle.
///
/// Events are serialised with a `"type"` tag field so that subscribers to the
/// `/v1/events` WebSocket endpoint can dispatch on the variant without knowing
/// the full payload schema in advance. The seven variants cover the job states
/// from queue entry through completion, failure, or cancellation.
///
/// # Serde tag
///
/// The `#[serde(tag = "type", rename_all = "snake_case")]` attribute means each
/// event serialises to JSON like:
/// ```json
/// {"type": "job_queued", "job_id": "...", "queue_position": 3}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// A job has been queued and is waiting for a worker.
    ///
    /// Emitted when a job is first submitted to the scheduler.
    JobQueued {
        /// UUID of the queued job.
        job_id: Uuid,
        /// Position in the queue (1-based).
        queue_position: u32,
    },

    /// A worker has claimed the job and begun execution.
    ///
    /// Emitted when a worker picks up a queued job.
    JobStarted {
        /// UUID of the started job.
        job_id: Uuid,
        /// Identifier of the worker that claimed the job (e.g. `"gpu:0"`).
        worker_id: String,
    },

    /// Progress update during job execution.
    ///
    /// Emitted periodically as the worker advances through diffusion steps.
    /// The `preview_b64` field may contain a base64-encoded PNG preview of
    /// the current step's latent output.
    JobProgress {
        /// UUID of the in-progress job.
        job_id: Uuid,
        /// Current step number (0-based).
        step: u32,
        /// Total number of steps in the generation.
        total_steps: u32,
        /// Optional base64-encoded PNG preview of the current step.
        preview_b64: Option<String>,
    },

    /// A generation artifact is ready for retrieval.
    ///
    /// Emitted when the worker finishes producing the final image.
    JobImageReady {
        /// UUID of the completed job.
        job_id: Uuid,
        /// SHA-256 hex hash of the generated artifact.
        artifact_hash: String,
        /// Generated image width in pixels.
        width: u32,
        /// Generated image height in pixels.
        height: u32,
        /// Random seed used for this generation.
        seed: i64,
        /// Number of diffusion steps executed.
        steps: u32,
    },

    /// The job completed successfully.
    ///
    /// Emitted after `JobImageReady` to signal the end of the job lifecycle.
    JobCompleted {
        /// UUID of the completed job.
        job_id: Uuid,
        /// Wall-clock time from job start to completion, in milliseconds.
        elapsed_ms: u64,
    },

    /// The job failed during execution.
    ///
    /// Emitted when the worker encounters an unrecoverable error.
    JobFailed {
        /// UUID of the failed job.
        job_id: Uuid,
        /// Human-readable error description from the worker.
        error: String,
    },

    /// The job was cancelled by the user or scheduler.
    ///
    /// Emitted when a running or queued job is explicitly cancelled.
    JobCancelled {
        /// UUID of the cancelled job.
        job_id: Uuid,
    },
}
