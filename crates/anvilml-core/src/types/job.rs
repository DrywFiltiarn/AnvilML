//! Job domain types per ANVILML_DESIGN §4.1.
//!
//! Defines `JobStatus`, `JobSettings`, `Job`, `SubmitJobRequest`, and
//! `SubmitJobResponse` — all serializable, clonable, debuggable, and
//! schema-annotated for OpenAPI generation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

// ── JobStatus ─────────────────────────────────────────────────────────────────

/// Lifecycle status of a generation job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum JobStatus {
    /// Job is waiting in the queue for a worker.
    Queued,
    /// Job has been dispatched to a worker and is executing.
    Running,
    /// Job completed successfully; artifacts are available.
    Completed,
    /// Job failed during execution; see `Job::error` for details.
    Failed,
    /// Job was cancelled by the user.
    Cancelled,
}

// ── JobSettings ───────────────────────────────────────────────────────────────

/// Generation parameters supplied by the frontend when creating a job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobSettings {
    /// Seed for the random number generator. `-1` means "random", resolved
    /// by the worker at execution time.
    #[serde(default = "default_seed")]
    pub seed: i64,
    /// Number of diffusion steps.
    #[serde(default = "default_steps")]
    pub steps: u32,
    /// Classifier-free guidance scale.
    #[serde(default = "default_guidance_scale")]
    pub guidance_scale: f32,
    /// Target image width in pixels.
    #[serde(default = "default_width")]
    pub width: u32,
    /// Target image height in pixels.
    #[serde(default = "default_height")]
    pub height: u32,
    /// User-requested device index. `None` means auto-select.
    #[serde(default)]
    pub device_preference: Option<u32>,
}

fn default_seed() -> i64 {
    -1
}

fn default_steps() -> u32 {
    20
}

fn default_guidance_scale() -> f32 {
    7.5
}

fn default_width() -> u32 {
    1024
}

fn default_height() -> u32 {
    1024
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            seed: default_seed(),
            steps: default_steps(),
            guidance_scale: default_guidance_scale(),
            width: default_width(),
            height: default_height(),
            device_preference: None,
        }
    }
}

// ── Job ───────────────────────────────────────────────────────────────────────

/// A generation job tracked by the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    /// Unique identifier for this job.
    pub id: Uuid,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// The validated DAG graph (as raw JSON).
    #[schema(value_type = Object)]
    pub graph: Value,
    /// Generation settings supplied at submission time.
    pub settings: JobSettings,
    /// Assigned GPU device index. `None` until dispatched.
    #[serde(default)]
    pub device_index: Option<u32>,
    /// When the job was created (always set).
    pub created_at: DateTime<Utc>,
    /// When execution began. `None` until dispatched.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    /// When the job reached a terminal state. `None` while in-flight.
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    /// The worker that processed this job. `None` until dispatched.
    #[serde(default)]
    pub worker_id: Option<String>,
    /// Number of artifacts produced so far.
    #[serde(default)]
    pub artifact_count: u32,
    /// Error message if the job failed. `None` otherwise.
    #[serde(default)]
    pub error: Option<String>,
}

// ── SubmitJobRequest / SubmitJobResponse ──────────────────────────────────────

/// Request body for the `POST /v1/jobs` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitJobRequest {
    /// The validated DAG graph (as raw JSON).
    #[schema(value_type = Object)]
    pub graph: Value,
    /// Generation settings.
    pub settings: JobSettings,
}

/// Response body for the `POST /v1/jobs` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SubmitJobResponse {
    /// The UUID of the newly created job.
    pub job_id: Uuid,
    /// Position in the queue (1-based).
    #[serde(default)]
    pub queue_position: u32,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `JobStatus` must have exactly 5 variants and all pairs must compare
    /// equal/unequal correctly.
    #[test]
    fn job_status_variants() {
        let statuses: Vec<JobStatus> = vec![
            JobStatus::Queued,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ];

        assert_eq!(statuses.len(), 5, "must have exactly 5 variants");

        // All variants must be distinct.
        for i in 0..statuses.len() {
            for j in (i + 1)..statuses.len() {
                assert_ne!(statuses[i], statuses[j], "variants {i} and {j} must differ");
            }
        }

        // Self-equality.
        assert_eq!(JobStatus::Queued, JobStatus::Queued);
        assert_eq!(JobStatus::Running, JobStatus::Running);
        assert_eq!(JobStatus::Completed, JobStatus::Completed);
        assert_eq!(JobStatus::Failed, JobStatus::Failed);
        assert_eq!(JobStatus::Cancelled, JobStatus::Cancelled);

        // Cross-inequality.
        assert_ne!(JobStatus::Queued, JobStatus::Running);
        assert_ne!(JobStatus::Completed, JobStatus::Failed);
    }

    /// `JobSettings` fields must round-trip through JSON serialization.
    #[test]
    fn job_settings_roundtrip() {
        let settings = JobSettings {
            seed: 42,
            steps: 50,
            guidance_scale: 12.0,
            width: 768,
            height: 512,
            device_preference: Some(0),
        };

        let json = serde_json::to_string(&settings).expect("serialize JobSettings");
        let restored: JobSettings = serde_json::from_str(&json).expect("deserialize JobSettings");

        assert_eq!(restored.seed, 42);
        assert_eq!(restored.steps, 50);
        assert_eq!(restored.guidance_scale, 12.0);
        assert_eq!(restored.width, 768);
        assert_eq!(restored.height, 512);
        assert_eq!(restored.device_preference, Some(0));
    }

    /// `JobSettings` defaults must apply when fields are omitted.
    #[test]
    fn job_settings_defaults() {
        let empty = "{}";
        let settings: JobSettings = serde_json::from_str(empty).expect("empty object parses");

        assert_eq!(settings.seed, -1);
        assert_eq!(settings.steps, 20);
        assert_eq!(settings.guidance_scale, 7.5);
        assert_eq!(settings.width, 1024);
        assert_eq!(settings.height, 1024);
        assert!(settings.device_preference.is_none());
    }

    /// Full `Job` struct must round-trip with all fields populated.
    #[test]
    fn job_roundtrip() {
        let now = Utc::now();
        let job = Job {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            status: JobStatus::Running,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings {
                seed: 12345,
                steps: 30,
                guidance_scale: 8.0,
                width: 512,
                height: 768,
                device_preference: Some(1),
            },
            device_index: Some(0),
            created_at: now,
            started_at: Some(now),
            completed_at: None,
            worker_id: Some("worker-0".to_string()),
            artifact_count: 0,
            error: None,
        };

        let json = serde_json::to_string(&job).expect("serialize Job");
        let restored: Job = serde_json::from_str(&json).expect("deserialize Job");

        assert_eq!(restored.id, job.id);
        assert_eq!(restored.status, job.status);
        assert_eq!(restored.graph, job.graph);
        assert_eq!(restored.settings.seed, job.settings.seed);
        assert_eq!(restored.settings.steps, job.settings.steps);
        assert_eq!(
            restored.settings.guidance_scale,
            job.settings.guidance_scale
        );
        assert_eq!(restored.settings.width, job.settings.width);
        assert_eq!(restored.settings.height, job.settings.height);
        assert_eq!(
            restored.settings.device_preference,
            job.settings.device_preference
        );
        assert_eq!(restored.device_index, job.device_index);
        assert_eq!(restored.created_at, job.created_at);
        assert_eq!(restored.started_at, job.started_at);
        assert_eq!(restored.completed_at, job.completed_at);
        assert_eq!(restored.worker_id, job.worker_id);
        assert_eq!(restored.artifact_count, job.artifact_count);
        assert_eq!(restored.error, job.error);
    }

    /// `serde_json::Value` graph field must preserve arbitrary JSON content.
    #[test]
    fn job_graph_json_value() {
        let graph = serde_json::json!({
            "nodes": [
                {"id": "load", "type": "ZitLoadPipeline", "inputs": {"model": "path/to/model.safetensors"}},
                {"id": "encode", "type": "ZitTextEncode", "inputs": {"text": "a beautiful sunset"}}
            ],
            "edges": [["load", "encode"]]
        });

        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: graph.clone(),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };

        let json = serde_json::to_string(&job).expect("serialize Job with graph");
        let restored: Job = serde_json::from_str(&json).expect("deserialize Job");

        assert_eq!(restored.graph, graph);
    }

    /// `SubmitJobRequest` must serialize and deserialize correctly.
    #[test]
    fn submit_job_request_roundtrip() {
        let req = SubmitJobRequest {
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings {
                seed: 99,
                steps: 10,
                guidance_scale: 5.0,
                width: 256,
                height: 256,
                device_preference: Some(2),
            },
        };

        let json = serde_json::to_string(&req).expect("serialize SubmitJobRequest");
        let restored: SubmitJobRequest =
            serde_json::from_str(&json).expect("deserialize SubmitJobRequest");

        assert_eq!(restored.graph, req.graph);
        assert_eq!(restored.settings.seed, req.settings.seed);
        assert_eq!(restored.settings.steps, req.settings.steps);
        assert_eq!(
            restored.settings.guidance_scale,
            req.settings.guidance_scale
        );
        assert_eq!(restored.settings.width, req.settings.width);
        assert_eq!(restored.settings.height, req.settings.height);
        assert_eq!(
            restored.settings.device_preference,
            req.settings.device_preference
        );
    }

    /// `SubmitJobResponse` must serialize and deserialize correctly.
    #[test]
    fn submit_job_response_roundtrip() {
        let resp = SubmitJobResponse {
            job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            queue_position: 3,
        };

        let json = serde_json::to_string(&resp).expect("serialize SubmitJobResponse");
        let restored: SubmitJobResponse =
            serde_json::from_str(&json).expect("deserialize SubmitJobResponse");

        assert_eq!(restored.job_id, resp.job_id);
        assert_eq!(restored.queue_position, resp.queue_position);
    }

    /// `DateTime<Utc>` fields must serialize to ISO 8601 strings.
    #[test]
    fn job_timestamps_iso8601() {
        let ts = Utc::now();
        // Serialize via serde_json — chrono's default is RFC 3339 (a profile of ISO 8601)
        let val = serde_json::to_value(&ts).expect("serialize DateTime<Utc>");
        let json_str = val.as_str().expect("timestamp is a JSON string");

        // chrono's RFC 3339 serialization produces strings like
        // `2024-01-15T12:30:00.123456789Z`
        assert!(
            json_str.contains('T'),
            "timestamp must contain 'T' separator: {json_str}"
        );
        assert!(
            json_str.ends_with('Z') || json_str.contains('+'),
            "timestamp must end with 'Z' or have timezone offset: {json_str}"
        );
    }

    /// Verify that optional timestamp fields default to `None` when absent.
    #[test]
    fn job_optional_timestamps_default_none() {
        let minimal = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "Queued",
            "graph": {"nodes": []},
            "settings": {},
            "created_at": "2024-01-01T00:00:00Z"
        });

        let job: Job = serde_json::from_value(minimal).expect("minimal job parses");
        assert!(
            job.started_at.is_none(),
            "started_at must be None when absent"
        );
        assert!(
            job.completed_at.is_none(),
            "completed_at must be None when absent"
        );
    }

    /// Verify that optional string fields default to `None` when absent.
    #[test]
    fn job_optional_string_fields_default_none() {
        let minimal = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "Queued",
            "graph": {"nodes": []},
            "settings": {},
            "created_at": "2024-01-01T00:00:00Z"
        });

        let job: Job = serde_json::from_value(minimal).expect("minimal job parses");
        assert!(
            job.worker_id.is_none(),
            "worker_id must be None when absent"
        );
        assert!(job.error.is_none(), "error must be None when absent");
    }

    /// Verify that optional numeric fields default correctly.
    #[test]
    fn job_optional_numeric_fields_default() {
        let minimal = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "status": "Queued",
            "graph": {"nodes": []},
            "settings": {},
            "created_at": "2024-01-01T00:00:00Z"
        });

        let job: Job = serde_json::from_value(minimal).expect("minimal job parses");
        assert!(
            job.device_index.is_none(),
            "device_index must be None when absent"
        );
        assert_eq!(job.artifact_count, 0, "artifact_count must default to 0");
    }
}
