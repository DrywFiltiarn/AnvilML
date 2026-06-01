//! Worker domain types per ANVILML_DESIGN §4.4 and §6.1.
//!
//! Defines `WorkerInfo`, `WorkerStatus`, and `EnvReport` — all serializable,
//! clonable, debuggable, and schema-annotated for OpenAPI generation.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Lifecycle status of a worker process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum WorkerStatus {
    /// Worker process is starting up and initializing.
    Initializing,
    /// Worker is idle and ready to accept jobs.
    Idle,
    /// Worker is actively executing a job.
    Busy,
    /// Worker process has crashed or become unresponsive.
    Dead,
    /// Worker is being respawned after a crash (transient state).
    Respawning,
}

/// Information about a managed worker process.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkerInfo {
    /// Unique worker identifier (format: "worker-{device_index}").
    pub worker_id: String,
    /// Device index this worker manages.
    pub device_index: u32,
    /// Name of the GPU device (or "CPU" for CPU workers).
    pub device_name: String,
    /// Current lifecycle status.
    pub status: WorkerStatus,
    /// The job currently being executed. `None` when idle/dead.
    #[serde(default)]
    pub current_job_id: Option<Uuid>,
    /// VRAM consumed by the worker in MiB.
    #[serde(default)]
    pub vram_used_mib: u32,
}

/// Python environment health report from the preflight check.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct EnvReport {
    /// Path to the Python interpreter.
    pub python_path: String,
    /// Python version string (e.g. "3.12.4").
    #[serde(default)]
    pub python_version: String,
    /// PyTorch version string (e.g. "2.4.0+cu121").
    #[serde(default)]
    pub torch_version: String,
    /// Whether the preflight checks all passed.
    pub preflight_ok: bool,
    /// Reason if preflight failed; empty string if it succeeded.
    #[serde(default)]
    pub reason: String,
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `WorkerStatus` must have exactly 5 variants and all pairs must compare
    /// equal/unequal correctly.
    #[test]
    fn worker_status_variants() {
        let statuses: Vec<WorkerStatus> = vec![
            WorkerStatus::Initializing,
            WorkerStatus::Idle,
            WorkerStatus::Busy,
            WorkerStatus::Dead,
            WorkerStatus::Respawning,
        ];

        assert_eq!(statuses.len(), 5, "must have exactly 5 variants");

        // All variants must be distinct.
        for i in 0..statuses.len() {
            for j in (i + 1)..statuses.len() {
                assert_ne!(statuses[i], statuses[j], "variants {i} and {j} must differ");
            }
        }

        // Self-equality.
        assert_eq!(WorkerStatus::Initializing, WorkerStatus::Initializing);
        assert_eq!(WorkerStatus::Idle, WorkerStatus::Idle);
        assert_eq!(WorkerStatus::Busy, WorkerStatus::Busy);
        assert_eq!(WorkerStatus::Dead, WorkerStatus::Dead);
        assert_eq!(WorkerStatus::Respawning, WorkerStatus::Respawning);

        // Cross-inequality.
        assert_ne!(WorkerStatus::Initializing, WorkerStatus::Idle);
        assert_ne!(WorkerStatus::Busy, WorkerStatus::Dead);
    }

    /// `WorkerStatus` serializes to expected JSON strings.
    #[test]
    fn worker_status_json_strings() {
        let initializing = serde_json::to_string(&WorkerStatus::Initializing).unwrap();
        assert_eq!(initializing, "\"Initializing\"");

        let idle = serde_json::to_string(&WorkerStatus::Idle).unwrap();
        assert_eq!(idle, "\"Idle\"");

        let busy = serde_json::to_string(&WorkerStatus::Busy).unwrap();
        assert_eq!(busy, "\"Busy\"");

        let dead = serde_json::to_string(&WorkerStatus::Dead).unwrap();
        assert_eq!(dead, "\"Dead\"");

        let respawning = serde_json::to_string(&WorkerStatus::Respawning).unwrap();
        assert_eq!(respawning, "\"Respawning\"");
    }

    /// `WorkerInfo` fields must round-trip through JSON serialization.
    #[test]
    fn worker_info_roundtrip() {
        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let info = WorkerInfo {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            device_name: "NVIDIA A100-SXM4-80GB".to_string(),
            status: WorkerStatus::Busy,
            current_job_id: Some(job_id),
            vram_used_mib: 45000,
        };

        let json = serde_json::to_string(&info).expect("serialize WorkerInfo");
        let restored: WorkerInfo = serde_json::from_str(&json).expect("deserialize WorkerInfo");

        assert_eq!(restored.worker_id, info.worker_id);
        assert_eq!(restored.device_index, info.device_index);
        assert_eq!(restored.device_name, info.device_name);
        assert_eq!(restored.status, info.status);
        assert_eq!(restored.current_job_id, info.current_job_id);
        assert_eq!(restored.vram_used_mib, info.vram_used_mib);
    }

    /// `WorkerInfo` with no current job must round-trip correctly.
    #[test]
    fn worker_info_idle() {
        let info = WorkerInfo {
            worker_id: "worker-1".to_string(),
            device_index: 1,
            device_name: "NVIDIA A100-SXM4-80GB".to_string(),
            status: WorkerStatus::Idle,
            current_job_id: None,
            vram_used_mib: 1200,
        };

        let json = serde_json::to_string(&info).expect("serialize idle WorkerInfo");
        let restored: WorkerInfo =
            serde_json::from_str(&json).expect("deserialize idle WorkerInfo");

        assert_eq!(restored.status, WorkerStatus::Idle);
        assert!(restored.current_job_id.is_none());
    }

    /// `EnvReport` fields must round-trip through JSON serialization.
    #[test]
    fn env_report_roundtrip() {
        let report = EnvReport {
            python_path: "/home/user/venv/bin/python3".to_string(),
            python_version: "3.12.4".to_string(),
            torch_version: "2.4.0+cu121".to_string(),
            preflight_ok: true,
            reason: String::new(),
        };

        let json = serde_json::to_string(&report).expect("serialize EnvReport");
        let restored: EnvReport = serde_json::from_str(&json).expect("deserialize EnvReport");

        assert_eq!(restored.python_path, report.python_path);
        assert_eq!(restored.python_version, report.python_version);
        assert_eq!(restored.torch_version, report.torch_version);
        assert_eq!(restored.preflight_ok, true);
        assert!(restored.reason.is_empty());
    }

    /// `EnvReport` with failure reason must round-trip correctly.
    #[test]
    fn env_report_failure() {
        let report = EnvReport {
            python_path: "/opt/venv/bin/python3".to_string(),
            python_version: String::new(),
            torch_version: String::new(),
            preflight_ok: false,
            reason: "python_missing".to_string(),
        };

        let json = serde_json::to_string(&report).expect("serialize failed EnvReport");
        let restored: EnvReport =
            serde_json::from_str(&json).expect("deserialize failed EnvReport");

        assert_eq!(restored.preflight_ok, false);
        assert_eq!(restored.reason, "python_missing");
    }

    /// `EnvReport` defaults must produce an empty, failing report.
    #[test]
    fn env_report_defaults() {
        let report = EnvReport::default();
        assert!(report.python_path.is_empty());
        assert!(report.python_version.is_empty());
        assert!(report.torch_version.is_empty());
        assert!(!report.preflight_ok);
        assert!(report.reason.is_empty());
    }

    /// `EnvReport` minimal JSON (only required fields) must parse correctly.
    #[test]
    fn env_report_minimal_parse() {
        let minimal = serde_json::json!({
            "python_path": "/usr/bin/python3",
            "preflight_ok": true
        });

        let report: EnvReport = serde_json::from_value(minimal).expect("minimal EnvReport parses");

        assert_eq!(report.python_path, "/usr/bin/python3");
        assert_eq!(report.python_version, "");
        assert_eq!(report.torch_version, "");
        assert!(report.preflight_ok);
        assert_eq!(report.reason, "");
    }

    /// `WorkerInfo` optional fields must default correctly when absent.
    #[test]
    fn worker_info_optional_defaults() {
        let minimal = serde_json::json!({
            "worker_id": "worker-0",
            "device_index": 0,
            "device_name": "CPU",
            "status": "Initializing"
        });

        let info: WorkerInfo = serde_json::from_value(minimal).expect("minimal WorkerInfo parses");

        assert!(
            info.current_job_id.is_none(),
            "current_job_id must be None when absent"
        );
        assert_eq!(info.vram_used_mib, 0, "vram_used_mib must default to 0");
    }
}
