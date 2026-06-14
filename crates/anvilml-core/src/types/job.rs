//! Job domain types for the AnvilML scheduler.
//!
//! Defines the core types that represent a job through its lifecycle:
//! `JobStatus` (enum of lifecycle states), `JobSettings` (user preferences),
//! `SubmitJobRequest` (client submission), `SubmitJobResponse` (server
//! acknowledgment), and `Job` (the persisted job record).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Status of a job in its lifecycle.
///
/// Transitions: `Queued` → `Running` → `Completed` | `Failed` | `Cancelled`.
/// `Cancelled` can be reached from `Queued` or `Running`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum JobStatus {
    /// Job is waiting in the queue for a worker to pick it up.
    Queued,
    /// Job is currently executing on a worker.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed — an error occurred during execution.
    Failed,
    /// Job was cancelled by the user or scheduler before completion.
    Cancelled,
}

/// User-provided settings for a job submission.
///
/// Currently only specifies a device preference. When `device_preference` is
/// `None`, the scheduler auto-selects a device based on available VRAM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
pub struct JobSettings {
    /// Requested device. `None` = auto-select by VRAM.
    pub device_preference: Option<String>,
}

/// A client request to submit a new job.
///
/// The `graph` field contains the submitted computation graph as opaque JSON.
/// The Rust types do not interpret the graph contents — that is handled by
/// the Python worker nodes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SubmitJobRequest {
    /// Submitted graph JSON; opaque to Rust.
    pub graph: serde_json::Value,
    /// Job settings (device preference, etc.).
    pub settings: JobSettings,
}

/// The server's response to a successful job submission.
///
/// Contains the assigned job ID and the job's position in the queue.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SubmitJobResponse {
    /// Unique identifier assigned to the new job.
    pub job_id: Uuid,
    /// Position in the queue (1-based index).
    pub queue_position: u32,
}

/// A persisted job record.
///
/// Created by the scheduler at submission time with a `created_at` timestamp.
/// Fields like `started_at`, `completed_at`, `worker_id`, and `error` are
/// populated as the job progresses through its lifecycle.
///
/// `Job` intentionally does not derive `Default` because `created_at` must
/// always reflect the actual submission time — a zero timestamp would be
/// meaningless.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    /// Unique job identifier, assigned at submission.
    pub id: Uuid,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// Submitted computation graph as JSON.
    pub graph: serde_json::Value,
    /// Job settings at submission time.
    pub settings: JobSettings,
    /// Timestamp when the job was created by the scheduler.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the job started executing. `None` if not yet started.
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when the job completed (successfully or with failure).
    /// `None` if not yet completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// ID of the worker that is executing or executed this job.
    /// `None` if the job has not been dispatched yet.
    pub worker_id: Option<String>,
    /// Error message if the job failed. `None` if the job is still running
    /// or has not yet failed.
    pub error: Option<String>,
    /// Current position in the queue. `None` once the job starts executing.
    pub queue_position: Option<u32>,
}
