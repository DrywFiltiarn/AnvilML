//! Worker state and environment reporting types for the AnvilML system.
//!
//! Defines `WorkerStatus` (the current lifecycle state of a worker),
//! `ProvisioningState` (the state of the Python worker provisioning process),
//! `WorkerInfo` (a snapshot of a worker's state including device and job info),
//! and `EnvReport` (the environment report sent by the Python worker at startup
//! as part of the `Ready` event). These types are used by the Rust supervisor
//! to track worker health and by the scheduler to route jobs.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::types::node::NodeTypeDescriptor;

/// The current lifecycle state of a worker process.
///
/// Each variant corresponds to a distinct phase in the worker's lifecycle.
/// The Rust supervisor transitions workers between these states: on spawn
/// → `Initializing`, on `Ready` event → `Idle`, on job dispatch → `Busy`,
/// on unexpected exit → `Dead`, and during respawn → `Respawning`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    /// The worker process has been spawned but has not yet reported `Ready`.
    Initializing,
    /// The worker is idle and ready to accept jobs.
    Idle,
    /// The worker is currently executing a job.
    Busy,
    /// The worker process has exited unexpectedly and is not being respawned.
    Dead,
    /// The worker is being respawned after a previous death.
    Respawning,
}

/// The current state of the Python worker provisioning process.
///
/// Provisioning occurs once at server startup and involves creating the
/// Python virtual environment, installing dependencies, and verifying the
/// torch import. The state transitions are: `NotStarted` → `Provisioning`
/// → `Ready` (success) or `Failed` (error).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    /// Provisioning has not yet been attempted.
    NotStarted,
    /// Provisioning is currently in progress.
    Provisioning,
    /// Provisioning completed successfully; the Python worker environment is ready.
    Ready,
    /// Provisioning failed; the error reason is in `EnvReport.reason`.
    Failed,
}

/// A snapshot of a single worker's state and identity.
///
/// Maintained by the Rust supervisor. Contains the worker's stable identity
/// (`id`), device information (`device_index`, `device_name`), current
/// lifecycle state (`status`), and optional job tracking fields
/// (`current_job_id`, `vram_used_mib`). The `current_job_id` is `None`
/// when the worker is idle and set to the active job's UUID when busy.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkerInfo {
    /// Stable worker identity string (e.g. `"worker-0"`).
    pub id: String,
    /// Zero-based GPU device index this worker is bound to.
    pub device_index: u32,
    /// Human-readable GPU device name (e.g. `"NVIDIA A100-SXM4-40GB"`).
    pub device_name: String,
    /// Current lifecycle state of this worker.
    pub status: WorkerStatus,
    /// UUID of the job currently being executed, or `None` if idle.
    pub current_job_id: Option<Uuid>,
    /// VRAM used by this worker in mebibytes, or `None` if unknown.
    pub vram_used_mib: Option<u32>,
}

/// The environment report sent by the Python worker during the `Ready` event.
///
/// Produced by the Python worker after `_probe_hardware()` and `_import_nodes()`
/// complete successfully. Contains the Python runtime information (`python_path`,
/// `python_version`, `torch_version`), the provisioning state, a preflight
/// check result, and the list of all `NodeTypeDescriptor` values registered
/// in the worker's `NODE_REGISTRY`. The Rust supervisor uses this to validate
/// the worker's environment and populate the scheduler's node registry.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvReport {
    /// Path to the Python interpreter used by this worker.
    pub python_path: Option<String>,
    /// Python version string (e.g. `"3.12.3"`).
    pub python_version: Option<String>,
    /// PyTorch version string (e.g. `"2.4.0+cu121"`).
    pub torch_version: Option<String>,
    /// The provisioning state at the time of this report.
    pub provisioning: ProvisioningState,
    /// Whether the preflight checks passed. `false` indicates the worker
    /// environment has issues and should not accept jobs.
    pub preflight_ok: bool,
    /// Human-readable reason for failure if `preflight_ok` is `false`.
    /// `None` when preflight passed.
    pub reason: Option<String>,
    /// All node types registered in the worker's `NODE_REGISTRY`.
    /// Empty vector when the worker has not yet loaded nodes.
    pub node_types: Vec<NodeTypeDescriptor>,
}
