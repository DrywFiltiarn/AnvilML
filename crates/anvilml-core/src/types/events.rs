//! WebSocket event types — the wire-format event enum and all nine event structs.
//!
//! All events serialize as `{ "event": "<type>", "timestamp": "<iso8601>", ...fields }`.
//! The `WsEvent` enum uses serde internally-tagged serialization to produce
//! the correct wire format. The `#[serde(tag = "event", rename_all = "snake_case")]`
//! attribute on the enum automatically produces the event discriminator from
//! the variant name (e.g. `SystemStats` → `"system_stats"`).
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::types::WorkerStatus;

// ---------------------------------------------------------------------------
// GpuStatSnapshot — runtime GPU statistics
// ---------------------------------------------------------------------------

/// A snapshot of GPU memory usage at a point in time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct GpuStatSnapshot {
    /// Zero-based device index.
    pub index: u32,

    /// Currently used VRAM in mebibytes.
    pub vram_used_mib: u32,

    /// Total VRAM in mebibytes.
    pub vram_total_mib: u32,
}

// ---------------------------------------------------------------------------
// Event structs — one per WsEvent variant
//
// NOTE: These structs do NOT have an `event` field. The `WsEvent` enum's
// `#[serde(tag = "event", rename_all = "snake_case")]` attribute handles
// the discriminator automatically from the variant name.
// ---------------------------------------------------------------------------

/// Event emitted when system stats are collected (e.g. periodic health check).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct SystemStatsEvent {
    /// Timestamp when the stats were collected.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// GPU memory snapshots.
    pub gpus: Vec<GpuStatSnapshot>,
    /// Currently used RAM in mebibytes.
    pub ram_used_mib: u64,
    /// Total system RAM in mebibytes.
    pub ram_total_mib: u64,
}

/// Event emitted when a new job is added to the queue.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobQueuedEvent {
    /// Timestamp when the job was queued.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The unique identifier of the queued job.
    pub job_id: Uuid,
    /// The model used by this job.
    pub model_id: Uuid,
}

/// Event emitted when a worker begins processing a job.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobStartedEvent {
    /// Timestamp when the job started.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The unique identifier of the started job.
    pub job_id: Uuid,
    /// The worker that began processing this job.
    pub worker_id: String,
}

/// Event emitted during job processing to report progress.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobProgressEvent {
    /// Timestamp of this progress update.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The job being processed.
    pub job_id: Uuid,
    /// Current node index (0-based).
    pub node_index: u32,
    /// Total number of nodes.
    pub node_total: u32,
    /// Human-readable node type label.
    pub node_type: String,
    /// Per-step progress — reserved for future use, always `None` in MVP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    /// Total steps — reserved for future use, always `None` in MVP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_total: Option<u32>,
}

/// Event emitted when a generated image is ready for download.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobImageReadyEvent {
    /// Timestamp when the image was generated.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The job that produced this image.
    pub job_id: Uuid,
    /// Content-addressable hash for fetching the image via REST.
    pub artifact_hash: String,
    /// Generated image width in pixels.
    pub width: u32,
    /// Generated image height in pixels.
    pub height: u32,
    /// Random seed used for generation.
    pub seed: i64,
}

/// Event emitted when a job completes successfully.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobCompletedEvent {
    /// Timestamp when the job completed.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The unique identifier of the completed job.
    pub job_id: Uuid,
}

/// Event emitted when a job fails with an error.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobFailedEvent {
    /// Timestamp when the failure was detected.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The unique identifier of the failed job.
    pub job_id: Uuid,
    /// Human-readable error message.
    pub error: String,
    /// Optional stack trace or detailed traceback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traceback: Option<String>,
}

/// Event emitted when a job is cancelled.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobCancelledEvent {
    /// Timestamp when the cancellation occurred.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The unique identifier of the cancelled job.
    pub job_id: Uuid,
}

/// Event emitted when a worker's status changes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct WorkerStatusChangedEvent {
    /// Timestamp when the status changed.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    /// The worker whose status changed.
    pub worker_id: String,
    /// The new status of the worker.
    pub status: WorkerStatus,
}

// ---------------------------------------------------------------------------
// WsEvent — the top-level WebSocket event enum
// ---------------------------------------------------------------------------

/// A single WebSocket event emitted by the AnvilML server.
///
/// Uses serde internally-tagged serialization to produce wire format:
/// `{ "event": "<type>", "timestamp": "...", ...fields }`.
/// The `event` discriminator is derived from the variant name via
/// `#[serde(tag = "event", rename_all = "snake_case")]`.
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WsEvent {
    /// System statistics snapshot.
    SystemStats(SystemStatsEvent),
    /// A new job has been queued.
    JobQueued(JobQueuedEvent),
    /// A worker has started processing a job.
    JobStarted(JobStartedEvent),
    /// Progress update during job execution.
    JobProgress(JobProgressEvent),
    /// A generated image is ready for download.
    JobImageReady(JobImageReadyEvent),
    /// A job completed successfully.
    JobCompleted(JobCompletedEvent),
    /// A job failed with an error.
    JobFailed(JobFailedEvent),
    /// A job was cancelled.
    JobCancelled(JobCancelledEvent),
    /// A worker's status has changed.
    WorkerStatusChanged(WorkerStatusChangedEvent),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // GpuStatSnapshot — serialization round-trip
    // ------------------------------------------------------------------

    #[test]
    fn gpu_stat_snapshot_round_trip() {
        let snap = GpuStatSnapshot {
            index: 0,
            vram_used_mib: 8192,
            vram_total_mib: 24576,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: GpuStatSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }

    // ------------------------------------------------------------------
    // SystemStatsEvent — serialization with correct discriminator
    // ------------------------------------------------------------------

    #[test]
    fn system_stats_event_serialization() {
        let ev = WsEvent::SystemStats(SystemStatsEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            gpus: vec![GpuStatSnapshot {
                index: 0,
                vram_used_mib: 8192,
                vram_total_mib: 24576,
            }],
            ram_used_mib: 16384,
            ram_total_mib: 32768,
        });
        let json = serde_json::to_string(&ev).unwrap();
        // Assert the event discriminator is present as a top-level key
        assert!(json.contains(r#""event":"system_stats""#) || json.contains(r#""event": "system_stats""#));
        assert!(json.contains("timestamp"));
        assert!(json.contains("gpus"));
    }

    // ------------------------------------------------------------------
    // JobQueuedEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_queued_event_round_trip() {
        let job_id = Uuid::new_v4();
        let model_id = Uuid::new_v4();
        let ev = WsEvent::JobQueued(JobQueuedEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
            model_id,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobQueued(b) => {
                assert_eq!(b.job_id, job_id);
                assert_eq!(b.model_id, model_id);
            }
            other => panic!("expected JobQueued, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobStartedEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_started_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobStarted(JobStartedEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
            worker_id: "worker-0".into(),
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobStarted(b) => assert_eq!(b.job_id, job_id),
            other => panic!("expected JobStarted, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobProgressEvent — round-trip with None step/step_total
    // ------------------------------------------------------------------

    #[test]
    fn job_progress_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobProgress(JobProgressEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
            node_index: 3,
            node_total: 10,
            node_type: "KSampler".into(),
            step: None,
            step_total: None,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobProgress(b) => {
                assert_eq!(b.job_id, job_id);
                assert_eq!(b.node_index, 3);
                assert_eq!(b.step, None);
                assert_eq!(b.step_total, None);
            }
            other => panic!("expected JobProgress, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobImageReadyEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_image_ready_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobImageReady(JobImageReadyEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
            artifact_hash: "abc123def456".into(),
            width: 1024,
            height: 1024,
            seed: 42,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobImageReady(b) => {
                assert_eq!(b.job_id, job_id);
                assert_eq!(b.width, 1024);
                assert_eq!(b.seed, 42);
            }
            other => panic!("expected JobImageReady, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobCompletedEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_completed_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobCompleted(JobCompletedEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobCompleted(b) => assert_eq!(b.job_id, job_id),
            other => panic!("expected JobCompleted, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobFailedEvent — round-trip with error and optional traceback
    // ------------------------------------------------------------------

    #[test]
    fn job_failed_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobFailed(JobFailedEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
            error: "CUDA out of memory".into(),
            traceback: Some("Traceback (most recent call last):\n  ...".into()),
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobFailed(b) => {
                assert_eq!(b.job_id, job_id);
                assert_eq!(b.error, "CUDA out of memory");
                assert!(b.traceback.is_some());
            }
            other => panic!("expected JobFailed, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // JobCancelledEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_cancelled_event_round_trip() {
        let job_id = Uuid::new_v4();
        let ev = WsEvent::JobCancelled(JobCancelledEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            job_id,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::JobCancelled(b) => assert_eq!(b.job_id, job_id),
            other => panic!("expected JobCancelled, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // WorkerStatusChangedEvent — round-trip
    // ------------------------------------------------------------------

    #[test]
    fn worker_status_changed_event_round_trip() {
        let ev = WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent {
            timestamp: DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            worker_id: "worker-0".into(),
            status: WorkerStatus::Busy,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let back: WsEvent = serde_json::from_str(&json).unwrap();
        match back {
            WsEvent::WorkerStatusChanged(b) => {
                assert_eq!(b.worker_id, "worker-0");
                assert_eq!(b.status, WorkerStatus::Busy);
            }
            other => panic!("expected WorkerStatusChanged, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // WsEvent — all variants serialize with correct discriminator
    // ------------------------------------------------------------------

    #[test]
    fn ws_event_all_variants_serialize() {
        let now = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let events: Vec<WsEvent> = vec![
            WsEvent::SystemStats(SystemStatsEvent {
                timestamp: now,
                gpus: vec![],
                ram_used_mib: 0,
                ram_total_mib: 0,
            }),
            WsEvent::JobQueued(JobQueuedEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
                model_id: Uuid::new_v4(),
            }),
            WsEvent::JobStarted(JobStartedEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
                worker_id: "w".into(),
            }),
            WsEvent::JobProgress(JobProgressEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
                node_index: 0,
                node_total: 1,
                node_type: "t".into(),
                step: None,
                step_total: None,
            }),
            WsEvent::JobImageReady(JobImageReadyEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
                artifact_hash: "h".into(),
                width: 1,
                height: 1,
                seed: 0,
            }),
            WsEvent::JobCompleted(JobCompletedEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
            }),
            WsEvent::JobFailed(JobFailedEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
                error: "err".into(),
                traceback: None,
            }),
            WsEvent::JobCancelled(JobCancelledEvent {
                timestamp: now,
                job_id: Uuid::new_v4(),
            }),
            WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent {
                timestamp: now,
                worker_id: "w".into(),
                status: WorkerStatus::Idle,
            }),
        ];

        for ev in &events {
            let json = serde_json::to_string(ev).unwrap();
            // Each must have an event discriminator
            assert!(
                json.contains("\"event\":"),
                "missing event discriminator in: {}",
                json
            );
            // Each must have a timestamp
            assert!(
                json.contains("\"timestamp\":"),
                "missing timestamp in: {}",
                json
            );
            // Round-trip must succeed
            let back: WsEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(
                std::mem::discriminant(ev),
                std::mem::discriminant(&back),
                "variant mismatch after round-trip for {:?}",
                ev
            );
        }
    }
}
