//! Worker-process status and provisioning types.
//!
//! These types describe the lifecycle state of a Python worker process, the metadata
//! the scheduler uses for dispatch decisions, and the environment report the scheduler
//! collects at startup. They are consumed by:
//!
//! - The scheduler's dispatch logic (worker selection, job assignment).
//! - The `/v1/workers` HTTP handler (list, inspect workers).
//! - The `/v1/system` HTTP handler (report host environment).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::hardware::DeviceType;
use super::node::NodeTypeDescriptor;

/// The lifecycle state of a Python worker process.
///
/// Workers transition through these states as the scheduler spawns, assigns jobs,
/// and eventually shuts them down. The scheduler uses this enum to decide which
/// workers are eligible for job assignment.
///
/// State machine (per `ANVILML_DESIGN.md §5.7`):
/// ```text
/// Initializing → Idle ↔ Busy → Dying → Dead → Respawning → Initializing
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    /// The worker process has been spawned and is initializing (loading models,
    /// connecting IPC). The scheduler waits up to 60 seconds for a `Ready` event;
    /// if none arrives, the worker is declared `Dead`.
    Initializing,
    /// The worker is ready and waiting for a job assignment.
    Idle,
    /// The worker is currently executing a job.
    Busy,
    /// The scheduler has sent a shutdown signal and is waiting for the worker
    /// to exit cleanly.
    Dying,
    /// The worker process has terminated and is no longer tracked.
    Dead,
    /// The worker is being respawned after a `Dead` transition. The next state
    /// is `Initializing` — the same as a fresh spawn.
    Respawning,
}

/// Metadata the scheduler uses for worker selection and job assignment.
///
/// Constructed from the hardware snapshot (for `device_type`) and the worker's
/// registration message (for `worker_id`, `status`, `device_index`, `pid`).
/// The `current_job_id` is updated when the scheduler assigns a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkerInfo {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    pub worker_id: String,
    /// Current lifecycle state of the worker process.
    pub status: WorkerStatus,
    /// Zero-based device index as reported by the OS/driver.
    pub device_index: u32,
    /// The compute backend of the worker's device.
    pub device_type: DeviceType,
    /// OS process ID of the worker subprocess, if known.
    pub pid: Option<u32>,
    /// The job currently assigned to this worker, if any.
    pub current_job_id: Option<Uuid>,
}

/// Python runtime environment report collected at worker startup preflight.
///
/// The scheduler uses this to verify the worker's Python and PyTorch
/// environment, provisioning status, and the node types it supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EnvReport {
    /// Path to the Python interpreter used by the worker (e.g. `./worker/.venv/bin/python3`).
    pub python_path: Option<String>,
    /// The Python interpreter version string (e.g. `"3.12.3"`), or `None` if unavailable.
    pub python_version: Option<String>,
    /// The PyTorch version string if `import torch` succeeded, or `None` if
    /// the import failed or torch is not installed.
    pub torch_version: Option<String>,
    /// The provisioning status of the worker's environment.
    pub provisioning: ProvisioningState,
    /// Whether all preflight checks passed (interpreter exists, Python version OK, torch importable).
    pub preflight_ok: bool,
    /// An optional human-readable reason when `preflight_ok` is `false`.
    pub reason: Option<String>,
    /// The node types registered by this worker, reported at startup.
    pub node_types: Vec<NodeTypeDescriptor>,
}

/// The provisioning status of a worker's environment.
///
/// Used by the provisioning subsystem to track whether a worker's Python
/// dependencies (torch, node packages) have been installed and are ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    /// Dependencies have not yet been installed for this worker.
    NotStarted,
    /// The provisioning process is currently running.
    Provisioning,
    /// All required dependencies are installed and verified.
    Ready,
    /// The provisioning process failed (e.g. pip install error).
    Failed,
}
