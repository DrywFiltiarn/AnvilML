//! INTERIM-P14-PATCH — manual retrofit, applied ahead of Phase 16.
//!
//! `docs/PHASES_GRAPH.md`'s "Deep Trace Findings" section documents that no
//! task anywhere consumes a terminal `WorkerEvent` (`Completed`/`Failed`) to
//! update a job's row in `JobStore` — that responsibility was scoped to
//! `P16-A1`/`P16-A2` (the real `EventBroadcaster`-subscribing event loop),
//! a phase that has not executed yet. Phase 14's own Runnable Proof
//! (`P14-E1`) requires a submitted job to observably reach `Completed` via
//! `GET /v1/jobs/:id` — which is impossible without *some* consumer of
//! that event existing first.
//!
//! This module is that consumer, deliberately minimal: it receives
//! `(job_id, status, error)` tuples over an unbounded channel (populated by
//! `anvilml-worker`'s own interim patch — see `ManagedWorker::handle_event()`
//! in `crates/anvilml-worker/src/managed.rs`) and writes the resulting
//! status directly to `JobStore`.
//!
//! # This module must not be extended
//!
//! When Phase 16 executes, `P16-A1`/`P16-A2` must:
//! 1. Delete this file and its `pub mod` declaration in `lib.rs`.
//! 2. Delete `ManagedWorker`/`WorkerPool`'s `job_completion_tx` fields and
//!    setters (`crates/anvilml-worker/src/managed.rs`, `pool.rs`).
//! 3. Delete the wiring in `backend/src/main.rs` that constructs the
//!    channel and calls `spawn_interim_job_completion_listener()`.
//! 4. Replace all of the above with the real design: `AppState` gains a
//!    `broadcaster: EventBroadcaster` field (`P16-B1`), and the scheduler
//!    subscribes to it directly — no unbounded mpsc channel, no
//!    worker-crate-level coupling to job status at all.
//!
//! This is a stopgap, not a smaller version of the real feature — replace
//! it wholesale, don't merge into it.

use std::sync::Arc;

use anvilml_core::JobStatus;
use anvilml_registry::JobStore;
use chrono::Utc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Spawn the interim completion listener as a background task.
///
/// Loops on `rx`, and for each `(job_id, status, error)` tuple:
/// 1. Fetches the current `Job` row via `job_store.get()`.
/// 2. If found, sets `status`, `completed_at = Some(Utc::now())`, and
///    `error` (only meaningful for `JobStatus::Failed`), then persists via
///    `job_store.upsert()`.
/// 3. If the job is not found (should not happen in practice — the job was
///    persisted as `Running` by `dispatch_one()` before `Execute` was ever
///    sent), logs a warning and continues; never panics.
///
/// Runs until `rx`'s sender side (`ManagedWorker`/`WorkerPool`'s clones of
/// the channel constructed in `backend/main.rs`) is fully dropped, i.e. for
/// the lifetime of the process under normal operation.
pub fn spawn_interim_job_completion_listener(
    job_store: Arc<JobStore>,
    mut rx: mpsc::UnboundedReceiver<(Uuid, JobStatus, Option<String>)>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("interim_job_completion: listener started");
        while let Some((job_id, status, error)) = rx.recv().await {
            match job_store.get(job_id).await {
                Ok(Some(mut job)) => {
                    job.status = status;
                    job.completed_at = Some(Utc::now());
                    job.error = error;
                    if let Err(e) = job_store.upsert(&job).await {
                        tracing::error!(
                            job_id = %job_id,
                            error = %e,
                            "interim_job_completion: failed to persist terminal status"
                        );
                    } else {
                        tracing::debug!(
                            job_id = %job_id,
                            status = ?status,
                            "interim_job_completion: persisted terminal status"
                        );
                    }
                }
                Ok(None) => {
                    tracing::warn!(
                        job_id = %job_id,
                        "interim_job_completion: received terminal event for unknown job_id"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        job_id = %job_id,
                        error = %e,
                        "interim_job_completion: job_store.get() failed"
                    );
                }
            }
        }
        tracing::info!("interim_job_completion: listener exiting (channel closed)");
    })
}
