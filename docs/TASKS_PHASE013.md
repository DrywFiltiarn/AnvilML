# Tasks: Phase 013 — Job Queue & Persistence

| Field | Value |
|-------|-------|
| Phase | 013 |
| Name | Job Queue & Persistence |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 12 |

## Overview

Phase 013 replaces the Phase 012 placeholder job submission with real persistence and a FIFO queue. At phase start `POST /v1/jobs` validates the graph and returns a placeholder UUID that is never stored. At phase end submitted jobs are persisted to SQLite, assigned a queue position, broadcast as `WsEvent::JobQueued`, and retrievable via `GET /v1/jobs` and `GET /v1/jobs/:id`.

The `JobScheduler` introduced here is the central object that Phases 014–017 extend. It owns the queue, the VRAM ledger, the node registry reference, the database pool, and the event broadcaster. Getting the scheduler's data structures right in this phase keeps each subsequent phase's changes bounded to a single concern.

The dispatch loop (Phase 014) is not wired in this phase — jobs enter the queue with status `Queued` and stay there until Phase 014.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-scheduler | P13-A1 … P13-A3 | JobQueue, VramLedger, JobScheduler with SQLite persistence |
| B | anvilml-server | P13-B1 | POST /v1/jobs, GET /v1/jobs, GET /v1/jobs/:id wired to scheduler |

## Prerequisites

Phase 012 complete. `validate_graph` and `ValidatedGraph` exist in `anvilml-scheduler`. `AnvilError::InvalidGraph` exists. `AppState` carries `Arc<NodeTypeRegistry>`. `EventBroadcaster` exists. The SQLite pool is initialised in `main.rs`. Job and `WsEvent` domain types are defined in `anvilml-core`.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §5.4` | P13-A3 | `Job` struct fields: `id`, `status`, `graph`, `created_at`, `started_at`, `completed_at`, `error` |
| `ANVILML_DESIGN.md §5.8` | P13-A3 | `WsEvent::JobQueued` variant and fields |
| `ANVILML_DESIGN.md §12.4` | P13-B1 | Response shapes for POST /v1/jobs, GET /v1/jobs, GET /v1/jobs/:id |

## Task Descriptions

### Group A — anvilml-scheduler

#### P13-A1: anvilml-scheduler: queue.rs JobQueue FIFO with O(1) cancel

**Goal:** Create the `JobQueue` data structure in `crates/anvilml-scheduler/src/queue.rs`. The queue is FIFO for dispatch order; cancel must be O(1) rather than O(n) scan to avoid latency spikes when the queue is large.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/queue.rs` — new file; `JobQueue` struct and all methods
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod queue`

**Key implementation notes:**
- `JobQueue { items: VecDeque<Job>, by_id: HashMap<Uuid, usize> }` — `by_id` maintains index into `items`
- Public API: `push(&mut self, job: Job)`, `pop_front(&mut self) -> Option<Job>`, `cancel(&mut self, id: Uuid) -> bool`, `get(&self, id: &Uuid) -> Option<&Job>`, `list(&self) -> Vec<&Job>`, `len(&self) -> usize`
- `cancel` removes from `items` (swap-remove or drain) and `by_id`; updates any displaced indices
- Pure logic — no async, no I/O, no database

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware -- queue` exits 0 with ≥ 6 tests (push+pop FIFO order; cancel present; cancel absent returns false; get; list; len after operations).

---

#### P13-A2: anvilml-scheduler: ledger.rs VramLedger per-device reservation

**Goal:** Create `VramLedger` in `crates/anvilml-scheduler/src/ledger.rs` — a per-device VRAM reservation tracker used by the dispatch loop to avoid over-scheduling a device. Pure logic, no async.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/ledger.rs` — new file; `VramLedger` struct and all methods
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod ledger`

**Key implementation notes:**
- `VramLedger { reservations: HashMap<u32, u32>, totals: HashMap<u32, u32> }` — keyed by device index
- Public API: `register_device(&mut self, index: u32, vram_total_mib: u32)`, `would_fit(&self, index: u32, requested_mib: u32) -> bool`, `reserve(&mut self, index: u32, mib: u32)`, `release(&mut self, index: u32, mib: u32)`
- `would_fit` returns false if device index is unknown
- `tracing::debug!(device_index, reserved_mib, free_after_mib, "vram reserved")` in `reserve`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 with ≥ 5 tests (register + would_fit true/false; reserve reduces free; release restores free; unknown device returns false).

---

#### P13-A3: anvilml-scheduler: scheduler.rs JobScheduler submit and persistence

**Goal:** Implement `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs`. The scheduler owns the queue and ledger, persists jobs to SQLite, and broadcasts events. `submit` validates the graph (delegating to `validate_graph`), inserts the job, enqueues it, and notifies the dispatch loop. The dispatch loop itself is added in Phase 014; this task only wires the data path up to the notification.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — new file; `JobScheduler` struct and all methods
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod scheduler; pub use scheduler::JobScheduler`

**Key implementation notes:**
- `JobScheduler { queue: Arc<Mutex<JobQueue>>, ledger: Arc<Mutex<VramLedger>>, node_registry: Arc<NodeTypeRegistry>, db: SqlitePool, broadcaster: Arc<EventBroadcaster> }`
- `pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse>`: call `validate_graph`; create `Job { status: Queued, ... }`; `INSERT` to SQLite `jobs` table; `push` to queue; `broadcast(WsEvent::JobQueued)`; `notify` dispatch Notify (no-op until dispatch loop exists)
- `pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>>` — queries SQLite
- `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>, before: Option<DateTime>) -> Result<Vec<Job>>`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with ≥ 5 tests (submit valid graph → job persisted with Queued status; submit invalid graph → error; get_job returns job; list_jobs returns all; list_jobs filtered by status).

---

### Group B — anvilml-server

#### P13-B1: anvilml-server: POST /v1/jobs, GET /v1/jobs, GET /v1/jobs/:id wired to scheduler

**Goal:** Replace the Phase 012 placeholder handlers with real implementations that delegate to `JobScheduler`. After this task, submitted jobs are persisted and queryable.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — update `submit_job`; add `list_jobs` and `get_job`
- `crates/anvilml-server/src/state.rs` — add `scheduler: Arc<JobScheduler>` to `AppState`
- `crates/anvilml-server/src/lib.rs` — mount `GET /v1/jobs`, `GET /v1/jobs/:id`; init scheduler in `main.rs`

**Key implementation notes:**
- `submit_job` now calls `scheduler.submit(req).await` and returns the real `job_id` and `queue_position`
- `list_jobs` accepts `Query<{ status: Option<String>, limit: Option<u32>, before: Option<String> }>`; delegates to `scheduler.list_jobs()`
- `get_job` returns 200 with `Job` JSON or 404 via `AnvilError::JobNotFound`
- Integration tests: POST valid graph → 202 `{ job_id, queue_position: 0 }`; GET `/v1/jobs/:id` → 200 `{ status: "queued" }`

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; tests verify 202 submit response contains real UUID, and GET returns Queued status.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Runnable Proof (manual): submit a job and retrieve it as a real persisted record
cargo run --features mock-hardware &
sleep 5
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='Queued'"
# -> 200 {"status":"Queued",...} (job persisted in SQLite, retrievable by id)
kill %1
```

## Known Constraints and Gotchas

- `Arc<Mutex<JobQueue>>` must use `tokio::sync::Mutex` (not `std::sync::Mutex`) because it will be held across `await` points in Phase 014's dispatch loop.
- The SQLite `jobs` table schema must match `ANVILML_DESIGN.md §5.4` exactly — column names are referenced by name in later phases and in the OpenAPI spec.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every `pub` item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
