use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The lifecycle status of a generation job.
///
/// Jobs progress through this state machine:
/// `Queued` → `Running` → (`Completed` | `Failed`) | `Cancelled` (at any point).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is waiting in the queue for a worker to pick it up.
    Queued,
    /// A worker has claimed the job and is executing it.
    Running,
    /// The job finished successfully; the output artifact is available.
    Completed,
    /// The job failed during execution; `error` may contain a diagnostic message.
    Failed,
    /// The job was explicitly cancelled before completion.
    Cancelled,
}

/// Optional settings that control how a job is executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JobSettings {
    /// Requested device. None = auto-select by VRAM.
    pub device_preference: Option<String>,
}

/// A single generation job submitted to the AnvilML scheduler.
///
/// A `Job` represents a request to run a computation graph on a worker.
/// It carries the graph definition, execution settings, lifecycle timestamps,
/// and the current status. Fields that are only populated after the job
/// enters a later lifecycle stage (`started_at`, `completed_at`, `worker_id`,
/// `error`, `queue_position`) are `Option` types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Job {
    /// Stable unique identifier for this job (UUID v4).
    pub id: Uuid,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// The computation graph to execute, in the format expected by workers.
    pub graph: serde_json::Value,
    /// Optional execution settings (device preference, etc.).
    pub settings: JobSettings,
    /// Timestamp when the job was first created and queued.
    pub created_at: DateTime<Utc>,
    /// Timestamp when a worker began executing the job. None while queued.
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when the job finished (completed or failed). None while running.
    pub completed_at: Option<DateTime<Utc>>,
    /// ID of the worker that executed this job. Set after execution begins.
    pub worker_id: Option<String>,
    /// Error message if the job failed. Set only for `Failed` jobs.
    pub error: Option<String>,
    /// Position in the queue. Set when `Queued`, cleared when picked up.
    pub queue_position: Option<u32>,
}
