//! JobScheduler — central orchestrator for job submission and dispatch coordination.
//!
//! Wraps the in-memory queue, database pool, event broadcaster, a `Notify` handle
//! used by the dispatch loop to wake on new submissions, and a `WorkerPool` for
//! worker management and IPC.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::events::{JobQueuedEvent, JobStartedEvent, WsEvent};
use anvilml_core::types::job::{Job, JobStatus, SubmitJobRequest, SubmitJobResponse};
use anvilml_ipc::WorkerMessage;
use anvilml_worker::WorkerPool;
use chrono::Utc;
use tokio::sync::broadcast;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::dag::validate_graph;
use crate::job_store::insert_job;
use crate::job_store::update_status;
use crate::ledger::VramLedger;
use crate::queue::JobQueue;

/// Central job scheduler.
///
/// Holds the in-memory queue, database pool, a broadcast sender for WebSocket events,
/// a `Notify` handle that the dispatch loop waits on, the worker pool for IPC and
/// status management, the VRAM ledger for worker selection, and the default device
/// mode (e.g. `"auto"` or `"cpu"`).
pub struct JobScheduler {
    /// In-memory FIFO queue of jobs awaiting dispatch.
    queue: Arc<JobQueue>,
    /// Worker pool for IPC, status management, and event subscription.
    workers: Arc<WorkerPool>,
    /// SQLite connection pool for job persistence.
    db: sqlx::SqlitePool,
    /// Broadcaster for WebSocket events (e.g. `job.queued`).
    broadcaster: broadcast::Sender<WsEvent>,
    /// Notify for dispatch loop wake-up.
    dispatch_notify: Arc<Notify>,
    /// VRAM ledger for worker selection ranking.
    ledger: Arc<tokio::sync::Mutex<VramLedger>>,
    /// Default device mode: `"auto"` or `"cpu"`.
    default_device: String,
}

impl JobScheduler {
    /// Create a new `JobScheduler`.
    pub fn new(
        queue: JobQueue,
        workers: Arc<WorkerPool>,
        db: sqlx::SqlitePool,
        broadcaster: broadcast::Sender<WsEvent>,
        ledger: Arc<tokio::sync::Mutex<VramLedger>>,
        default_device: String,
    ) -> Self {
        Self {
            queue: Arc::new(queue),
            workers,
            db,
            broadcaster,
            dispatch_notify: Arc::new(Notify::new()),
            ledger,
            default_device,
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
        self.dispatch_notify.notify_one();

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

    /// Start the background dispatch loop.
    ///
    /// The loop subscribes to `WorkerPool::subscribe_events()` for worker events
    /// and wakes on the `Notify` handle when a new job is submitted. On each wake
    /// it pops the next queued job, selects an idle worker, updates the job status
    /// to Running in the database, marks the worker busy, broadcasts a
    /// `JobStarted` WebSocket event, and sends an `Execute` IPC message to the
    /// worker. The loop repeats until no further dispatch is possible, then waits
    /// on both the `Notify` and worker events.
    ///
    /// Returns a [`JoinHandle`] for the dispatch loop task.
    #[tracing::instrument(skip(self), fields(default_device = %self.default_device))]
    pub fn start_dispatch_loop(&self) -> JoinHandle<()> {
        let notify = self.dispatch_notify.clone();
        let workers = self.workers.clone();
        let queue = self.queue.clone();
        let db = self.db.clone();
        let broadcaster = self.broadcaster.clone();
        let ledger = self.ledger.clone();
        let default_device = self.default_device.clone();

        tokio::spawn(async move {
            tracing::info!("dispatch loop started");

            loop {
                // Wait for a trigger: new job notification.
                // Use a short timeout so we periodically check even without
                // a notification (handles the case where notify is missed).
                let notified =
                    tokio::time::timeout(std::time::Duration::from_millis(100), notify.notified())
                        .await;

                if notified.is_err() {
                    // Timeout — no notification received, just check the queue.
                    tracing::debug!("dispatch loop: timeout, checking queue");
                } else {
                    tracing::debug!("dispatch loop: job submitted notification");
                }

                // Try to dispatch as many queued jobs as possible.
                while let Some(job) = queue.pop_next() {
                    let job_id = job.id;

                    // Get worker list and select an idle worker.
                    let worker_infos = workers.list().await;
                    let ledger_guard = ledger.lock().await;
                    let worker_idx =
                        select_worker(&job, &worker_infos, &ledger_guard, &default_device);

                    let Some(idx) = worker_idx else {
                        // No suitable worker — push job back and exit dispatch cycle.
                        queue.enqueue(job);
                        break;
                    };

                    let worker_info = &worker_infos[idx];
                    let worker_id = worker_info.worker_id.clone();
                    let device_index = worker_info.device_index;

                    drop(ledger_guard);

                    // Update job status to Running in the database.
                    let now = Utc::now();
                    let updated = update_status(&db, job_id, JobStatus::Running, Some(now))
                        .await
                        .unwrap_or(false);

                    if !updated {
                        tracing::warn!(
                            job_id = %job_id,
                            worker_id = %worker_id,
                            "dispatch: failed to update job status to Running"
                        );
                        queue.enqueue(job);
                        continue;
                    }

                    // Mark worker as busy.
                    workers.set_busy(&worker_id, &job_id.to_string()).await;

                    // Broadcast JobStarted event.
                    let started_event = WsEvent::JobStarted(JobStartedEvent {
                        event: "job.started".to_string(),
                        timestamp: now,
                        job_id,
                    });
                    let _ = broadcaster.send(started_event);

                    // Send Execute IPC message to the worker.
                    let execute_msg = WorkerMessage::Execute {
                        job_id,
                        graph: job.graph.clone(),
                        settings: job.settings.clone(),
                        device_index,
                    };
                    if let Err(e) = workers.send(&worker_id, execute_msg).await {
                        tracing::warn!(
                            job_id = %job_id,
                            worker_id = %worker_id,
                            error = %e,
                            "dispatch: failed to send Execute to worker"
                        );
                        // Try to enqueue the job back for retry.
                        queue.enqueue(job);
                        continue;
                    }

                    tracing::debug!(
                        job_id = %job_id,
                        worker_id = %worker_id,
                        "job dispatched to worker"
                    );
                }
            }
        })
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
        let (broadcaster, _rx) = broadcast::channel(16);

        JobScheduler::new(
            queue,
            Arc::new(WorkerPool::new_test_pool()),
            pool,
            broadcaster,
            Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
            "auto".to_string(),
        )
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
            Arc::new(WorkerPool::new_test_pool()),
            pool,
            broadcaster,
            Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
            "auto".to_string(),
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

    // ── dispatch loop test ────────────────────────────────────────────────────

    /// Submitting a job causes the dispatch loop to:
    /// - Transition the job's DB status from Queued → Running
    /// - Mark the worker Busy
    /// - Decrease the queue length
    ///
    /// Note: We verify observable state changes. The actual Execute IPC
    /// message send is verified indirectly by the worker becoming Busy.
    #[serial]
    #[tokio::test]
    async fn test_dispatch_sends_execute() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        // Set the worker to Idle so select_worker picks it.
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));

        // Build the scheduler.
        let pool_clone = pool.clone();
        let pool_clone2 = pool.clone();
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, _rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool_clone,
            setup_pool().await,
            broadcaster,
            ledger.clone(),
            "auto".to_string(),
        );

        // Start the dispatch loop.
        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — this notifies the dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for the dispatch loop to process the job.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // 1. Verify DB status is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists in DB");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job status should be Running after dispatch"
        );
        assert!(
            db_job.started_at.is_some(),
            "started_at should be set after dispatch"
        );

        // 2. Verify worker is Busy.
        let infos = pool_clone2.list().await;
        assert_eq!(infos.len(), 1, "should have exactly one worker");
        assert_eq!(
            infos[0].status,
            WorkerStatus::Busy,
            "worker should be Busy after dispatch"
        );

        // 3. Verify queue length decreased (job was dequeued).
        assert!(
            scheduler.queue.is_empty(),
            "queue should be empty after dispatch"
        );

        // Cleanup: abort the dispatch loop.
        dispatch_handle.abort();
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
