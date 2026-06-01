//! WebSocket event types per ANVILML_DESIGN §4.5.
//!
//! Defines the `WsEvent` enum and nine associated variant structs
//! (`SystemStatsEvent`, `JobQueuedEvent`, `JobStartedEvent`, `JobProgressEvent`,
//! `JobImageReadyEvent`, `JobCompletedEvent`, `JobFailedEvent`,
//! `JobCancelledEvent`, `WorkerStatusChangedEvent`) plus the helper struct
//! `GpuStatSnapshot`, all serializing to `{ "event": "<type>", "timestamp":
//! "<iso8601>", ...fields }` as specified in ANVILML_DESIGN §4.5.
//!
//! These types form the contract for the WebSocket broadcaster in
//! `anvilml-server` and are consumed by the scheduler's event-dispatch logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// GPU VRAM snapshot for a single device at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpuStatSnapshot {
    /// Zero-based index of the GPU device.
    pub index: u32,
    /// VRAM currently used by this GPU in MiB.
    pub vram_used_mib: u32,
    /// Total VRAM available on this GPU in MiB.
    pub vram_total_mib: u32,
}

/// System statistics event — periodically broadcast to report hardware state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemStatsEvent {
    /// Event type name, always `"system.stats"`.
    pub event: String,
    /// When this snapshot was taken.
    pub timestamp: DateTime<Utc>,
    /// Per-GPU VRAM snapshots.
    #[serde(default)]
    pub gpus: Vec<GpuStatSnapshot>,
    /// Host RAM currently used in MiB.
    pub ram_used_mib: u64,
    /// Total host RAM in MiB.
    pub ram_total_mib: u64,
}

/// Job-queued event — emitted when a new job enters the queue.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobQueuedEvent {
    /// Event type name, always `"job.queued"`.
    pub event: String,
    /// When the job was queued.
    pub timestamp: DateTime<Utc>,
    /// UUID of the newly queued job.
    pub job_id: Uuid,
}

/// Job-started event — emitted when a worker begins executing a job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobStartedEvent {
    /// Event type name, always `"job.started"`.
    pub event: String,
    /// When execution began.
    pub timestamp: DateTime<Utc>,
    /// UUID of the job that started.
    pub job_id: Uuid,
}

/// Job-progress event — emitted periodically during job execution to report
/// DAG node completion progress.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobProgressEvent {
    /// Event type name, always `"job.progress"`.
    pub event: String,
    /// When this progress update was emitted.
    pub timestamp: DateTime<Utc>,
    /// UUID of the job being executed.
    pub job_id: Uuid,
    /// Index of the node that just completed (0-based).
    pub node_index: u32,
    /// Total number of nodes in the DAG.
    pub node_total: u32,
    /// Type / class name of the completed node.
    pub node_type: String,
    /// Per-step progress within a node. Reserved for future use; always `None`
    /// in the MVP.
    #[serde(default)]
    pub step: Option<u32>,
    /// Total steps within a node. Reserved for future use; always `None` in
    /// the MVP.
    #[serde(default)]
    pub step_total: Option<u32>,
}

/// Job-image-ready event — emitted when a worker produces an output image.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobImageReadyEvent {
    /// Event type name, always `"job.image_ready"`.
    pub event: String,
    /// When the image became available.
    pub timestamp: DateTime<Utc>,
    /// UUID of the job that produced this image.
    pub job_id: Uuid,
    /// SHA-256 hash used to fetch the artifact via `GET /v1/artifacts/:hash`.
    pub artifact_hash: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Random seed used to generate this image.
    pub seed: i64,
}

/// Job-completed event — emitted when a job finishes successfully.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobCompletedEvent {
    /// Event type name, always `"job.completed"`.
    pub event: String,
    /// When the job completed.
    pub timestamp: DateTime<Utc>,
    /// UUID of the completed job.
    pub job_id: Uuid,
}

/// Job-failed event — emitted when a job encounters an unrecoverable error.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobFailedEvent {
    /// Event type name, always `"job.failed"`.
    pub event: String,
    /// When the failure was detected.
    pub timestamp: DateTime<Utc>,
    /// UUID of the failed job.
    pub job_id: Uuid,
    /// Human-readable error message.
    pub error: String,
    /// Optional stack trace or additional diagnostic context.
    #[serde(default)]
    pub traceback: Option<String>,
}

/// Job-cancelled event — emitted when a user cancels a running job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobCancelledEvent {
    /// Event type name, always `"job.cancelled"`.
    pub event: String,
    /// When the cancellation was processed.
    pub timestamp: DateTime<Utc>,
    /// UUID of the cancelled job.
    pub job_id: Uuid,
}

/// Worker-status-changed event — emitted whenever a worker's lifecycle state
/// transitions (e.g. Idle → Busy, Busy → Dead).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkerStatusChangedEvent {
    /// Event type name, always `"worker.status"`.
    pub event: String,
    /// When the status change occurred.
    pub timestamp: DateTime<Utc>,
    /// The worker identifier (format: `"worker-{device_index}"`).
    pub worker_id: String,
    /// The new lifecycle status of the worker.
    pub status: crate::types::worker::WorkerStatus,
}

/// All WebSocket event types that the server may broadcast to connected clients.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum WsEvent {
    /// System statistics snapshot (§4.5).
    SystemStats(SystemStatsEvent),
    /// Job has been queued for execution.
    JobQueued(JobQueuedEvent),
    /// Job execution has started on a worker.
    JobStarted(JobStartedEvent),
    /// Progress update during job execution.
    JobProgress(JobProgressEvent),
    /// An output image is available for retrieval.
    JobImageReady(JobImageReadyEvent),
    /// Job completed successfully.
    JobCompleted(JobCompletedEvent),
    /// Job failed with an error.
    JobFailed(JobFailedEvent),
    /// Job was cancelled by the user.
    JobCancelled(JobCancelledEvent),
    /// A worker's lifecycle status changed.
    WorkerStatusChanged(WorkerStatusChangedEvent),
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `WsEvent` must have exactly 9 variants and all pairs must compare
    /// equal/unequal correctly.
    #[test]
    fn ws_event_enum_variants() {
        let variants: Vec<WsEvent> = vec![
            WsEvent::SystemStats(SystemStatsEvent {
                event: "system.stats".to_string(),
                timestamp: Utc::now(),
                gpus: vec![],
                ram_used_mib: 0,
                ram_total_mib: 0,
            }),
            WsEvent::JobQueued(JobQueuedEvent {
                event: "job.queued".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
            }),
            WsEvent::JobStarted(JobStartedEvent {
                event: "job.started".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
            }),
            WsEvent::JobProgress(JobProgressEvent {
                event: "job.progress".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
                node_index: 0,
                node_total: 5,
                node_type: "Load".to_string(),
                step: None,
                step_total: None,
            }),
            WsEvent::JobImageReady(JobImageReadyEvent {
                event: "job.image_ready".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
                artifact_hash: "abc123".to_string(),
                width: 512,
                height: 512,
                seed: 42,
            }),
            WsEvent::JobCompleted(JobCompletedEvent {
                event: "job.completed".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
            }),
            WsEvent::JobFailed(JobFailedEvent {
                event: "job.failed".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
                error: "test error".to_string(),
                traceback: None,
            }),
            WsEvent::JobCancelled(JobCancelledEvent {
                event: "job.cancelled".to_string(),
                timestamp: Utc::now(),
                job_id: Uuid::new_v4(),
            }),
            WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent {
                event: "worker.status".to_string(),
                timestamp: Utc::now(),
                worker_id: "worker-0".to_string(),
                status: crate::types::worker::WorkerStatus::Idle,
            }),
        ];

        assert_eq!(variants.len(), 9, "WsEvent must have exactly 9 variants");

        // All variants must be distinct (by discriminant).
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(
                    std::mem::discriminant(&variants[i]),
                    std::mem::discriminant(&variants[j]),
                    "variants {i} and {j} must differ"
                );
            }
        }
    }

    /// SystemStatsEvent JSON must contain `"event":"system.stats"`.
    #[test]
    fn system_stats_event_json() {
        let event = SystemStatsEvent {
            event: "system.stats".to_string(),
            timestamp: Utc::now(),
            gpus: vec![GpuStatSnapshot {
                index: 0,
                vram_used_mib: 45000,
                vram_total_mib: 81920,
            }],
            ram_used_mib: 32768,
            ram_total_mib: 65536,
        };

        let json = serde_json::to_string(&event).expect("serialize SystemStatsEvent");
        assert!(
            json.contains(r#""event":"system.stats""#),
            "JSON must contain event name system.stats: {json}"
        );
        assert!(
            json.contains("timestamp"),
            "JSON must contain timestamp field: {json}"
        );
    }

    /// SystemStatsEvent fields must round-trip through JSON serialization.
    #[test]
    fn system_stats_roundtrip() {
        let event = SystemStatsEvent {
            event: "system.stats".to_string(),
            timestamp: Utc::now(),
            gpus: vec![
                GpuStatSnapshot {
                    index: 0,
                    vram_used_mib: 45000,
                    vram_total_mib: 81920,
                },
                GpuStatSnapshot {
                    index: 1,
                    vram_used_mib: 40000,
                    vram_total_mib: 81920,
                },
            ],
            ram_used_mib: 32768,
            ram_total_mib: 65536,
        };

        let json = serde_json::to_string(&event).expect("serialize SystemStatsEvent");
        let restored: SystemStatsEvent =
            serde_json::from_str(&json).expect("deserialize SystemStatsEvent");

        assert_eq!(restored.event, "system.stats");
        assert_eq!(restored.gpus.len(), 2);
        assert_eq!(restored.gpus[0].index, 0);
        assert_eq!(restored.gpus[0].vram_used_mib, 45000);
        assert_eq!(restored.gpus[1].index, 1);
        assert_eq!(restored.ram_used_mib, 32768);
        assert_eq!(restored.ram_total_mib, 65536);
    }

    /// JobQueuedEvent must serialize and deserialize correctly.
    #[test]
    fn job_queued_roundtrip() {
        let event = JobQueuedEvent {
            event: "job.queued".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };

        let json = serde_json::to_string(&event).expect("serialize JobQueuedEvent");
        let restored: JobQueuedEvent =
            serde_json::from_str(&json).expect("deserialize JobQueuedEvent");

        assert_eq!(restored.event, "job.queued");
        assert_eq!(restored.job_id, event.job_id);
    }

    /// JobStartedEvent must serialize and deserialize correctly.
    #[test]
    fn job_started_roundtrip() {
        let event = JobStartedEvent {
            event: "job.started".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };

        let json = serde_json::to_string(&event).expect("serialize JobStartedEvent");
        let restored: JobStartedEvent =
            serde_json::from_str(&json).expect("deserialize JobStartedEvent");

        assert_eq!(restored.event, "job.started");
        assert_eq!(restored.job_id, event.job_id);
    }

    /// JobProgressEvent optional fields (step / step_total) must serialize as
    /// `null` when `None`.
    #[test]
    fn job_progress_optional_fields() {
        let event = JobProgressEvent {
            event: "job.progress".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::new_v4(),
            node_index: 3,
            node_total: 10,
            node_type: "Decode".to_string(),
            step: None,
            step_total: None,
        };

        let json = serde_json::to_string(&event).expect("serialize JobProgressEvent");
        assert!(
            json.contains(r#""step":null"#),
            "step must serialize as null when None: {json}"
        );
        assert!(
            json.contains(r#""step_total":null"#),
            "step_total must serialize as null when None: {json}"
        );
    }

    /// JobProgressEvent fields must round-trip through JSON serialization.
    #[test]
    fn job_progress_roundtrip() {
        let event = JobProgressEvent {
            event: "job.progress".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            node_index: 5,
            node_total: 12,
            node_type: "Generate".to_string(),
            step: Some(3),
            step_total: Some(20),
        };

        let json = serde_json::to_string(&event).expect("serialize JobProgressEvent");
        let restored: JobProgressEvent =
            serde_json::from_str(&json).expect("deserialize JobProgressEvent");

        assert_eq!(restored.event, "job.progress");
        assert_eq!(restored.job_id, event.job_id);
        assert_eq!(restored.node_index, 5);
        assert_eq!(restored.node_total, 12);
        assert_eq!(restored.node_type, "Generate");
        assert_eq!(restored.step, Some(3));
        assert_eq!(restored.step_total, Some(20));
    }

    /// JobImageReadyEvent must serialize and deserialize correctly.
    #[test]
    fn job_image_ready_roundtrip() {
        let event = JobImageReadyEvent {
            event: "job.image_ready".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            artifact_hash: "sha256:abcdef123456".to_string(),
            width: 1024,
            height: 768,
            seed: 98765,
        };

        let json = serde_json::to_string(&event).expect("serialize JobImageReadyEvent");
        let restored: JobImageReadyEvent =
            serde_json::from_str(&json).expect("deserialize JobImageReadyEvent");

        assert_eq!(restored.event, "job.image_ready");
        assert_eq!(restored.job_id, event.job_id);
        assert_eq!(restored.artifact_hash, "sha256:abcdef123456");
        assert_eq!(restored.width, 1024);
        assert_eq!(restored.height, 768);
        assert_eq!(restored.seed, 98765);
    }

    /// JobCompletedEvent must serialize and deserialize correctly.
    #[test]
    fn job_completed_roundtrip() {
        let event = JobCompletedEvent {
            event: "job.completed".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };

        let json = serde_json::to_string(&event).expect("serialize JobCompletedEvent");
        let restored: JobCompletedEvent =
            serde_json::from_str(&json).expect("deserialize JobCompletedEvent");

        assert_eq!(restored.event, "job.completed");
        assert_eq!(restored.job_id, event.job_id);
    }

    /// JobFailedEvent with traceback must round-trip correctly.
    #[test]
    fn job_failed_roundtrip() {
        let event = JobFailedEvent {
            event: "job.failed".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            error: "CUDA out of memory".to_string(),
            traceback: Some(
                "  File \"worker.py\", line 42, in run\n    raise RuntimeError()".to_string(),
            ),
        };

        let json = serde_json::to_string(&event).expect("serialize JobFailedEvent");
        let restored: JobFailedEvent =
            serde_json::from_str(&json).expect("deserialize JobFailedEvent");

        assert_eq!(restored.event, "job.failed");
        assert_eq!(restored.job_id, event.job_id);
        assert_eq!(restored.error, "CUDA out of memory");
        assert!(restored.traceback.is_some());
        assert!(restored.traceback.as_ref().unwrap().contains("worker.py"));
    }

    /// JobFailedEvent without traceback must also round-trip.
    #[test]
    fn job_failed_no_traceback() {
        let event = JobFailedEvent {
            event: "job.failed".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::new_v4(),
            error: "unknown".to_string(),
            traceback: None,
        };

        let json = serde_json::to_string(&event).expect("serialize JobFailedEvent");
        let restored: JobFailedEvent =
            serde_json::from_str(&json).expect("deserialize JobFailedEvent");

        assert_eq!(restored.error, "unknown");
        assert!(restored.traceback.is_none());
    }

    /// JobCancelledEvent must serialize and deserialize correctly.
    #[test]
    fn job_cancelled_roundtrip() {
        let event = JobCancelledEvent {
            event: "job.cancelled".to_string(),
            timestamp: Utc::now(),
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };

        let json = serde_json::to_string(&event).expect("serialize JobCancelledEvent");
        let restored: JobCancelledEvent =
            serde_json::from_str(&json).expect("deserialize JobCancelledEvent");

        assert_eq!(restored.event, "job.cancelled");
        assert_eq!(restored.job_id, event.job_id);
    }

    /// WorkerStatusChangedEvent must round-trip all WorkerStatus values.
    #[test]
    fn worker_status_changed_roundtrip() {
        for status in [
            crate::types::worker::WorkerStatus::Initializing,
            crate::types::worker::WorkerStatus::Idle,
            crate::types::worker::WorkerStatus::Busy,
            crate::types::worker::WorkerStatus::Dead,
            crate::types::worker::WorkerStatus::Respawning,
        ] {
            let event = WorkerStatusChangedEvent {
                event: "worker.status".to_string(),
                timestamp: Utc::now(),
                worker_id: format!("worker-{}", status as u8),
                status,
            };

            let json = serde_json::to_string(&event).expect("serialize WorkerStatusChangedEvent");
            let restored: WorkerStatusChangedEvent =
                serde_json::from_str(&json).expect("deserialize WorkerStatusChangedEvent");

            assert_eq!(restored.event, "worker.status");
            assert_eq!(restored.worker_id, event.worker_id);
            assert_eq!(restored.status, status);
        }
    }
}
