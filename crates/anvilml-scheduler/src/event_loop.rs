//! Event loop for processing worker events (Completed, Failed, ImageReady).
//!
//! This module implements the event subscription loop that receives
//! `WorkerEvent` messages from the `EventBroadcaster` and updates
//! job status in the database accordingly.
//!
//! **Lifecycle:**
//! 1. The scheduler spawns this loop via `start_event_loop()`.
//! 2. The loop subscribes to the worker event channel.
//! 3. On `Completed`: updates DB (status=completed, completed_at=now),
//!    releases VRAM reservation, broadcasts `WsEvent::JobCompleted`.
//! 4. On `Failed`: updates DB (status=failed, error), releases VRAM,
//!    broadcasts `WsEvent::JobFailed`.
//! 5. On `ImageReady`: decodes base64 image, persists via `ArtifactStore`,
//!    broadcasts `WsEvent::JobImageReady`.
//! 6. On unknown events: logs at DEBUG and continues.
//! 7. On channel closure: logs at WARN and exits.

use std::sync::Arc;

use anvilml_core::types::WsEvent;
use anvilml_ipc::{ArtifactStore, EventBroadcaster, WorkerEvent};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;

use crate::ledger::VramLedger;
use crate::scheduler::JobScheduler;

/// VRAM release amount used by the event loop.
///
/// Matches the hardcoded 4096 MiB default used by the dispatch loop
/// for VRAM reservation. Phase 015 will replace both with model-specific
/// metadata. This constant ensures consistency between reservation and
/// release amounts.
const VRAM_RELEASE_MIB: u32 = 4096;

/// Start the event subscription loop background task.
///
/// Spawns a tokio task that receives `WorkerEvent` messages from the
/// broadcaster's worker event channel and processes them:
/// - `Completed` → update DB, release VRAM, broadcast WsEvent::JobCompleted
/// - `Failed` → update DB with error, release VRAM, broadcast WsEvent::JobFailed
/// - Unknown → log at DEBUG and continue
///
/// The caller must store the returned `JoinHandle` and await it on
/// shutdown to prevent the task from running indefinitely. Dropping
/// the handle without awaiting detaches the task.
///
/// # Arguments
///
/// * `scheduler` — The `JobScheduler` instance providing access to the
///   database pool and internal state.
///
/// # Returns
///
/// A `JoinHandle<()>` for the background event loop task.
pub fn start_event_loop(scheduler: &JobScheduler) -> tokio::task::JoinHandle<()> {
    let db = scheduler.db();
    let ledger = Arc::clone(scheduler.ledger());
    let broadcaster = Arc::clone(scheduler.broadcaster());
    let artifact_store = Arc::clone(scheduler.artifact_store());

    tokio::spawn(async move {
        // Subscribe to the worker event channel.
        // The broadcast::Receiver will deliver all events sent after
        // this subscription point. Events sent before subscription are
        // not delivered, but that's acceptable — no Completed/Failed
        // events can occur before the event loop is started.
        let mut rx = broadcaster.subscribe_worker_events();

        loop {
            // Wait for the next worker event.
            // If the sender is dropped (broadcaster dropped), recv()
            // returns Err(RecvError::Closed) and we exit the loop.
            match rx.recv().await {
                Ok(event) => {
                    handle_event(&db, &ledger, &broadcaster, &artifact_store, event).await;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // The broadcaster was dropped — this should not happen
                    // in normal operation since the broadcaster lives for
                    // the lifetime of the scheduler. Log and exit cleanly.
                    tracing::warn!("worker event channel closed, event loop exiting");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // The event loop fell behind and missed `n` events.
                    // This can happen if the event loop task is blocked on
                    // a slow database write. Skip the missed events and
                    // continue with the latest — the event loop processes
                    // events in order, so skipping is safe for Completed/Failed.
                    tracing::debug!(
                        missed_events = n,
                        "worker event loop lagged, skipping {} events",
                        n
                    );
                }
            }
        }
    })
}

/// Handle a single worker event.
///
/// Dispatches to the appropriate handler based on the event variant:
/// - `Completed` → update DB, release VRAM, broadcast WsEvent
/// - `Failed` → update DB with error, release VRAM, broadcast WsEvent
/// - `ImageReady` → decode base64, persist artifact, broadcast WsEvent
/// - Other variants → log at DEBUG and continue (future phases handle these)
///
/// # Arguments
///
/// * `db` — The SQLite database pool for status updates.
/// * `ledger` — The VRAM ledger for releasing reservations.
/// * `broadcaster` — The event broadcaster for WsEvent notifications.
/// * `artifact_store` — The artifact storage backend for persisting images.
/// * `event` — The worker event to process.
async fn handle_event(
    db: &SqlitePool,
    ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
    broadcaster: &Arc<EventBroadcaster>,
    artifact_store: &Arc<ArtifactStore>,
    event: WorkerEvent,
) {
    // Log every event at DEBUG for observability.
    // This helps diagnose missing events during development.
    tracing::debug!(event_type = ?event, "received worker event");

    match event {
        WorkerEvent::Completed { job_id, elapsed_ms } => {
            handle_completed(db, ledger, broadcaster, job_id, elapsed_ms).await;
        }
        WorkerEvent::Failed {
            job_id,
            error,
            traceback: _,
        } => {
            handle_failed(db, ledger, broadcaster, job_id, error).await;
        }
        WorkerEvent::ImageReady {
            job_id,
            image_b64,
            width,
            height,
            format: _,
            seed,
            steps,
        } => {
            handle_image_ready(
                broadcaster,
                artifact_store,
                job_id,
                &image_b64,
                width,
                height,
                seed,
                steps,
            )
            .await;
        }
        // Unknown event variant — log at DEBUG and continue.
        // Progress, Cancelled, Ready, Pong, Dying, and MemoryReport events
        // are handled by future phases.
        _ => {
            tracing::debug!(event_type = ?event, "ignoring non-terminal worker event");
        }
    }
}

/// Handle a `WorkerEvent::Completed` event.
///
/// 1. Updates the job's status to `completed` and sets `completed_at`
///    to the current UTC time in the database.
/// 2. Queries the job's `device_index` from the database to determine
///    which device to release VRAM on.
/// 3. Releases VRAM reservation via the ledger.
/// 4. Broadcasts `WsEvent::JobCompleted` to WebSocket clients.
/// 5. Emits mandatory INFO log: `job_id`, `elapsed_ms`.
///
/// # Arguments
///
/// * `db` — The SQLite database pool.
/// * `ledger` — The VRAM ledger for releasing reservations.
/// * `broadcaster` — The event broadcaster.
/// * `job_id` — The UUID of the completed job.
/// * `elapsed_ms` — Total wall-clock execution time in milliseconds.
async fn handle_completed(
    db: &SqlitePool,
    ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
    broadcaster: &Arc<EventBroadcaster>,
    job_id: Uuid,
    elapsed_ms: u64,
) {
    // Update the job status to completed and set the timestamp.
    // completed_at is stored as an RFC 3339 string, matching the
    // format used by insert_job for created_at timestamps.
    let completed_at = Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE jobs SET status = 'completed', completed_at = ? WHERE id = ?")
        .bind(completed_at.clone())
        .bind(job_id.to_string())
        .execute(db)
        .await;

    // Derive the device index for VRAM release.
    // The dispatch loop stores worker_id (e.g. "worker-0") and optionally
    // device_index. We try device_index first, then fall back to parsing
    // worker_id. If neither is available, skip VRAM release.
    let device_index: Option<i64> =
        sqlx::query_scalar("SELECT device_index FROM jobs WHERE id = ?")
            .bind(job_id.to_string())
            .fetch_optional(db)
            .await
            .unwrap_or(None);

    let idx = match device_index {
        Some(idx) => Some(idx as u32),
        None => {
            // device_index is NULL — either the column doesn't exist yet
            // or the job hasn't been dispatched. Fall back to parsing
            // worker_id ("worker-N" → N). This keeps the event loop
            // compatible with databases that haven't run migration 002.
            let worker_id: Option<String> =
                sqlx::query_scalar("SELECT worker_id FROM jobs WHERE id = ?")
                    .bind(job_id.to_string())
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);

            worker_id.as_ref().and_then(|wid| {
                // Parse "worker-N" → N. If parsing fails, return None
                // and skip VRAM release.
                wid.strip_prefix("worker-")
                    .and_then(|n| n.parse::<u32>().ok())
            })
        }
    };

    if let Some(idx) = idx {
        // Release the VRAM reservation for this job on the assigned device.
        // The amount (VRAM_RELEASE_MIB) matches the dispatch loop's default
        // reservation amount. Phase 015 will replace this with model-specific
        // metadata. The ledger panics on underflow, catching any mismatch.
        let mut guard = ledger.lock().await;
        guard.release(idx, VRAM_RELEASE_MIB);
    }

    // Broadcast the JobCompleted event to WebSocket clients so they
    // can update their UI and show the completion time.
    broadcaster.send(WsEvent::JobCompleted { job_id, elapsed_ms });

    // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
    // Job completed" with job_id and elapsed_ms fields.
    info!(job_id = %job_id, elapsed_ms = elapsed_ms, "job completed");
}

/// Handle a `WorkerEvent::Failed` event.
///
/// 1. Updates the job's status to `failed` and stores the error
///    message in the database.
/// 2. Queries the job's `device_index` and releases VRAM reservation.
/// 3. Broadcasts `WsEvent::JobFailed` to WebSocket clients.
/// 4. Emits mandatory INFO log: `job_id`, `error`.
///
/// # Arguments
///
/// * `db` — The SQLite database pool.
/// * `ledger` — The VRAM ledger for releasing reservations.
/// * `broadcaster` — The event broadcaster.
/// * `job_id` — The UUID of the failed job.
/// * `error` — The human-readable error message from the worker.
async fn handle_failed(
    db: &SqlitePool,
    ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
    broadcaster: &Arc<EventBroadcaster>,
    job_id: Uuid,
    error: String,
) {
    // Update the job status to failed and store the error message.
    // The error column stores the worker's error string for display
    // to the user. Traceback is intentionally not stored — the error
    // field is the primary diagnostic and traceback would make it unwieldy.
    let _ = sqlx::query("UPDATE jobs SET status = 'failed', error = ? WHERE id = ?")
        .bind(&error)
        .bind(job_id.to_string())
        .execute(db)
        .await;

    // Release VRAM reservation, same logic as Completed handler.
    let device_index: Option<i64> =
        sqlx::query_scalar("SELECT device_index FROM jobs WHERE id = ?")
            .bind(job_id.to_string())
            .fetch_optional(db)
            .await
            .unwrap_or(None);

    let idx = match device_index {
        Some(idx) => Some(idx as u32),
        None => {
            let worker_id: Option<String> =
                sqlx::query_scalar("SELECT worker_id FROM jobs WHERE id = ?")
                    .bind(job_id.to_string())
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);

            worker_id.as_ref().and_then(|wid| {
                wid.strip_prefix("worker-")
                    .and_then(|n| n.parse::<u32>().ok())
            })
        }
    };

    if let Some(idx) = idx {
        let mut guard = ledger.lock().await;
        guard.release(idx, VRAM_RELEASE_MIB);
    }

    // Broadcast the JobFailed event so clients can display the error
    // and disable retry controls if appropriate.
    broadcaster.send(WsEvent::JobFailed {
        job_id,
        error: error.clone(),
    });

    // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
    // Job failed" with job_id and error fields.
    info!(job_id = %job_id, error = %error, "job failed");
}

/// Handle a `WorkerEvent::ImageReady` event.
///
/// 1. Decodes the base64-encoded image payload.
/// 2. Persists the image via `ArtifactStore::save()`.
/// 3. Broadcasts `WsEvent::JobImageReady` with the artifact hash and
///    image dimensions.
/// 4. Emits mandatory INFO log: `job_id`, `artifact_hash`, `size_bytes`.
///
/// On decode failure, logs at WARN and returns early without updating
/// the job state — the job may still complete via the Completed event.
///
/// # Arguments
///
/// * `broadcaster` — The event broadcaster for WsEvent notifications.
/// * `artifact_store` — The artifact storage backend for persisting images.
/// * `job_id` — The UUID of the job that produced the image.
/// * `image_b64` — The base64-encoded image payload from the worker.
/// * `width` — Image width in pixels.
/// * `height` — Image height in pixels.
/// * `seed` — Random seed used for generation.
/// * `steps` — Number of steps executed to produce the image.
#[expect(clippy::too_many_arguments, reason = "event handler parameters")]
async fn handle_image_ready(
    broadcaster: &Arc<EventBroadcaster>,
    artifact_store: &Arc<ArtifactStore>,
    job_id: Uuid,
    image_b64: &str,
    width: u32,
    height: u32,
    seed: i64,
    steps: u32,
) {
    // Decode the base64-encoded image payload. The worker sends the image
    // as a base64 string, but ArtifactStore::save() expects raw bytes.
    // If decoding fails, log at WARN and return early — the job may still
    // complete via the Completed event, and the image is simply not saved.
    let image_bytes: Vec<u8> = match STANDARD.decode(image_b64) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(
                job_id = %job_id,
                error = %e,
                "failed to decode ImageReady base64 payload, skipping artifact save"
            );
            return;
        }
    };

    // Persist the decoded image bytes to disk and record metadata in
    // the database. The save method computes a SHA-256 hash and uses
    // content-addressed storage for deduplication.
    let meta = match artifact_store.save(job_id, &image_bytes).await {
        Ok(meta) => meta,
        Err(e) => {
            tracing::warn!(
                job_id = %job_id,
                error = %e,
                "failed to save artifact, skipping broadcast"
            );
            return;
        }
    };

    // Broadcast the JobImageReady event so connected WebSocket clients
    // can display the generated image preview and access the artifact.
    // Clone the hash before the broadcast — we need it for the INFO log
    // below, and the broadcast consumes it via the WsEvent struct.
    let artifact_hash = meta.hash.clone();
    broadcaster.send(WsEvent::JobImageReady {
        job_id,
        artifact_hash,
        width,
        height,
        seed,
        steps,
    });

    // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
    // Job completed" maps to artifact save completion for image jobs.
    // We log the artifact hash and size so operators can track artifact
    // production rates and file sizes.
    info!(
        job_id = %job_id,
        artifact_hash = %meta.hash,
        size_bytes = meta.size_bytes,
        "artifact saved"
    );
}
