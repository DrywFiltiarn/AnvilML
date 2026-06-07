# Plan Report: P12-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A3                                      |
| Phase       | 012 — Job Submission & Queue                |
| Description | anvilml-scheduler: JobScheduler::submit (validate, persist, enqueue, notify) |
| Depends on  | P12-A1, P12-A2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-07T15:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-scheduler/src/scheduler.rs` implementing `JobScheduler::submit`, which validates a job graph, persists the job to SQLite as Queued, enqueues it in the in-memory queue, broadcasts a `job.queued` WebSocket event, and notifies a waiting dispatcher loop. This is the core orchestration step that bridges HTTP submission to the scheduler's internal state.

## Scope

### In Scope
- Create `src/scheduler.rs` with `JobScheduler` struct and `async fn submit`
- Export `scheduler` module from `lib.rs` (add `pub mod scheduler;` and re-export)
- Add `tokio` dev-dependency to `anvilml-scheduler/Cargo.toml` (for `Notify` in tests)
- Write unit tests for `submit`: valid job persists + enqueues + broadcasts; invalid graph returns `InvalidGraph`; database error propagates
- Bump `anvilml-scheduler` patch version from `0.1.6` to `0.1.7` per FORGE_AGENT_RULES §12

### Out of Scope
- The HTTP handler wiring (POST /v1/jobs) — handled by P12-A4
- Job dispatch loop — handled by P13 tasks
- Worker assignment logic — handled by P13 tasks
- GET /v1/jobs and GET /v1/jobs/:id endpoints — handled by P12-A4, P12-A5
- Any changes to `anvilml-server` crate

## Approach

### Step 1: Add tokio dev-dependency

Add `tokio = { workspace = true, features = ["sync"] }` to `crates/anvilml-scheduler/Cargo.toml` under `[dev-dependencies]`. This is needed for the `Notify` type used in tests. The `workspace` dependency already has `"full"` features so this is safe.

### Step 2: Create `src/scheduler.rs`

Implement the following:

```rust
//! JobScheduler — central orchestrator for job submission and dispatch coordination.
//!
//! Wraps the in-memory queue, database pool, event broadcaster, and a Notify handle
//! used by the (future) dispatch loop to wake on new submissions.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::events::{JobQueuedEvent, WsEvent};
use anvilml_core::types::job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
use chrono::Utc;
use tokio::sync::broadcast;
use tokio::sync::Notify;
use tracing;
use uuid::Uuid;

use crate::dag::validate_graph;
use crate::job_store::{insert_job, update_status};
use crate::queue::JobQueue;

/// Central job scheduler.
///
/// Holds the in-memory queue, database pool, a broadcast sender for WebSocket events,
/// and a `Notify` handle that the dispatch loop waits on.
pub struct JobScheduler {
    /// In-memory FIFO queue of jobs awaiting dispatch.
    queue: JobQueue,
    /// List of available workers (read-only snapshot; populated by server).
    workers: Arc<Vec<anvilml_core::types::worker::WorkerInfo>>,
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
            workers,
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
        validate_graph(&req.graph).map_err(|errors| {
            AnvilError::InvalidGraph(errors.join("; "))
        })?;

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
        insert_job(&self.db, &job).await.map_err(|e| {
            AnvilError::DbError(format!("failed to insert job: {e}"))
        })?;

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
```

**Key design decisions:**
- `broadcaster` is `broadcast::Sender<WsEvent>` rather than an `EventBroadcaster` wrapper, because `EventBroadcaster` lives in `anvilml-server` and `anvilml-scheduler` must not depend on it (dependency graph rule). The server passes the sender when constructing `JobScheduler`.
- `workers` is `Arc<Vec<WorkerInfo>>` — a read-only snapshot passed from the server. Actual worker assignment logic belongs in phase 13.
- `submit` uses `tracing::instrument` per FORGE_AGENT_RULES §11.6 and logs at INFO per ENVIRONMENT.md §9 (mandatory job scheduler log point: "job submitted").
- Error handling: `validate_graph` → `AnvilError::InvalidGraph`; `insert_job` → `AnvilError::DbError`.

### Step 3: Update `lib.rs`

Add `pub mod scheduler;` and re-export `JobScheduler`:

```rust
pub mod dag;
pub mod job_store;
pub mod nodes;
pub mod queue;
pub mod scheduler;

pub use dag::{validate_graph, ValidatedGraph};
pub use job_store::*;
pub use nodes::{KNOWN_NODE_TYPES, NODE_SLOTS};
pub use queue::JobQueue;
pub use scheduler::JobScheduler;
```

### Step 4: Write tests in `src/scheduler.rs` (`#[cfg(test)]` module)

Tests run with `--features mock-hardware`:

1. **`test_submit_valid_job`** — Create a `JobScheduler` with an in-memory SQLite pool (with jobs table created), a fresh `JobQueue`, and a broadcast channel. Call `submit` with a valid ZiT graph. Assert:
   - Returns `Ok(SubmitJobResponse)` with non-empty `job_id`
   - `queue_position >= 1`
   - The job exists in the database (query via `get_job`)
   - The job status is `Queued`
   - The queue length increased

2. **`test_submit_invalid_graph`** — Submit a graph with an unknown node type. Assert:
   - Returns `Err(AnvilError::InvalidGraph(_))`
   - The job was NOT persisted (no row in DB for the generated UUID)
   - The queue length is unchanged

3. **`test_submit_broadcasts_event`** — Create a scheduler with a broadcast channel that has a subscriber. Call `submit` with a valid graph. Assert:
   - Returns `Ok`
   - A `WsEvent::JobQueued` event was received on the subscriber channel
   - The event's `job_id` matches the response

4. **`test_submit_persists_settings`** — Submit a job with custom settings (non-default seed, steps, etc.). Assert:
   - Returns `Ok`
   - The persisted job's settings match the submitted values exactly

### Step 5: Bump crate version

Update `crates/anvilml-scheduler/Cargo.toml`:
```toml
version = "0.1.7"
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-scheduler/src/scheduler.rs` | JobScheduler struct + submit method + tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod scheduler;` and re-export |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version 0.1.6→0.1.7, add tokio dev-dep |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `scheduler.rs` (mod tests) | `test_submit_valid_job` | Valid job persists as Queued + enqueues + returns response with job_id and queue_position |
| `scheduler.rs` (mod tests) | `test_submit_invalid_graph` | Invalid graph (unknown node type) returns AnvilError::InvalidGraph, no DB row, no enqueue |
| `scheduler.rs` (mod tests) | `test_submit_broadcasts_event` | WsEvent::JobQueued is sent on the broadcast channel with matching job_id |
| `scheduler.rs` (mod tests) | `test_submit_persists_settings` | Custom JobSettings (seed, steps, guidance_scale, width, height) round-trip through submit → DB |

## CI Impact

No CI workflow files are modified. The task only adds source and test files within the existing `anvilml-scheduler` crate. The standard CI gates apply:
- `cargo fmt --all -- --check` — formatting check
- `cargo clippy --workspace --features mock-hardware -- -D warnings` — linting
- `cargo test --workspace --features mock-hardware` — full test suite (must exit 0)
- Platform cross-checks (mock-hardware Linux + Windows cross, real-hardware Linux + Windows cross)

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::broadcast::Sender` is not compatible with test setup (no runtime) | Medium | Tests fail to compile or panic | Use `#[tokio::test]` for all async tests; broadcast sender works synchronously (send is non-blocking), so sync tests can also use it |
| `tracing` crate not yet in anvilml-scheduler dependencies | High | Compilation fails on tracing macros | Add `tracing = { workspace = true }` to `[dependencies]` in Cargo.toml |
| `anvilml_core::types::worker::WorkerInfo` import creates unused variable warning with clippy `-D warnings` | Medium | Clippy failure | Use `_workers: Arc<Vec<...>>` pattern or derive the field for future use; suppress with `#[allow(dead_code)]` only if truly not used in this task's scope. Better: keep as-is since it will be used in phase 13 |
| Broadcast channel subscriber test requires tokio runtime | Low | Test compilation issue | Use `#[tokio::test]` attribute; the workspace `tokio` dev-dep already has `"rt-multi-thread"` feature |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 (all submit tests pass)
- [ ] Valid job submitted via `submit()` is persisted to SQLite with status `Queued`
- [ ] `WsEvent::JobQueued` event is broadcast on the channel
- [ ] Invalid graph returns `AnvilError::InvalidGraph`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `anvilml-scheduler` version bumped to `0.1.7` in Cargo.toml
