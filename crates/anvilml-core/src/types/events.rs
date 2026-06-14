//! WebSocket broadcast event types for the AnvilML system.
//!
//! Defines `WsEvent` — a tagged enum covering all event types broadcast
//! over the WebSocket stream to connected clients. The enum uses
//! `#[serde(tag = "type", rename_all = "snake_case")]` so each variant
//! serialises as `{"type": "job_queued", ...fields...}`.
//!
//! Per `ANVILML_DESIGN.md §5.8`, there are ten variants: seven job-life-cycle
//! events (`JobQueued` through `JobCancelled`), one worker-lifecycle event
//! (`WorkerStatusChanged`), one periodic metrics event (`SystemStats`), and
//! one provisioning event (`ProvisioningProgress`).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::types::worker::{WorkerInfo, WorkerStatus};

/// All event types broadcast over the WebSocket stream to connected clients.
///
/// Each variant carries domain-specific data relevant to that event.
/// The `#[serde(tag = "type")]` attribute causes every serialised event
/// to include a `"type"` key whose value is the snake_case variant name
/// (e.g. `"job_queued"`, `"job_completed"`), enabling clients to dispatch
/// on the event type before inspecting fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// A job has been placed in the scheduler queue.
    ///
    /// Sent immediately after a job is accepted and assigned a queue position.
    /// Clients can use `queue_position` to show ordering to the user.
    JobQueued {
        /// UUID of the newly queued job.
        job_id: Uuid,
        /// 1-based position in the scheduler queue.
        queue_position: u32,
    },

    /// A job has been dispatched to a worker for execution.
    ///
    /// Sent when the scheduler assigns the job to a specific worker.
    /// The `worker_id` identifies which worker will process the job.
    JobStarted {
        /// UUID of the job being executed.
        job_id: Uuid,
        /// Stable identity of the worker that received the job (e.g. `"worker-0"`).
        worker_id: String,
    },

    /// An execution progress update for a running job.
    ///
    /// Sent periodically during job execution. The `preview_b64` field
    /// may contain a base64-encoded thumbnail of the current generation
    /// state; it is `None` when no preview is available at this step.
    JobProgress {
        /// UUID of the job being executed.
        job_id: Uuid,
        /// Current step number (0-based or 1-based depending on the model).
        step: u32,
        /// Total number of steps for this job.
        total_steps: u32,
        /// Base64-encoded preview image, or `None` if unavailable.
        preview_b64: Option<String>,
    },

    /// A generated image is available for a completed job.
    ///
    /// This is the most data-rich variant, carrying the artifact hash
    /// and image dimensions so clients can display a preview without
    /// fetching the full artifact. The `seed` field is included for
    /// reproducibility.
    JobImageReady {
        /// UUID of the job that produced the image.
        job_id: Uuid,
        /// SHA-256 hash of the generated artifact file.
        artifact_hash: String,
        /// Generated image width in pixels.
        width: u32,
        /// Generated image height in pixels.
        height: u32,
        /// Random seed used for generation (for reproducibility).
        seed: i64,
        /// Total number of steps executed for this job.
        steps: u32,
    },

    /// A job finished successfully.
    ///
    /// Sent after the worker reports completion. `elapsed_ms` gives the
    /// total wall-clock time from dispatch to completion, useful for
    /// performance metrics and user feedback.
    JobCompleted {
        /// UUID of the completed job.
        job_id: Uuid,
        /// Total execution time in milliseconds.
        elapsed_ms: u64,
    },

    /// A job failed with an error.
    ///
    /// The `error` field contains a human-readable error message from
    /// the worker or scheduler. Clients should display this to the user
    /// and disable retry controls if the error is non-recoverable.
    JobFailed {
        /// UUID of the failed job.
        job_id: Uuid,
        /// Human-readable error message from the worker or scheduler.
        error: String,
    },

    /// A job was cancelled by the user or an external request.
    ///
    /// No additional context fields are needed — the cancellation is
    /// a terminal state with no error or result data.
    JobCancelled {
        /// UUID of the cancelled job.
        job_id: Uuid,
    },

    /// A worker's lifecycle state has changed.
    ///
    /// Sent by the Rust supervisor whenever a worker transitions between
    /// states (`Initializing`, `Idle`, `Busy`, `Dead`, `Respawning`).
    /// Clients can use this to update the worker status display.
    WorkerStatusChanged {
        /// Stable worker identity (e.g. `"worker-0"`).
        worker_id: String,
        /// The new lifecycle state of the worker.
        status: WorkerStatus,
        /// Zero-based GPU device index this worker is bound to.
        device_index: u32,
    },

    /// Periodic system metrics snapshot.
    ///
    /// Contains CPU and memory utilisation plus a snapshot of all known
    /// workers (via `Vec<WorkerInfo>`). Sent at a configurable interval
    /// (default: every 5 seconds) to keep connected clients informed
    /// about system health.
    SystemStats {
        /// CPU utilisation as a percentage (0.0–100.0).
        cpu_pct: f32,
        /// Current RAM usage in mebibytes.
        ram_used_mib: u64,
        /// Snapshot of all known workers, including their states.
        workers: Vec<WorkerInfo>,
    },

    /// Progress update for the Python worker provisioning process.
    ///
    /// Sent during server startup while the Python virtual environment
    /// is being provisioned. The `message` field provides human-readable
    /// context (e.g. "Installing torch", "Verifying import"), and `pct`
    /// gives a rough percentage of completion (0–100).
    ProvisioningProgress {
        /// Human-readable description of the current provisioning step.
        message: String,
        /// Approximate completion percentage (0–100).
        pct: u8,
    },
}
