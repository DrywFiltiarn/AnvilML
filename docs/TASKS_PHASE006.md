# Tasks: Phase 006 — Scheduler

| Field            | Value                                                                       |
|------------------|-----------------------------------------------------------------------------|
| Phase            | 006                                                                         |
| Name             | Scheduler                                                                   |
| ANVIL Milestone  | M3                                                                          |
| Status           | Draft                                                                       |
| Depends on phases| 1, 2, 3, 4, 5                                                               |
| Task file        | `forge/tasks/tasks_phase006.json`                                           |
| Design reference | `ANVILML_DESIGN.md` §9 (Scheduler), §4.1 (Job Types)                       |

---

## Overview

Phase 006 implements `anvilml-scheduler`: the VRAM ledger, DAG validation engine, in-memory job queue, GPU selection algorithm, cancellation logic, and the background dispatch loop that feeds work to the `WorkerPool`. This phase completes M3.

The scheduler is the coordination centre of the system. Every job submission, every dispatch decision, every cancellation, and every status transition flows through it. The two most complex pieces are the DAG validator and the dispatch loop.

The DAG validator is a deliberately strict gate: unknown node types are rejected at submission time, not at execution time. This means the Rust `KNOWN_NODE_TYPES` constant must stay in sync with the Python `NODE_REGISTRY`. A parity test (phase 009) enforces this automatically; the constant is defined here for the first time.

The dispatch loop runs as a background `tokio` task and wakes on two conditions: a new job arriving in the queue (via `Notify`) and a worker transitioning to `Idle` (via the event broadcast). This two-trigger design means dispatch is responsive to both events without polling, and a newly idle worker is offered queued work immediately.

At the end of this phase: `cargo test -p anvilml-scheduler --features mock-hardware` passes with tests covering VRAM ledger arithmetic, all four DAG error classes, queue operations, all three `select_worker` modes, and a dispatch-to-Execute integration test using the real mock worker.

---

## Group Reference

| Group | Subsystem            | Tasks          | Summary                                                        |
|-------|----------------------|----------------|----------------------------------------------------------------|
| A     | anvilml-scheduler    | P6-A1 … P6-A4  | VramLedger, DAG engine, JobQueue+select_worker, dispatch loop  |

---

## Prerequisites

- P5-A3 complete: `WorkerPool` with `acquire_idle`, `set_busy`, `set_idle`, `subscribe_events`, and `send` are all implemented and tested.
- `anvilml-core` domain types (`Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse`, `ValidatedGraph`) are stable.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                     |
|---------------------------|----------------|---------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §9.2  | P6-A1          | `VramLedger` exact struct and method signatures                     |
| `ANVILML_DESIGN.md` §9.5  | P6-A2          | All four validation checks; `KNOWN_NODE_TYPES` set; `ValidatedGraph` newtype |
| `ANVILML_DESIGN.md` §14.6 | P6-A2          | Node slot tables used in edge-reference validation                  |
| `ANVILML_DESIGN.md` §9.1  | P6-A3          | Queue is a `VecDeque`; cancel marks in-place; not persisted         |
| `ANVILML_DESIGN.md` §9.3  | P6-A3          | `select_worker` three-mode algorithm exactly as specified           |
| `ANVILML_DESIGN.md` §9.4  | P6-A4          | Cancellation HTTP status codes and IPC path                         |
| `ANVILML_DESIGN.md` §9.6  | P6-A4          | Dispatch loop wakes on Notify + Idle; lock held across `.await`     |

---

## Task Descriptions

### Group A — anvilml-scheduler

#### P6-A1: anvilml-scheduler — VramLedger

**Goal:** Implement the lightweight in-memory per-device VRAM tracker that the dispatch algorithm uses to rank workers by available VRAM.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/ledger.rs` — `VramLedger` struct
- `crates/anvilml-scheduler/src/lib.rs` — expose `ledger::VramLedger`
- `crates/anvilml-scheduler/Cargo.toml` — add `anvilml-core` path dep

**Key implementation notes:**
- `VramLedger` is a plain struct wrapping `HashMap<u32, (u32 /*total_mib*/, u32 /*used_mib*/)>`. It is not wrapped in a lock here; the `JobScheduler` owns it inside a `tokio::sync::Mutex` along with the queue.
- `update(device_index, used_mib, total_mib)`: upsert the tuple.
- `free_mib(device_index) -> u32`: `total - used`; if device unknown, return 0.
- `would_fit(device_index, required_mib) -> bool`: `free_mib(device_index) >= required_mib`. Advisory only; never used as a hard gate.
- Initialize from `HardwareInfo.gpus`: `update(gpu.index, 0, gpu.vram_total_mib)` for each GPU.
- Write tests: init from hardware, `free_mib` returns correct value, `update` changes free, `would_fit` returns true/false correctly, unknown device returns 0 for `free_mib`.

**Acceptance criterion:** `cargo test -p anvilml-scheduler -- ledger` exits 0.

---

#### P6-A2: anvilml-scheduler — DAG validation engine

**Goal:** Implement the graph validation function that gates every job submission, rejecting graphs with cycles, unknown node types, bad edge references, or duplicate node IDs — collecting all errors rather than failing on the first.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — `validate_graph`, `KNOWN_NODE_TYPES`, `ValidatedGraph`
- `crates/anvilml-scheduler/src/lib.rs` — expose these

**Key implementation notes:**
- `KNOWN_NODE_TYPES`: a `HashSet<&'static str>` (or `phf::Set`) initialized as a `static` or `const` with the nine MVP node names: `ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`, `SdxlLoadPipeline`, `SdxlTextEncode`, `SdxlSampler`, `SdxlDecode`, `SaveImage`.
- `NODE_SLOTS`: a `HashMap<&'static str, (&[&str] /* inputs */, &[&str] /* outputs */)>` matching the slot tables in `ANVILML_DESIGN.md §14.6`. This is used in check 2 (edge reference validation).
- `validate_graph(value: &serde_json::Value) -> Result<ValidatedGraph, Vec<String>>`: run all four checks, collect errors, return `Err(errors)` if non-empty, else `Ok(ValidatedGraph(value.clone()))`.
  1. **Duplicate node id**: collect all `nodes[].id` strings; if any appear more than once, add `"duplicate_node_id: {id}"` per duplicate.
  2. **Edge reference validity**: for every input whose value is an object with `node_id` and `output_slot`, check that `node_id` exists in the node map and that the referenced node's type declares `output_slot` in `NODE_SLOTS`. Errors: `"unknown_node_ref: {node_id}"`, `"unknown_output_slot: {node_id}.{slot}"`.
  3. **Acyclicity** (Kahn's algorithm): build an adjacency list from edge references. Kahn's: count in-degrees; queue all zero-in-degree nodes; pop, decrement neighbours, re-queue if zero. If the processed count < total nodes, a cycle exists; collect the unprocessed node IDs and add `"cycle_detected: {ids}"`.
  4. **Unknown node types**: for every `nodes[].type`, if not in `KNOWN_NODE_TYPES`, add `"unknown_node_type: {type}"`.
- `ValidatedGraph` is a newtype `pub struct ValidatedGraph(pub serde_json::Value)`. Only code with access to `dag.rs` can construct one (the field is pub for reads, but the constructor is via `validate_graph` only).
- Write tests: a well-formed two-node ZiT graph succeeds; cycle of two nodes returns `cycle_detected`; edge referencing non-existent node returns `unknown_node_ref`; unknown node type returns `unknown_node_type`; duplicate IDs return `duplicate_node_id`. Errors for a multi-error graph are all present in the returned `Vec`.

**Acceptance criterion:** `cargo test -p anvilml-scheduler -- dag` exits 0 with ≥5 test cases.

---

#### P6-A3: anvilml-scheduler — JobQueue and select_worker

**Goal:** Implement the in-memory job queue and the GPU selection algorithm used by the dispatch loop.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/queue.rs` — `JobQueue` and `select_worker`

**Key implementation notes:**
- `JobQueue` wraps `VecDeque<Job>` behind a `tokio::sync::Mutex`. It is not public by itself; `JobScheduler` owns it.
- `enqueue(&mut self, job: Job)`: push to back.
- `cancel_queued(&mut self, id: Uuid) -> bool`: find the job with matching `id` and status `Queued`; set its status to `Cancelled` in place (do not remove — removal happens on the next dispatch pass via `pop_next`). Return `true` if found and cancelled.
- `pop_next(&mut self) -> Option<Job>`: iterate front-to-back, remove and return the first job with status `Queued`. Skip (remove) any job with status `Cancelled`.
- `select_worker(job: &Job, workers: &[WorkerInfo], ledger: &VramLedger) -> Option<usize /*worker index*/>`:
  1. If `job.settings.device_preference = Some(n)`: return the index of the worker with `device_index == n` only if its status is `Idle`. If `Busy`, return `None` (hold in queue). If the device index does not appear in workers at all, this should have been rejected at submission — treat as `None`.
  2. Else (`device_preference = None`): collect all workers whose status is `Idle`; rank by `ledger.free_mib(worker.device_index)` descending, ties broken by `device_index` ascending; return index of the top candidate, or `None` if empty.
  3. Force-CPU (`gpu_selection.default_device = "cpu"` in config): only include the worker with `DeviceType::Cpu` in the candidate set.
- Write tests: enqueue+pop; cancel-then-pop skips cancelled; `select_worker` with single idle worker; `select_worker` with busy preferred device returns None; `select_worker` auto-ranks by VRAM.

**Acceptance criterion:** `cargo test -p anvilml-scheduler -- queue` exits 0.

---

#### P6-A4: anvilml-scheduler — dispatch loop and JobScheduler public API

**Goal:** Assemble the public `JobScheduler` struct and the background dispatch task that ties together the queue, ledger, worker pool, and DB.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — `JobScheduler`
- `crates/anvilml-scheduler/src/lib.rs` — `pub use scheduler::JobScheduler`
- `crates/anvilml-scheduler/Cargo.toml` — add `sqlx`, `tokio`, `uuid`, `anvilml-worker` path dep

**Key implementation notes:**
- `JobScheduler { queue: Arc<Mutex<JobQueue>>, ledger: Arc<Mutex<VramLedger>>, workers: Arc<WorkerPool>, db: SqlitePool, broadcaster: Arc<EventBroadcaster>, notify: Arc<Notify> }`.
- `submit(req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError>`: call `validate_graph`, return `Err(AnvilError::InvalidGraph(errors.join("; ")))` on failure with HTTP 422. On success: build `Job { id: Uuid::new_v4(), status: Queued, ... }`, `INSERT INTO jobs` (all fields), `enqueue(job)`, `notify.notify_one()`. Return `SubmitJobResponse { job_id, queue_position }` where `queue_position` is the current queue length.
- `cancel(id: Uuid) -> Result<(), AnvilError>`:
  - Read the job from DB. If not found → `Err(JobNotFound)`.
  - If terminal (`Completed/Failed/Cancelled`) → `Err` (409 `job_not_cancellable`).
  - If `Queued`: `cancel_queued(id)`, `UPDATE jobs SET status='Cancelled' WHERE id=?`, `broadcaster.send(WsEvent::JobCancelled(...))`.
  - If `Running`: `workers.send(worker_id, WorkerMessage::CancelJob { job_id: id })`, `UPDATE jobs SET status='Cancelled'`, broadcast. The worker's `Cancelled` event will be received by the dispatch loop and triggers the final worker→Idle transition.
- `start_dispatch_loop() -> JoinHandle<()>`: background task. Wakes when `notify` fires OR when a `WorkerEvent::Ready` / status-change event arrives on `workers.subscribe_events()`. Per wake: lock queue + ledger, call `pop_next()`, call `select_worker`, if a match: `UPDATE jobs SET status='Running', started_at=now, worker_id=?, device_index=? WHERE id=?`, `workers.set_busy(...)`, `broadcaster.send(WsEvent::JobStarted(...))`, `workers.send(..., Execute { ... })`. Release lock. Repeat until no match. The lock is `tokio::sync::Mutex` because it is held across async `.await` points.
- Handle incoming `WorkerEvent`s in the dispatch loop: `ImageReady` → call `artifact_store.save(...)`, update `jobs.artifact_count`, broadcast `JobImageReady`. `Completed` → set job `Completed`, `workers.set_idle`, broadcast, `notify.notify_one()` to trigger next dispatch. `Failed/Cancelled` → set respective terminal status, `workers.set_idle`, broadcast, notify.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with integration test: submit a job → dispatch loop sends `Execute` to the mock worker.

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-scheduler --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings
```

---

## Known Constraints and Gotchas

- The dispatch loop holds a `tokio::sync::Mutex` across `.await` points intentionally. `std::sync::Mutex` cannot be held across awaits without risk of deadlock and the design explicitly requires it (§9.6). This is the correct tool here.
- `Notify::notify_one()` stores a single permit. If two jobs are submitted in rapid succession, only one notification may fire. The dispatch loop must loop until `pop_next()` returns `None` in a single wake — do not break out of the inner loop after the first dispatch.
- The `cancel` path for a `Running` job sends the IPC message and immediately writes `status='Cancelled'` to the DB. This means the job is terminally cancelled in the DB before the worker has acknowledged it. If the worker then sends `Completed` instead of `Cancelled` (it may have finished a microsecond before the cancel arrived), the dispatch loop must ignore the stale `Completed` event for a job already in terminal status. Always re-read job status from the DB before applying a `WorkerEvent` transition.
- `KNOWN_NODE_TYPES` is defined in `dag.rs` and is the authoritative Rust-side list. The parity test (P9-B2) will compare it to the Python `NODE_REGISTRY`. Do not split the constant across files.
