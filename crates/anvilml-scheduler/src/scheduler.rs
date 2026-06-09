//! JobScheduler — central orchestrator for job submission and dispatch coordination.
//!
//! Wraps the in-memory queue, database pool, event broadcaster, and a `Notify` handle
//! used by the (future) dispatch loop to wake on new submissions.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::events::{JobQueuedEvent, WsEvent};
use anvilml_core::types::job::{Job, JobStatus, SubmitJobRequest, SubmitJobResponse};
use chrono::Utc;
use tokio::sync::broadcast;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::dag::validate_graph;
use crate::job_store::insert_job;
use crate::ledger::VramLedger;
use crate::queue::JobQueue;

/// Central job scheduler.
///
/// Holds the in-memory queue, database pool, a broadcast sender for WebSocket events,
/// and a `Notify` handle that the dispatch loop waits on.
pub struct JobScheduler {
    /// In-memory FIFO queue of jobs awaiting dispatch.
    queue: JobQueue,
    /// List of available workers (read-only snapshot; populated by server).
    _workers: Arc<Vec<anvilml_core::types::worker::WorkerInfo>>,
    /// SQLite connection pool for job persistence.
    db: sqlx::SqlitePool,
    /// Broadcaster for WebSocket events (e.g. `job.queued`).
    broadcaster: broadcast::Sender<WsEvent>,
    /// Notifies the dispatch loop when a new job is submitted.
    notify: Arc<Notify>,
}

impl JobScheduler {
    /// Create a new `JobScheduler`.
    pub fn new(
        queue: JobQueue,
        workers: Arc<Vec<anvilml_core::types::worker::WorkerInfo>>,
        db: sqlx::SqlitePool,
        broadcaster: broadcast::Sender<WsEvent>,
        notify: Arc<Notify>,
    ) -> Self {
        Self {
            queue,
            _workers: workers,
            db,
            broadcaster,
            notify,
        }
    }

    /// Submit a new job: validate graph → persist as Queued → enqueue → broadcast → notify.
    ///
    /// Returns `SubmitJobResponse` with the job ID and its 1-based queue position.
    #[tracing::instrument(skip(self, req), fields(job_id = tracing::field::Empty))]
    pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError> {
        // 1. Validate the DAG graph.
        validate_graph(&req.graph).map_err(|errors| AnvilError::InvalidGraph(errors.join("; ")))?;

        // 2. Build a Job struct with status=Queued.
        let job_id = Uuid::new_v4();
        let now = Utc::now();
        let job = Job {
            id: job_id,
            status: JobStatus::Queued,
            graph: req.graph.clone(),
            settings: req.settings.clone(),
            device_index: None,
            created_at: now,
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };

        // 3. Persist to database.
        insert_job(&self.db, &job)
            .await
            .map_err(|e| AnvilError::DbError(format!("failed to insert job: {e}")))?;

        tracing::debug!(job_id = %job_id, status = "Queued", "job status transition");

        tracing::info!(job_id = %job_id, "job submitted and persisted as Queued");

        // 4. Enqueue in the in-memory queue.
        self.queue.enqueue(job.clone());

        // 5. Broadcast job.queued event.
        let queued_event = WsEvent::JobQueued(JobQueuedEvent {
            event: "job.queued".to_string(),
            timestamp: now,
            job_id,
        });
        let _ = self.broadcaster.send(queued_event);

        // 6. Notify the dispatch loop.
        self.notify.notify_one();

        // 7. Return response with queue position (1-based).
        Ok(SubmitJobResponse {
            job_id,
            queue_position: self.queue.len() as u32,
        })
    }

    /// Return a reference to the in-memory queue length.
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }
}

/// Select the best idle worker for a job.
///
/// Implements three selection modes:
/// 1. **Force-CPU** — when `default_device == "cpu"`, only the worker whose
///    `device_name` is `"CPU"` is considered.
/// 2. **User-specified** — when `job.settings.device_preference` is `Some(n)`,
///    the worker at index `n` is selected if it is `Idle`.
/// 3. **Auto** — all `Idle` workers are ranked by `free_mib` descending,
///    ties broken by `device_index` ascending, and the top-ranked worker
///    is returned.
///
/// Returns `None` when no suitable worker is found.
pub fn select_worker(
    job: &Job,
    workers: &[anvilml_core::types::worker::WorkerInfo],
    ledger: &VramLedger,
    default_device: &str,
) -> Option<usize> {
    // 1. Force-CPU mode: only consider the CPU worker.
    if default_device == "cpu" {
        return workers.iter().position(|w| {
            w.device_name == "CPU" && w.status == anvilml_core::types::worker::WorkerStatus::Idle
        });
    }

    // 2. Device preference: user-specified index.
    if let Some(n) = job.settings.device_preference {
        let n = n as usize;
        if n < workers.len() && workers[n].status == anvilml_core::types::worker::WorkerStatus::Idle
        {
            return Some(n);
        }
        return None;
    }

    // 3. Auto mode: rank idle workers by free_mib desc, then device_index asc.
    let mut idle_workers: Vec<(usize, &anvilml_core::types::worker::WorkerInfo)> = workers
        .iter()
        .enumerate()
        .filter(|(_, w)| w.status == anvilml_core::types::worker::WorkerStatus::Idle)
        .collect();

    idle_workers.sort_by(|a, b| {
        let free_a = ledger.free_mib(a.1.device_index);
        let free_b = ledger.free_mib(b.1.device_index);
        free_b
            .cmp(&free_a)
            .then_with(|| a.1.device_index.cmp(&b.1.device_index))
    });

    idle_workers.into_iter().next().map(|(idx, _)| idx)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    use crate::job_store::get_job;
    use anvilml_core::types::job::JobSettings;
    use anvilml_core::types::worker::{WorkerInfo, WorkerStatus};
    use sqlx::SqlitePool;

    /// Create an in-memory SQLite pool and initialise the `jobs` table.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id         TEXT PRIMARY KEY,
                status     TEXT    NOT NULL DEFAULT 'Queued',
                graph      TEXT    NOT NULL,
                settings   TEXT    NOT NULL,
                device_index INTEGER          DEFAULT -1,
                created_at INTEGER   NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                worker_id  TEXT,
                artifact_count INTEGER DEFAULT 0,
                error      TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create jobs table");

        pool
    }

    /// Helper: create a valid ZiT 2-node graph.
    fn valid_zit_graph() -> serde_json::Value {
        serde_json::json!({
            "nodes": [
                { "id": "load", "type": "ZitLoadPipeline" },
                {
                    "id": "encode",
                    "type": "ZitTextEncode",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "pipeline" }
                    }
                }
            ],
            "edges": []
        })
    }

    /// Helper: create a JobScheduler with fresh components.
    async fn make_scheduler(pool: SqlitePool) -> JobScheduler {
        let queue = JobQueue::new();
        let workers: Arc<Vec<anvilml_core::types::worker::WorkerInfo>> = Arc::new(vec![]);
        let (broadcaster, _rx) = broadcast::channel(16);
        let notify = Arc::new(Notify::new());

        JobScheduler::new(queue, workers, pool, broadcaster, notify)
    }

    /// Valid job submitted via `submit()` is persisted as Queued + enqueued + returns
    /// response with job_id and queue_position.
    #[serial]
    #[tokio::test]
    async fn test_submit_valid_job() {
        let pool = setup_pool().await;
        let scheduler = make_scheduler(pool).await;

        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };

        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Response assertions.
        assert!(!resp.job_id.is_nil(), "job_id must be non-empty UUID");
        assert!(resp.queue_position >= 1, "queue_position must be >= 1");

        // Database assertions: job exists and is Queued.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists in DB");
        assert_eq!(db_job.status, JobStatus::Queued);
        assert_eq!(db_job.id, resp.job_id);

        // Queue length increased.
        assert_eq!(scheduler.queue.len(), 1);
    }

    /// Invalid graph (unknown node type) returns AnvilError::InvalidGraph, no DB row,
    /// and queue length is unchanged.
    #[serial]
    #[tokio::test]
    async fn test_submit_invalid_graph() {
        let pool = setup_pool().await;
        let scheduler = make_scheduler(pool).await;

        let initial_queue_len = scheduler.queue.len();

        let graph = serde_json::json!({
            "nodes": [
                { "id": "n0", "type": "NopeNode" }
            ],
            "edges": []
        });
        let req = SubmitJobRequest {
            graph,
            settings: JobSettings::default(),
        };

        let result = scheduler.submit(req).await;

        // Must return InvalidGraph error.
        match result {
            Err(AnvilError::InvalidGraph(msg)) => {
                assert!(
                    msg.contains("NopeNode"),
                    "error must mention the unknown node type: {msg}"
                );
            }
            other => panic!("expected AnvilError::InvalidGraph, got {other:?}"),
        }

        // Queue length must be unchanged.
        assert_eq!(scheduler.queue.len(), initial_queue_len);

        // No job was persisted (we can't know the UUID, but the queue check above is sufficient).
    }

    /// WsEvent::JobQueued is sent on the broadcast channel with matching job_id.
    #[serial]
    #[tokio::test]
    async fn test_submit_broadcasts_event() {
        let pool = setup_pool().await;
        let (broadcaster, mut rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            Arc::new(vec![]),
            pool,
            broadcaster,
            Arc::new(Notify::new()),
        );

        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };

        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Receive the broadcast event.
        let event = rx.recv().await.expect("received broadcast event");

        match event {
            WsEvent::JobQueued(jqe) => {
                assert_eq!(jqe.job_id, resp.job_id);
                assert_eq!(jqe.event, "job.queued");
            }
            other => panic!("expected WsEvent::JobQueued, got {:?}", other),
        }
    }

    /// Custom JobSettings (seed, steps, guidance_scale, width, height) round-trip
    /// through submit → database.
    #[serial]
    #[tokio::test]
    async fn test_submit_persists_settings() {
        let pool = setup_pool().await;
        let scheduler = make_scheduler(pool).await;

        let custom_settings = JobSettings {
            seed: 42,
            steps: 50,
            guidance_scale: 12.0,
            width: 768,
            height: 512,
            device_preference: Some(0),
        };

        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: custom_settings.clone(),
        };

        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Retrieve from DB and verify settings.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists in DB");

        assert_eq!(db_job.settings.seed, custom_settings.seed);
        assert_eq!(db_job.settings.steps, custom_settings.steps);
        assert_eq!(
            db_job.settings.guidance_scale,
            custom_settings.guidance_scale
        );
        assert_eq!(db_job.settings.width, custom_settings.width);
        assert_eq!(db_job.settings.height, custom_settings.height);
        assert_eq!(
            db_job.settings.device_preference,
            custom_settings.device_preference
        );
    }

    // ── select_worker tests ─────────────────────────────────────────────────

    /// `select_worker` with `device_preference = Some(0)` returns index 0
    /// when the worker at index 0 is Idle.
    #[serial]
    #[tokio::test]
    async fn test_select_preference_idle() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings {
                device_preference: Some(0),
                ..JobSettings::default()
            },
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "auto");
        assert_eq!(result, Some(0));
    }

    /// `select_worker` with `device_preference = Some(0)` returns `None`
    /// when the worker at index 0 is Busy.
    #[serial]
    #[tokio::test]
    async fn test_select_preference_busy() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings {
                device_preference: Some(0),
                ..JobSettings::default()
            },
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Busy,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "auto");
        assert_eq!(result, None);
    }

    /// `select_worker` with `device_preference = Some(99)` returns `None`
    /// when no worker exists at that index.
    #[serial]
    #[tokio::test]
    async fn test_select_preference_not_found() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings {
                device_preference: Some(99),
                ..JobSettings::default()
            },
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![WorkerInfo {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            device_name: "GPU 0".to_string(),
            status: WorkerStatus::Idle,
            current_job_id: None,
            vram_used_mib: 0,
        }];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "auto");
        assert_eq!(result, None);
    }

    /// Auto mode returns the only idle worker when there is exactly one.
    #[serial]
    #[tokio::test]
    async fn test_select_auto_single_idle() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Busy,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "auto");
        assert_eq!(result, Some(1));
    }

    /// Auto mode picks the worker with the highest `free_mib` from the ledger.
    #[serial]
    #[tokio::test]
    async fn test_select_auto_ranked_by_free_mib() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let mut ledger = VramLedger::new();
        ledger.update(0, 6000, 8192); // free = 2192
        ledger.update(1, 2000, 8192); // free = 6192

        let result = select_worker(&job, &workers, &ledger, "auto");
        // Worker 1 has more free VRAM.
        assert_eq!(result, Some(1));
    }

    /// Auto mode breaks free_mib ties by `device_index` ascending.
    #[serial]
    #[tokio::test]
    async fn test_select_auto_tie_break_device_index() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let mut ledger = VramLedger::new();
        ledger.update(0, 4000, 8192); // free = 4192
        ledger.update(1, 4000, 8192); // free = 4192

        let result = select_worker(&job, &workers, &ledger, "auto");
        // Same free_mib — lower device_index wins.
        assert_eq!(result, Some(0));
    }

    /// Auto mode returns `None` when all workers are Busy.
    #[serial]
    #[tokio::test]
    async fn test_select_auto_all_busy() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Busy,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-1".to_string(),
                device_index: 1,
                device_name: "GPU 1".to_string(),
                status: WorkerStatus::Busy,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "auto");
        assert_eq!(result, None);
    }

    /// Force-CPU mode (`default_device == "cpu"`) picks the CPU worker.
    #[serial]
    #[tokio::test]
    async fn test_select_cpu() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![
            WorkerInfo {
                worker_id: "worker-0".to_string(),
                device_index: 0,
                device_name: "GPU 0".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
            WorkerInfo {
                worker_id: "worker-cpu".to_string(),
                device_index: 99,
                device_name: "CPU".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: 0,
            },
        ];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "cpu");
        assert_eq!(result, Some(1));
    }

    /// Force-CPU mode returns `None` when no CPU worker exists.
    #[serial]
    #[tokio::test]
    async fn test_select_cpu_not_available() {
        let job = Job {
            id: Uuid::new_v4(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let workers = vec![WorkerInfo {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            device_name: "GPU 0".to_string(),
            status: WorkerStatus::Idle,
            current_job_id: None,
            vram_used_mib: 0,
        }];
        let ledger = VramLedger::new();

        let result = select_worker(&job, &workers, &ledger, "cpu");
        assert_eq!(result, None);
    }
}
