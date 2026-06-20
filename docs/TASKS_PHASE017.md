# Tasks: Phase 017 — Cancellation

| Field | Value |
|-------|-------|
| Phase | 017 |
| Name | Cancellation |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 16 |

## Overview

Phase 017 implements job cancellation. Two distinct paths exist: cancelling a `Queued` job is immediate (the job is removed from the queue and marked `Cancelled` in a single operation); cancelling a `Running` job sends a `CancelJob` IPC message to the owning worker, which cooperatively stops and confirms via `WorkerEvent::Cancelled`. Attempting to cancel a job already in a terminal state returns 409.

This phase also adds the delete endpoints that allow operators to clean up finished jobs and their artifacts from disk and SQLite.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-scheduler + server | P17-A1 … P17-A2 | cancel_job logic, CancelJob IPC, HTTP cancel and delete endpoints |

## Prerequisites

Phase 016 complete. `JobScheduler` has running dispatch and event subscription loops. `WorkerMessage::CancelJob { job_id: Uuid }` and `WorkerEvent::Cancelled { job_id: Uuid }` exist in `anvilml-core`. `ArtifactStore::delete` exists or is added in P17-A2. The Python worker handles `CancelJob` messages (or that handling is added in P17-A1).

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §8.1` | P17-A1 | `WorkerMessage::CancelJob { job_id }` |
| `ANVILML_DESIGN.md §8.2` | P17-A1 | `WorkerEvent::Cancelled { job_id }` |
| `ANVILML_DESIGN.md §12.4` | P17-A2 | POST /v1/jobs/:id/cancel → 202; DELETE /v1/jobs/:id → 204; DELETE /v1/jobs → 200 `{ removed: u32 }` |
| `ANVILML_DESIGN.md §12.5` | P17-A2 | 409 error shape for terminal-state cancel attempt |

## Task Descriptions

### Group A — anvilml-scheduler and anvilml-server

#### P17-A1: anvilml-scheduler: cancel queued job (immediate) and cancel running job (IPC)

**Goal:** Implement `cancel_job` in `JobScheduler`. Queued cancellation is synchronous; running cancellation is asynchronous (the scheduler sends the IPC message and the actual cancellation is confirmed when `WorkerEvent::Cancelled` arrives).

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — add `pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError>` and `WorkerEvent::Cancelled` handler
- `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` — new file; ≥ 4 tests
- `worker/worker_main.py` — handle `WorkerMessage::CancelJob`: set cancel flag; send `WorkerEvent::Cancelled { job_id }`

**Key implementation notes:**
- `cancel_job`: look up job status; if `Queued`: `queue.cancel(id)`; `UPDATE jobs SET status=Cancelled`; `broadcast(WsEvent::JobCancelled { job_id })`; return `Ok(())`
- If `Running`: send `WorkerMessage::CancelJob { job_id }` to the owning worker; return `Ok(())` — caller receives 202 immediately, Cancelled status arrives asynchronously
- If already terminal (`Completed`, `Failed`, `Cancelled`): return `AnvilError::InvalidOperation` (409)
- On `WorkerEvent::Cancelled { job_id }`: `UPDATE jobs SET status=Cancelled`; `ledger.release(...)`; `broadcast(WsEvent::JobCancelled { job_id })`
- `tracing::info!(job_id, "job cancelled")`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with ≥ 4 tests (cancel queued → immediate Cancelled; cancel running → 202 then Cancelled via event; cancel terminal → error 409; cancel unknown → 404).

---

#### P17-A2: anvilml-server: POST /v1/jobs/:id/cancel + DELETE endpoints

**Goal:** Expose the cancellation and deletion HTTP endpoints. Delete endpoints are only valid for terminal jobs; attempting to delete a Queued or Running job returns 409.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — add `cancel_job`, `delete_job`, `bulk_clear` handlers
- `crates/anvilml-server/src/lib.rs` — mount `POST /v1/jobs/:id/cancel`, `DELETE /v1/jobs/:id`, `DELETE /v1/jobs`

**Key implementation notes:**
- `cancel_job(State<AppState>, Path<Uuid>)`: call `scheduler.cancel_job(id).await`; map `Ok(())` → 202; `AnvilError::InvalidOperation` → 409; `AnvilError::JobNotFound` → 404
- `delete_job(Path<Uuid>)`: only if terminal; `DELETE FROM jobs WHERE id=?`; delete artifact files via `artifact_store`; return 204
- `bulk_clear(Query<{ status: String }>)`: delete all jobs matching the status filter (must be a terminal status string: `completed`, `failed`, `cancelled`, or `all`); return `200 { removed: u32 }`
- Integration test: cancel a queued job via POST → 202; verify `GET /v1/jobs/:id` returns `{ status: "cancelled" }`

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; integration test covers cancel → 202 → Cancelled status confirmed via GET.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Runnable Proof (manual): submit and immediately cancel a job
cargo run --features mock-hardware &
sleep 30
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
curl -s -o /dev/null -w "%{http_code}" -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 202 (cancel accepted, whether job was still Queued or already Running)
sleep 1
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status'] in ('Cancelled','Completed')"
# -> status is a terminal state (Cancelled, or Completed if the race favored execution)
kill %1
```

## Known Constraints and Gotchas

- The `bulk_clear` endpoint must reject non-terminal status values (e.g. `status=running`) with 400. Only `completed`, `failed`, `cancelled`, and `all` are valid.
- When deleting jobs with `bulk_clear`, artifact files must be deleted from disk alongside the DB rows. An orphaned PNG on disk with no DB entry is acceptable; a DB entry with no file is not.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
