# Tasks: Phase 014 — Dispatch & Mock Execute

| Field | Value |
|-------|-------|
| Phase | 014 |
| Name | Dispatch & Mock Execute |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 13 |

## Overview

Phase 014 closes the job submission loop: queued jobs are dispatched to idle workers, executed by the Python worker (using mock nodes), and their status is updated to Completed or Failed based on the worker's response.

At phase start, submitted jobs sit in the queue with status `Queued` indefinitely. At phase end, a submitted mock job travels through Queued → Running → Completed with a `SaveImage` node emitting an `ImageReady` event carrying a 64×64 black PNG. All three transitions are broadcast to connected WebSocket clients.

The executor (`worker/executor.py`) and the `SaveImage` node created here are load-bearing for Phase 015 (Artifact Storage), which stores the `ImageReady` payload. The dispatch loop's worker-selection logic (rank by `vram_free_mib`, respect `device_preference`) is the policy that all subsequent phases rely on.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-scheduler + worker | P14-A1 … P14-A3 | Dispatch loop, Python mock executor, Completed/Failed event handling |

## Prerequisites

Phase 013 complete. `JobScheduler` with `submit`, `get_job`, `list_jobs` exists. `JobQueue` and `VramLedger` are operational. `WorkerPool` with `ManagedWorker` can send `WorkerMessage::Execute`. The `NODE_REGISTRY` auto-import and `BaseNode` ABC from Phase 011 are in place. `worker_main.py` handles the Execute IPC message stub.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §8.1` | P14-A1 | `WorkerMessage::Execute` fields: `job_id`, `graph`, `settings` |
| `ANVILML_DESIGN.md §8.2` | P14-A3 | `WorkerEvent::Completed { job_id, elapsed_ms }` and `WorkerEvent::Failed { job_id, error }` |
| `ANVILML_DESIGN.md §10.3` | P14-A2 | `SaveImage` node: input slot `image: IMAGE`, no output slots |

## Task Descriptions

### Group A — anvilml-scheduler and Python worker

#### P14-A1: anvilml-scheduler: dispatch loop background task

**Goal:** Add `start_dispatch_loop` to `JobScheduler` — a background Tokio task that wakes on new-job notifications or worker-Idle events, selects the best available worker, and dispatches the job.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — add `pub fn start_dispatch_loop(&self, workers: Arc<WorkerPool>) -> JoinHandle<()>` and `select_worker` helper
- `crates/anvilml-scheduler/tests/dispatch_tests.rs` — new file; ≥ 4 tests

**Key implementation notes:**
- Loop wakes via `tokio::sync::Notify` (new job) or a worker-Idle broadcast channel
- `select_worker`: rank all Idle workers by `vram_free_mib` descending; if job has `device_preference`, filter to that device index first; return first match
- On dispatch: mark job `Running` in DB + queue; call `ledger.reserve(device_index, job.vram_estimate_mib)`; send `WorkerMessage::Execute { job_id, graph, settings }` to selected worker
- `tracing::info!(job_id, worker_id, "job dispatched")`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 with ≥ 4 tests (job dispatched to Idle worker; VRAM reserved on dispatch; worker-Idle wakes loop; no dispatch when no Idle workers).

---

#### P14-A2: anvilml-worker: mock execute in worker_main.py and executor.py

**Goal:** Implement `worker/executor.py` with a topological-sort node executor, and add `worker/nodes/image.py` with the `SaveImage` node. In mock mode `SaveImage` emits `ImageReady` with a 64×64 black PNG encoded as base64.

**Files to create or modify:**
- `worker/executor.py` — new file; `run_graph(graph, settings, ctx)` function
- `worker/nodes/image.py` — new file; `SaveImage` node registered via `@register`
- `worker/worker_main.py` — handle `Execute` IPC message by calling `run_graph`; send `Completed { job_id, elapsed_ms }`

**Key implementation notes:**
- `run_graph`: topologically sort nodes from graph JSON; for each node: resolve inputs from prior nodes' outputs; instantiate from `NODE_REGISTRY`; call `node.execute(**inputs)`; store outputs
- `SaveImage` in mock mode: generate a 1×1 or 64×64 black PNG via `struct.pack` (no PIL import in mock path) and base64-encode it; emit `ImageReady { job_id, image_b64, width, height }` via `ctx.emit`
- `worker_main.py`: on `Execute` message, record `start = time.monotonic()`; call `run_graph`; send `Completed { job_id, elapsed_ms: int((time.monotonic()-start)*1000) }`

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py` exits 0 with ≥ 4 tests (run_graph executes all nodes in topo order; SaveImage emits ImageReady; Completed sent after run_graph; failed node sends Failed).

---

#### P14-A3: anvilml-scheduler: handle Completed/Failed events, update job status

**Goal:** Close the event loop in `JobScheduler` by subscribing to the worker event broadcast and updating job status when workers report Completed or Failed.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — add event subscription loop (spawned task); handle `WorkerEvent::Completed` and `WorkerEvent::Failed`

**Key implementation notes:**
- On `WorkerEvent::Completed { job_id, elapsed_ms }`: `UPDATE jobs SET status=Completed, completed_at=now WHERE id=job_id`; `ledger.release(device_index, reserved_mib)`; `broadcast(WsEvent::JobCompleted { job_id, elapsed_ms })`; `tracing::info!(job_id, elapsed_ms, "job completed")`
- On `WorkerEvent::Failed { job_id, error }`: `UPDATE jobs SET status=Failed, error=error WHERE id=job_id`; `ledger.release(...)`; `broadcast(WsEvent::JobFailed { job_id, error })`; `tracing::info!(job_id, error, "job failed")`
- Integration test: submit mock job; assert `GET /v1/jobs/:id` returns `{ status: "completed" }` within 10 s

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0; integration test verifies a submitted mock job transitions to Completed status.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Runnable Proof (manual): a submitted mock job is dispatched and reaches Completed
cargo run --features mock-hardware &
sleep 30
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
for i in $(seq 1 10); do
  STATUS=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")
  [ "$STATUS" = "Completed" ] && break
  sleep 1
done
[ "$STATUS" = "Completed" ]
# -> loop exits with $STATUS == "Completed" within 10s of dispatch
kill %1
```

## Known Constraints and Gotchas

- `run_graph` in `executor.py` must perform a topological sort from the graph JSON, not rely on array order. Node order in the JSON is not guaranteed.
- `SaveImage` in mock mode must never import `PIL`, `torch`, or `diffusers` — only stdlib (`struct`, `base64`, `zlib`). The real-hardware import guard is enforced via `if not MOCK:`.
- The dispatch loop `JoinHandle` must be stored in `AppState` or another long-lived owner — if it is dropped, the loop silently stops.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
