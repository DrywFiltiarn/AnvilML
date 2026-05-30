//! Job lifecycle types — the core data contract for job submission and tracking.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// JobStatus — lifecycle state of a job
// ---------------------------------------------------------------------------

/// The lifecycle status of a job.
///
/// Ordered from earliest to latest state. `Pending` is the initial state;
/// `Completed`, `Failed`, and `Cancelled` are terminal states.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
pub enum JobStatus {
    /// The job has been submitted but not yet picked up by a worker.
    Pending,
    /// A worker has begun processing the job.
    Running,
    /// The job completed successfully.
    Completed,
    /// The job failed with an error.
    Failed,
    /// The job was cancelled (by user or scheduler).
    Cancelled,
}

// ---------------------------------------------------------------------------
// JobSettings — configuration for a single job
// ---------------------------------------------------------------------------

/// Configuration parameters that define how a job should be executed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct JobSettings {
    /// Identifier of the model to use for this job.
    pub model_id: Uuid,

    /// Kind of model (optional; inferred from model registry if omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Target device string (e.g. "cuda:0", "cpu", "rocm").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,

    /// Number of inference steps.
    #[serde(default = "default_num_steps")]
    pub num_steps: u32,

    /// Random seed for reproducibility (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            model_id: Uuid::default(),
            kind: None,
            device: None,
            num_steps: default_num_steps(),
            seed: None,
        }
    }
}

fn default_num_steps() -> u32 {
    20
}

// ---------------------------------------------------------------------------
// Job — the primary domain entity
// ---------------------------------------------------------------------------

/// A job represents a single execution request in the AnvilML system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct Job {
    /// Unique identifier for this job (UUID v4).
    pub id: Uuid,

    /// Current lifecycle status.
    pub status: JobStatus,

    /// Job configuration parameters.
    pub settings: JobSettings,

    /// Timestamp when the job was created.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,

    /// Timestamp of the most recent status transition.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

impl Job {
    /// Create a new `Job` in the `Pending` state with the given settings.
    pub fn new(settings: JobSettings) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            status: JobStatus::Pending,
            settings,
            created_at: now,
            updated_at: now,
        }
    }
}

// ---------------------------------------------------------------------------
// SubmitJobRequest — API/IPC input for creating a new job
// ---------------------------------------------------------------------------

/// Request body submitted by a client to create a new job.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct SubmitJobRequest {
    /// Identifier of the model to use.
    pub model_id: Uuid,

    /// Kind of model (e.g. "diffusion", "clip").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Target device string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,

    /// Number of inference steps.
    #[serde(default = "default_num_steps")]
    pub num_steps: u32,

    /// Random seed for reproducibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl From<SubmitJobRequest> for JobSettings {
    fn from(req: SubmitJobRequest) -> Self {
        JobSettings {
            model_id: req.model_id,
            kind: req.kind,
            device: req.device,
            num_steps: req.num_steps,
            seed: req.seed,
        }
    }
}

// ---------------------------------------------------------------------------
// SubmitJobResponse — API/IPC output after job creation
// ---------------------------------------------------------------------------

/// Response returned after a job is successfully submitted.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct SubmitJobResponse {
    /// The unique identifier of the newly created job.
    pub job_id: Uuid,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // JobStatus — PartialEq / Eq / Ord
    // ------------------------------------------------------------------

    #[test]
    fn job_status_ord() {
        assert!(JobStatus::Pending < JobStatus::Running);
        assert!(JobStatus::Running < JobStatus::Completed);
        assert!(JobStatus::Running < JobStatus::Failed);
        assert!(JobStatus::Running < JobStatus::Cancelled);
    }

    #[test]
    fn job_status_eq() {
        assert_eq!(JobStatus::Pending, JobStatus::Pending);
        assert_ne!(JobStatus::Pending, JobStatus::Running);
    }

    #[test]
    fn job_status_serialization_round_trip() {
        for status in [
            JobStatus::Pending,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: JobStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back, "failed for {:?}", status);
        }
    }

    // ------------------------------------------------------------------
    // JobSettings — defaults and round-trip
    // ------------------------------------------------------------------

    #[test]
    fn job_settings_defaults() {
        let settings = JobSettings::default();
        assert_eq!(settings.num_steps, 20);
        assert!(settings.kind.is_none());
        assert!(settings.device.is_none());
        assert!(settings.seed.is_none());
    }

    #[test]
    fn job_settings_round_trip() {
        let settings = JobSettings {
            model_id: Uuid::new_v4(),
            kind: Some("diffusion".into()),
            device: Some("cuda:0".into()),
            num_steps: 50,
            seed: Some(42),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let back: JobSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, back);
    }

    // ------------------------------------------------------------------
    // Job — construction, serialization, defaults
    // ------------------------------------------------------------------

    #[test]
    fn job_new_is_pending() {
        let settings = JobSettings::default();
        let job = Job::new(settings.clone());
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.settings, settings);
        assert_eq!(job.created_at, job.updated_at);
    }

    #[test]
    fn job_id_is_uuid_v4() {
        let job = Job::new(JobSettings::default());
        // UUID v4 has specific version bits set; verify it parses as v4.
        assert_eq!(job.id.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn job_serialization_round_trip() {
        let settings = JobSettings {
            model_id: Uuid::new_v4(),
            kind: Some("clip".into()),
            device: Some("cpu".into()),
            num_steps: 30,
            seed: Some(123),
        };
        let job = Job::new(settings);
        let json = serde_json::to_string(&job).unwrap();
        let back: Job = serde_json::from_str(&json).unwrap();
        assert_eq!(job.id, back.id);
        assert_eq!(job.status, back.status);
        assert_eq!(job.settings, back.settings);
        // ts_seconds serializes to whole Unix seconds; compare by timestamp().
        assert_eq!(job.created_at.timestamp(), back.created_at.timestamp());
        assert_eq!(job.updated_at.timestamp(), back.updated_at.timestamp());
    }

    // ------------------------------------------------------------------
    // SubmitJobRequest — conversion to JobSettings
    // ------------------------------------------------------------------

    #[test]
    fn submit_job_request_to_settings() {
        let req = SubmitJobRequest {
            model_id: Uuid::new_v4(),
            kind: Some("vae".into()),
            device: Some("cuda:1".into()),
            num_steps: 100,
            seed: Some(999),
        };
        let model_id = req.model_id;
        let kind = req.kind.clone();
        let device = req.device.clone();
        let num_steps = req.num_steps;
        let seed = req.seed;
        let settings: JobSettings = req.into();
        assert_eq!(settings.model_id, model_id);
        assert_eq!(settings.kind, kind);
        assert_eq!(settings.device, device);
        assert_eq!(settings.num_steps, num_steps);
        assert_eq!(settings.seed, seed);
    }

    #[test]
    fn submit_job_request_defaults() {
        let req = SubmitJobRequest {
            model_id: Uuid::new_v4(),
            kind: None,
            device: None,
            num_steps: 0, // test default override
            seed: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SubmitJobRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.model_id, back.model_id);
        assert_eq!(back.num_steps, 0); // explicit 0 preserved
    }

    #[test]
    fn submit_job_response_round_trip() {
        let job_id = Uuid::new_v4();
        let resp = SubmitJobResponse { job_id };
        let json = serde_json::to_string(&resp).unwrap();
        let back: SubmitJobResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.job_id, back.job_id);
    }

    // ------------------------------------------------------------------
    // DateTime serialization
    // ------------------------------------------------------------------

    #[test]
    fn job_datetime_serialization() {
        let settings = JobSettings::default();
        let job = Job::new(settings);
        let json = serde_json::to_string(&job).unwrap();
        // Verify the JSON contains timestamp fields as integers (unix seconds)
        assert!(json.contains("created_at"));
        assert!(json.contains("updated_at"));
        // Deserialize should succeed
        let back: Job = serde_json::from_str(&json).unwrap();
        assert_eq!(job.created_at.timestamp(), back.created_at.timestamp());
    }
}
