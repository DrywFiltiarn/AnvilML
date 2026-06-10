//! JobScheduler — central orchestrator for job submission and dispatch coordination.
//!
//! Wraps the in-memory queue, database pool, event broadcaster, a `Notify` handle
//! used by the dispatch loop to wake on new submissions, and a `WorkerPool` for
//! worker management and IPC.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::artifact::{ArtifactSave, ArtifactSaveInput};
use anvilml_core::types::events::{
    JobCancelledEvent, JobCompletedEvent, JobFailedEvent, JobImageReadyEvent, JobProgressEvent,
    JobQueuedEvent, JobStartedEvent, WsEvent,
};
use anvilml_core::types::job::{Job, JobStatus, SubmitJobRequest, SubmitJobResponse};
use anvilml_ipc::{WorkerEvent, WorkerMessage};
use anvilml_worker::WorkerPool;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::dag::validate_graph;
use crate::job_store::get_job;
use crate::job_store::insert_job;
use crate::job_store::update_status;
use crate::ledger::VramLedger;
use crate::queue::JobQueue;

/// Central job scheduler.
///
/// Holds the in-memory queue, database pool, a broadcast sender for WebSocket events,
/// a `Notify` handle that the dispatch loop waits on, the worker pool for IPC and
/// status management, the VRAM ledger for worker selection, the default device
/// mode (e.g. `"auto"` or `"cpu"`), and an artifact store for persisting generated images.
pub struct JobScheduler<A: ArtifactSave + Clone + 'static> {
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
    /// Artifact store for persisting generated images.
    artifact_store: A,
}

impl<A: ArtifactSave + Clone + 'static> JobScheduler<A> {
    /// Create a new `JobScheduler`.
    pub fn new(
        queue: JobQueue,
        workers: Arc<WorkerPool>,
        db: sqlx::SqlitePool,
        broadcaster: broadcast::Sender<WsEvent>,
        ledger: Arc<tokio::sync::Mutex<VramLedger>>,
        default_device: String,
        artifact_store: A,
    ) -> Self {
        Self {
            queue: Arc::new(queue),
            workers,
            db,
            broadcaster,
            dispatch_notify: Arc::new(Notify::new()),
            ledger,
            default_device,
            artifact_store,
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

    /// Cancel a job by its ID.
    ///
    /// Reads the job from the database. If the job is not found, returns
    /// `AnvilError::JobNotFound`. If the job is in a terminal state
    /// (Completed, Failed, Cancelled), returns `AnvilError::JobNotCancellable`.
    ///
    /// For **Queued** jobs: removes from the in-memory queue, updates the DB
    /// to `Cancelled`, and broadcasts a `JobCancelled` event.
    ///
    /// For **Running** jobs: sends a `CancelJob` IPC message to the owning
    /// worker, updates the DB to `Cancelled`, and broadcasts a
    /// `JobCancelled` event.
    #[tracing::instrument(skip(self), fields(job_id = %id))]
    pub async fn cancel(&self, id: Uuid) -> Result<(), AnvilError> {
        // 1. Read job from DB.
        let job = get_job(&self.db, id)
            .await
            .map_err(|e| AnvilError::DbError(format!("failed to read job: {e}")))?;
        let job = job.ok_or(AnvilError::JobNotFound(id))?;

        // 2. Terminal → reject.
        if !matches!(job.status, JobStatus::Queued | JobStatus::Running) {
            return Err(AnvilError::JobNotCancellable(id));
        }

        let now = Utc::now();
        let worker_id = job.worker_id.clone();

        match job.status {
            JobStatus::Queued => {
                // 3a. Cancel in queue + update DB + broadcast.
                self.queue.cancel_queued(id);
                update_status(&self.db, id, JobStatus::Cancelled, None, None, None, None)
                    .await
                    .ok();
                let _ = self
                    .broadcaster
                    .send(WsEvent::JobCancelled(JobCancelledEvent {
                        event: "job.cancelled".to_string(),
                        timestamp: now,
                        job_id: id,
                    }));
                self.dispatch_notify.notify_one();
                tracing::info!(job_id = %id, "job cancelled (queued)");
            }
            JobStatus::Running => {
                // 3b. Send CancelJob IPC + set worker idle + update DB + broadcast.
                if let Some(ref wid) = worker_id {
                    let _ = self
                        .workers
                        .send(wid, WorkerMessage::CancelJob { job_id: id })
                        .await;
                    self.workers.set_idle(wid).await;
                }
                update_status(&self.db, id, JobStatus::Cancelled, None, None, None, None)
                    .await
                    .ok();
                let _ = self
                    .broadcaster
                    .send(WsEvent::JobCancelled(JobCancelledEvent {
                        event: "job.cancelled".to_string(),
                        timestamp: now,
                        job_id: id,
                    }));
                self.dispatch_notify.notify_one();
                tracing::info!(job_id = %id, "job cancel requested (running)");
            }
            _ => unreachable!(),
        }

        Ok(())
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
    /// Worker events for `Completed`, `Failed`, and `ImageReady` are handled by
    /// transitioning the job to its terminal status (or persisting the artifact),
    /// setting the worker idle, and broadcasting the appropriate WebSocket event.
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
        let artifact_store = self.artifact_store.clone();

        tokio::spawn(async move {
            tracing::info!("dispatch loop started");

            let mut event_rx = workers.subscribe_events();

            loop {
                tokio::select! {
                    _ = notify.notified() => {
                        tracing::debug!("dispatch loop: job submitted notification");
                    }
                    result = event_rx.recv() => {
                        match result {
                            Ok((worker_id, event)) => {
                                tracing::debug!(worker_id = %worker_id, event_type = ?event_discriminant(&event), "dispatch loop: received worker event");
                                match &event {
                                    WorkerEvent::Completed { job_id, elapsed_ms: _ } => {
                                        let now = Utc::now();
                                        handle_completed(&db, &workers, &broadcaster, &notify, *job_id, now).await;
                                        notify.notify_one();
                                    }
                                    WorkerEvent::Failed { job_id, error, traceback: _ } => {
                                        let now = Utc::now();
                                        handle_failed(&db, &workers, &broadcaster, &notify, *job_id, error.clone(), now).await;
                                        notify.notify_one();
                                    }
                                    WorkerEvent::ImageReady {
                                        job_id,
                                        image_b64,
                                        width,
                                        height,
                                        format: _,
                                        seed,
                                        steps,
                                        prompt,
                                    } => {
                                        let now = Utc::now();
                                        handle_image_ready(
                                            &artifact_store,
                                            &db,
                                            &broadcaster,
                                            &notify,
                                            *job_id,
                                            image_b64,
                                            *width,
                                            *height,
                                            *seed,
                                            *steps as i64,
                                            prompt,
                                            now,
                                        )
                                        .await;
                                        notify.notify_one();
                                    }
                                    WorkerEvent::Progress {
                                        job_id,
                                        node_index,
                                        node_total,
                                        node_type,
                                        step: _,
                                        step_total: _,
                                    } => {
                                        let now = Utc::now();
                                        // Broadcast JobProgress event (step/step_total None in MVP).
                                        let _ = broadcaster.send(WsEvent::JobProgress(JobProgressEvent {
                                            event: "job.progress".to_string(),
                                            timestamp: now,
                                            job_id: *job_id,
                                            node_index: *node_index,
                                            node_total: *node_total,
                                            node_type: node_type.clone(),
                                            step: None,
                                            step_total: None,
                                        }));
                                        tracing::debug!(
                                            job_id = %job_id,
                                            node_index = *node_index,
                                            node_type = %node_type,
                                            "dispatch loop: progress event broadcast"
                                        );
                                    }
                                    WorkerEvent::Cancelled { job_id } => {
                                        let now = Utc::now();
                                        handle_cancelled(
                                            &db,
                                            &workers,
                                            &broadcaster,
                                            &notify,
                                            *job_id,
                                            now,
                                        )
                                        .await;
                                        notify.notify_one();
                                    }
                                    _ => {} // MemoryReport, etc. — not handled here
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::debug!(lagged = n, "dispatch loop: dropped events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                tracing::debug!("dispatch loop: event channel closed");
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        if !queue.is_empty() {
                            tracing::debug!("dispatch loop: timeout, checking queue");
                        }
                    }
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
                    let updated = update_status(
                        &db,
                        job_id,
                        JobStatus::Running,
                        Some(now),
                        None,
                        None,
                        Some(worker_id.clone()),
                    )
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

/// Handle a `WorkerEvent::Completed` event: transition the job to terminal
/// status, set the worker idle, and broadcast the completion event.
async fn handle_completed(
    db: &sqlx::SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    now: DateTime<Utc>,
) {
    // Re-read job status from DB to confirm it's still Running.
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = ?job.status, "completed: job already terminal, ignoring");
        return;
    }

    // Update status to Completed.
    let _ = update_status(
        db,
        job_id,
        JobStatus::Completed,
        None,
        Some(now),
        None,
        None,
    )
    .await;
    tracing::info!(job_id = %job_id, "job completed");

    // Set worker idle.
    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    // Broadcast completion event.
    let _ = broadcaster.send(WsEvent::JobCompleted(JobCompletedEvent {
        event: "job.completed".to_string(),
        timestamp: now,
        job_id,
    }));

    // Wake dispatch loop for next job.
    notify.notify_one();
}

/// Handle a `WorkerEvent::Failed` event: transition the job to terminal
/// status, set the worker idle, and broadcast the failure event.
async fn handle_failed(
    db: &sqlx::SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    error: String,
    now: DateTime<Utc>,
) {
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = ?job.status, "failed: job already terminal, ignoring");
        return;
    }

    let _ = update_status(
        db,
        job_id,
        JobStatus::Failed,
        None,
        None,
        Some(error.clone()),
        None,
    )
    .await;
    tracing::info!(job_id = %job_id, error = %error, "job failed");

    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    let _ = broadcaster.send(WsEvent::JobFailed(JobFailedEvent {
        event: "job.failed".to_string(),
        timestamp: now,
        job_id,
        error,
        traceback: None,
    }));

    notify.notify_one();
}

/// Handle a `WorkerEvent::ImageReady` event: persist the artifact via
/// the artifact store, then broadcast a `JobImageReady` WebSocket event
/// containing metadata only (no image bytes).
#[expect(clippy::too_many_arguments)]
async fn handle_image_ready<A: ArtifactSave>(
    artifact_store: &A,
    db: &sqlx::SqlitePool,
    broadcaster: &broadcast::Sender<WsEvent>,
    _notify: &Arc<Notify>,
    job_id: Uuid,
    image_b64: &str,
    width: u32,
    height: u32,
    seed: i64,
    steps: i64,
    prompt: &str,
    now: DateTime<Utc>,
) {
    // Re-read job status from DB to confirm it's still Running.
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else {
        tracing::debug!(job_id = %job_id, "image_ready: job not found, ignoring");
        return;
    };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = ?job.status, "image_ready: job already terminal, ignoring");
        return;
    }

    // Save the artifact.
    let meta = ArtifactSaveInput {
        width: width as i64,
        height: height as i64,
        seed,
        steps,
        prompt: prompt.to_string(),
    };
    let result = artifact_store
        .save(&job_id.to_string(), image_b64, meta)
        .await;

    match result {
        Ok(hash) => {
            tracing::info!(job_id = %job_id, artifact_hash = %hash, "image saved to artifact store");

            // Broadcast JobImageReady event (metadata only, no image bytes).
            let _ = broadcaster.send(WsEvent::JobImageReady(JobImageReadyEvent {
                event: "job.image_ready".to_string(),
                timestamp: now,
                job_id,
                artifact_hash: hash,
                width,
                height,
                seed,
            }));
        }
        Err(e) => {
            tracing::warn!(job_id = %job_id, error = %e, "artifact save failed");
        }
    }
}

/// Handle a `WorkerEvent::Cancelled` event: transition the job to terminal
/// status, set the worker idle, and broadcast the cancellation event.
async fn handle_cancelled(
    db: &sqlx::SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    now: DateTime<Utc>,
) {
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running | JobStatus::Queued) {
        tracing::debug!(job_id = %job_id, status = ?job.status, "cancelled: job already terminal, ignoring");
        return;
    }

    let _ = update_status(db, job_id, JobStatus::Cancelled, None, None, None, None).await;
    tracing::info!(job_id = %job_id, "job cancelled");

    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    let _ = broadcaster.send(WsEvent::JobCancelled(JobCancelledEvent {
        event: "job.cancelled".to_string(),
        timestamp: now,
        job_id,
    }));

    notify.notify_one();
}

/// Get a discriminant name for a WorkerEvent.
fn event_discriminant(event: &anvilml_ipc::WorkerEvent) -> &'static str {
    match event {
        anvilml_ipc::WorkerEvent::Ready { .. } => "Ready",
        anvilml_ipc::WorkerEvent::Ping { .. } => "Ping",
        anvilml_ipc::WorkerEvent::Pong { .. } => "Pong",
        anvilml_ipc::WorkerEvent::Dying { .. } => "Dying",
        anvilml_ipc::WorkerEvent::MemoryReport { .. } => "MemoryReport",
        anvilml_ipc::WorkerEvent::Progress { .. } => "Progress",
        anvilml_ipc::WorkerEvent::ImageReady { .. } => "ImageReady",
        anvilml_ipc::WorkerEvent::Completed { .. } => "Completed",
        anvilml_ipc::WorkerEvent::Failed { .. } => "Failed",
        anvilml_ipc::WorkerEvent::Cancelled { .. } => "Cancelled",
        anvilml_ipc::WorkerEvent::WorkerStatusChanged { .. } => "WorkerStatusChanged",
    }
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

    /// A no-op artifact store for tests that don't exercise image saving.
    #[derive(Clone)]
    struct NoopArtifactStore;

    #[async_trait::async_trait]
    impl ArtifactSave for NoopArtifactStore {
        async fn save(
            &self,
            _job_id: &str,
            _image_b64: &str,
            _meta: ArtifactSaveInput,
        ) -> Result<String, String> {
            Ok(String::new())
        }
    }

    /// Helper: create a JobScheduler with fresh components.
    async fn make_scheduler(pool: SqlitePool) -> JobScheduler<NoopArtifactStore> {
        let queue = JobQueue::new();
        let (broadcaster, _rx) = broadcast::channel(16);

        JobScheduler::new(
            queue,
            Arc::new(WorkerPool::new_test_pool()),
            pool,
            broadcaster,
            Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
            "auto".to_string(),
            NoopArtifactStore,
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
            NoopArtifactStore,
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
            NoopArtifactStore,
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

    /// Submitting a job causes the dispatch loop to transition it to Running,
    /// then a Completed event from the worker transitions it to Completed.
    #[serial]
    #[tokio::test]
    async fn test_complete() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, _rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool.clone(),
            setup_pool().await,
            broadcaster,
            ledger,
            "auto".to_string(),
            NoopArtifactStore,
        );

        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — triggers dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for dispatch to move job to Running.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should be Running after dispatch"
        );

        // Inject Completed event via pool test helper.
        pool.publish_event(
            "worker-0".to_string(),
            WorkerEvent::Completed {
                job_id: resp.job_id,
                elapsed_ms: 42,
            },
        );

        // Wait for event handler to process.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Completed in DB.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Completed,
            "job should be Completed after event"
        );
        assert!(db_job.completed_at.is_some(), "completed_at should be set");

        // Verify worker is back to Idle.
        let infos = pool.list().await;
        assert_eq!(
            infos[0].status,
            WorkerStatus::Idle,
            "worker should be Idle after completion"
        );

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

    // ── ImageReady handler test ──────────────────────────────────────────────

    /// A `MockArtifactStore` that records the save calls it receives.
    #[derive(Clone)]
    struct MockArtifactStore {
        saved: Arc<tokio::sync::Mutex<Vec<(String, String)>>>,
    }

    impl MockArtifactStore {
        fn new() -> Self {
            Self {
                saved: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            }
        }

        async fn get_saved(&self) -> Vec<(String, String)> {
            self.saved.lock().await.clone()
        }
    }

    #[async_trait::async_trait]
    impl ArtifactSave for MockArtifactStore {
        async fn save(
            &self,
            job_id: &str,
            _image_b64: &str,
            _meta: ArtifactSaveInput,
        ) -> Result<String, String> {
            let hash = format!("mock-{job_id}");
            self.saved
                .lock()
                .await
                .push((job_id.to_string(), hash.clone()));
            Ok(hash)
        }
    }

    /// ImageReady event triggers artifact save and broadcasts JobImageReady with correct fields.
    #[serial]
    #[tokio::test]
    async fn test_image_ready_broadcasts_event() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, mut rx) = broadcast::channel(16);
        let mock_store = MockArtifactStore::new();

        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool.clone(),
            setup_pool().await,
            broadcaster,
            ledger,
            "auto".to_string(),
            mock_store.clone(),
        );

        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — triggers dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for dispatch to move job to Running.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should be Running after dispatch"
        );

        // Inject ImageReady event via pool test helper.
        pool.publish_event(
            "worker-0".to_string(),
            WorkerEvent::ImageReady {
                job_id: resp.job_id,
                image_b64: "iVBORw0KGgoAAAANSUhEUg==".to_string(), // minimal valid-ish base64
                width: 512,
                height: 512,
                format: "png".to_string(),
                seed: 42,
                steps: 30,
                prompt: "test prompt".to_string(),
            },
        );

        // Wait for event handler to process.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // 1. Verify mock store received the save call.
        let saved = mock_store.get_saved().await;
        assert_eq!(
            saved.len(),
            1,
            "mock store should have received one save call"
        );
        assert_eq!(saved[0].0, resp.job_id.to_string());
        assert!(saved[0].1.starts_with("mock-"));

        // 2. Verify JobImageReady was broadcast with correct fields.
        // Drain any preceding JobQueued/JobStarted events from the submit + dispatch.
        loop {
            let event = rx.recv().await.expect("received broadcast event");
            match &event {
                WsEvent::JobQueued(_) | WsEvent::JobStarted(_) => continue,
                WsEvent::JobImageReady(jire) => {
                    assert_eq!(jire.job_id, resp.job_id);
                    assert_eq!(jire.event, "job.image_ready");
                    assert_eq!(jire.width, 512);
                    assert_eq!(jire.height, 512);
                    assert_eq!(jire.seed, 42);
                    assert_eq!(jire.artifact_hash, saved[0].1);
                    break;
                }
                other => panic!("expected WsEvent::JobImageReady, got {:?}", other),
            }
        }

        // 3. Verify job status is still Running (ImageReady does not change status).
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should still be Running after image_ready"
        );

        dispatch_handle.abort();
    }

    // ── Progress event test ────────────────────────────────────────────────────

    /// A `WorkerEvent::Progress` event triggers a `WsEvent::JobProgress` broadcast
    /// with the correct job_id, node_index, node_total, node_type, and step/step_total
    /// set to None (MVP).
    #[serial]
    #[tokio::test]
    async fn test_progress_broadcasts_event() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, mut rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool.clone(),
            setup_pool().await,
            broadcaster,
            ledger,
            "auto".to_string(),
            NoopArtifactStore,
        );

        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — triggers dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for dispatch to move job to Running.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should be Running after dispatch"
        );

        // Inject Progress event via pool test helper.
        pool.publish_event(
            "worker-0".to_string(),
            WorkerEvent::Progress {
                job_id: resp.job_id,
                node_index: 3,
                node_total: 5,
                node_type: "Encode".to_string(),
                step: Some(10),
                step_total: Some(50),
            },
        );

        // Wait for event handler to process.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Drain any preceding JobQueued/JobStarted events from the submit + dispatch.
        let jpe = loop {
            let event = rx.recv().await.expect("received broadcast event");
            match event {
                WsEvent::JobQueued(_) | WsEvent::JobStarted(_) => continue,
                WsEvent::JobProgress(e) => break e,
                other => panic!("expected JobProgress, got {:?}", other),
            }
        };
        assert_eq!(jpe.job_id, resp.job_id);
        assert_eq!(jpe.event, "job.progress");
        assert_eq!(jpe.node_index, 3);
        assert_eq!(jpe.node_total, 5);
        assert_eq!(jpe.node_type, "Encode");
        assert!(jpe.step.is_none(), "step should be None in MVP");
        assert!(jpe.step_total.is_none(), "step_total should be None in MVP");

        // Job status should still be Running (Progress does not change status).
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should still be Running after progress"
        );

        dispatch_handle.abort();
    }

    // ── Cancel event test ──────────────────────────────────────────────────────

    /// A `WorkerEvent::Cancelled` event triggers a `WsEvent::JobCancelled` broadcast,
    /// transitions the job to Cancelled in the DB, and sets the worker back to Idle.
    #[serial]
    #[tokio::test]
    async fn test_cancel_broadcasts_event() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, mut rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool.clone(),
            setup_pool().await,
            broadcaster,
            ledger,
            "auto".to_string(),
            NoopArtifactStore,
        );

        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — triggers dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for dispatch to move job to Running.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should be Running after dispatch"
        );

        // Inject Cancelled event via pool test helper.
        pool.publish_event(
            "worker-0".to_string(),
            WorkerEvent::Cancelled {
                job_id: resp.job_id,
            },
        );

        // Wait for event handler to process.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Drain any preceding JobQueued/JobStarted events from the submit + dispatch.
        let jce = loop {
            let event = rx.recv().await.expect("received broadcast event");
            match event {
                WsEvent::JobQueued(_) | WsEvent::JobStarted(_) => continue,
                WsEvent::JobCancelled(e) => break e,
                other => panic!("expected JobCancelled, got {:?}", other),
            }
        };
        assert_eq!(jce.job_id, resp.job_id);
        assert_eq!(jce.event, "job.cancelled");

        // Verify job status is Cancelled in DB.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Cancelled,
            "job should be Cancelled after cancellation event"
        );

        // Verify worker is back to Idle.
        let infos = pool.list().await;
        assert_eq!(
            infos[0].status,
            WorkerStatus::Idle,
            "worker should be Idle after cancellation"
        );

        dispatch_handle.abort();
    }

    // ── Cancel method tests ────────────────────────────────────────────────────

    /// Submitting a job, calling `cancel()` on it while Queued transitions
    /// the DB status to Cancelled, removes it from the queue, and broadcasts
    /// a `JobCancelled` event.
    #[serial]
    #[tokio::test]
    async fn test_cancel_queued() {
        let pool = setup_pool().await;
        let (broadcaster, mut rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            Arc::new(WorkerPool::new_test_pool()),
            pool.clone(),
            broadcaster,
            Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
            "auto".to_string(),
            NoopArtifactStore,
        );

        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Verify job is Queued.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(db_job.status, JobStatus::Queued);
        assert_eq!(scheduler.queue.len(), 1);

        // Call cancel.
        scheduler
            .cancel(resp.job_id)
            .await
            .expect("cancel succeeded");

        // Verify DB status is Cancelled.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Cancelled,
            "job should be Cancelled after cancel()"
        );

        // Verify queue is empty (job was removed).
        assert_eq!(
            scheduler.queue.len(),
            0,
            "queue should be empty after cancel"
        );

        // Verify JobCancelled was broadcast.
        let jce = loop {
            let event = rx.recv().await.expect("received broadcast event");
            match event {
                WsEvent::JobQueued(_) => continue,
                WsEvent::JobCancelled(e) => break e,
                other => panic!("expected JobCancelled, got {:?}", other),
            }
        };
        assert_eq!(jce.job_id, resp.job_id);
        assert_eq!(jce.event, "job.cancelled");
    }

    /// Submitting a job, waiting for dispatch to make it Running, then calling
    /// `cancel()` sends a `CancelJob` IPC message to the worker, transitions
    /// the DB to Cancelled, and broadcasts a `JobCancelled` event.
    #[serial]
    #[tokio::test]
    async fn test_cancel_running() {
        use anvilml_worker::ManagedWorker;

        // Build a minimal WorkerPool with one idle worker.
        let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
        worker.set_status(WorkerStatus::Idle).await;

        let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
        let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
        let (broadcaster, mut rx) = broadcast::channel(16);
        let scheduler = JobScheduler::new(
            JobQueue::new(),
            pool.clone(),
            setup_pool().await,
            broadcaster,
            ledger,
            "auto".to_string(),
            NoopArtifactStore,
        );

        let dispatch_handle = scheduler.start_dispatch_loop();

        // Submit a job — triggers dispatch loop.
        let req = SubmitJobRequest {
            graph: valid_zit_graph(),
            settings: JobSettings::default(),
        };
        let resp = scheduler.submit(req).await.expect("submit succeeded");

        // Wait for dispatch to move job to Running.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify job is Running.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Running,
            "job should be Running after dispatch"
        );

        // Call cancel while running.
        scheduler
            .cancel(resp.job_id)
            .await
            .expect("cancel succeeded");

        // Verify DB status is Cancelled.
        let db_job = get_job(&scheduler.db, resp.job_id)
            .await
            .expect("get from DB succeeded")
            .expect("job exists");
        assert_eq!(
            db_job.status,
            JobStatus::Cancelled,
            "job should be Cancelled after cancel()"
        );

        // Verify worker is back to Idle (set_idle was called).
        let infos = pool.list().await;
        assert_eq!(
            infos[0].status,
            WorkerStatus::Idle,
            "worker should be Idle after cancel"
        );

        // Verify JobCancelled was broadcast.
        let jce = loop {
            let event = rx.recv().await.expect("received broadcast event");
            match event {
                WsEvent::JobQueued(_) | WsEvent::JobStarted(_) => continue,
                WsEvent::JobCancelled(e) => break e,
                other => panic!("expected JobCancelled, got {:?}", other),
            }
        };
        assert_eq!(jce.job_id, resp.job_id);
        assert_eq!(jce.event, "job.cancelled");

        dispatch_handle.abort();
    }
}
