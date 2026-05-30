//! Worker state types — worker identification, status, and current assignment.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// WorkerStatus — lifecycle state of a worker
// ---------------------------------------------------------------------------

/// The lifecycle status of a worker process.
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    ToSchema,
)]
pub enum WorkerStatus {
    /// The worker is starting up and loading the model.
    Initializing,
    /// The worker is ready and waiting for jobs.
    Idle,
    /// The worker is actively processing a job.
    Busy,
    /// The worker process has crashed or become unresponsive.
    Dead,
    /// A dead worker is being restarted by the supervisor.
    Respawning,
}

// ---------------------------------------------------------------------------
// WorkerInfo — information about a single worker
// ---------------------------------------------------------------------------

/// Metadata describing a single worker in the AnvilML system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct WorkerInfo {
    /// Unique worker identifier (format: "worker-{device_index}").
    pub worker_id: String,

    /// Zero-based device index this worker manages.
    pub device_index: u32,

    /// Human-readable name of the device (e.g. GPU name or "cpu").
    pub device_name: String,

    /// Current lifecycle status.
    pub status: WorkerStatus,

    /// The job currently being processed, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_job_id: Option<Uuid>,

    /// Currently used VRAM in mebibytes (0 for CPU workers).
    pub vram_used_mib: u32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // WorkerStatus — serialization round-trip
    // ------------------------------------------------------------------

    #[test]
    fn worker_status_serialization_round_trip() {
        for status in [
            WorkerStatus::Initializing,
            WorkerStatus::Idle,
            WorkerStatus::Busy,
            WorkerStatus::Dead,
            WorkerStatus::Respawning,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: WorkerStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back, "failed for {:?}", status);
        }
    }

    // ------------------------------------------------------------------
    // WorkerInfo — construction and round-trip
    // ------------------------------------------------------------------

    #[test]
    fn worker_info_construct_and_round_trip() {
        let job_id = Uuid::new_v4();
        let worker = WorkerInfo {
            worker_id: "worker-0".into(),
            device_index: 0,
            device_name: "NVIDIA GeForce RTX 4090".into(),
            status: WorkerStatus::Busy,
            current_job_id: Some(job_id),
            vram_used_mib: 8192,
        };
        let json = serde_json::to_string(&worker).unwrap();
        let back: WorkerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(worker, back);
    }
}
