# Tasks: Phase 017 — Cancellation

| Field | Value |
|-------|-------|
| Phase | 017 |
| Name | Cancellation |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 16 |

## Overview

Phase 017 implements cancellation as the next vertical slice. All tasks in this phase build on Phase 16 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Cancellation | P17-A1…P17-A2 | Cancellation implementation |

## Prerequisites

Phase 16 complete. Refer to `docs/TASKS_PHASE016.md` for the terminal task and Runnable Proof of Phase 16.

## Task Descriptions

### P17-A1: anvilml-scheduler: cancel queued job (immediate) and cancel running job (IPC)

**Context:** Add to scheduler.rs: pub async fn cancel_job(&self,id:Uuid)->Result<(),AnvilError>. If Queued: mark Cancelled in queue+DB; broadcast JobCancelled; return Ok. If Running: send WorkerMessage::CancelJob{job_id} to owning worker; return Ok (actual cancellation confirmed via WorkerEvent::Cancelled). If terminal: return AnvilError::InvalidOperation (409). On WorkerEvent::Cancelled{job_id}: mark Cancelle...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P17-A2: anvilml-server: POST /v1/jobs/:id/cancel + DELETE endpoints

**Context:** Add to handlers/jobs.rs: cancel_job(State<AppState>,Path<Uuid>): call scheduler.cancel_job(); 202 on Ok; 409 on InvalidOperation; 404 if not found. delete_job(Path<Uuid>): only if terminal; DELETE from DB+artifacts. bulk_clear(Query<{status}>): DELETE matching terminal jobs+artifacts. Mount POST /v1/jobs/:id/cancel, DELETE /v1/jobs/:id, DELETE /v1/jobs in build_router. Integration test: cancel que...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every pub item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO and DEBUG log points must be present before a task is marked complete.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
