# Tasks: Phase 014 — Dispatch & Mock Execute

| Field | Value |
|-------|-------|
| Phase | 014 |
| Name | Dispatch & Mock Execute |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 13 |

## Overview

Phase 014 implements dispatch & mock execute as the next vertical slice. All tasks in this phase build on Phase 13 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Dispatch & Mock Execute | P14-A1…P14-A3 | Dispatch & Mock Execute implementation |

## Prerequisites

Phase 13 complete. Refer to `docs/TASKS_PHASE013.md` for the terminal task and Runnable Proof of Phase 13.

## Task Descriptions

### P14-A1: anvilml-scheduler: dispatch loop background task

**Context:** Add to scheduler.rs: pub fn start_dispatch_loop(&self,workers:Arc<WorkerPool>)->JoinHandle<()>. Background tokio task: wake on new-job Notify or worker-Idle channel. Per wake: iterate queue; for each Queued job call select_worker (rank Idle workers by vram_free_mib desc; prefer device_preference if set); dispatch first match. tracing::info!(job_id,worker_id,"job dispatched"). Mark job Running, res...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P14-A2: anvilml-worker: mock execute in worker_main.py and executor.py

**Context:** Create worker/executor.py: def run_graph(graph:dict,settings:dict,ctx:NodeContext)->None. Topo-sort nodes; for each node: resolve inputs from outputs; instantiate node class from NODE_REGISTRY; call execute(**inputs); store outputs. Create worker/nodes/image.py SaveImage node (mock: emit ImageReady with 64x64 black PNG b64). In worker_main.py handle Execute message: run_graph; send Completed{job_i...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P14-A3: anvilml-scheduler: handle Completed/Failed events, update job status

**Context:** Extend scheduler.rs: subscribe to worker event broadcast. On Completed{job_id}: UPDATE jobs SET status=Completed,completed_at=now; release VRAM; broadcast WsEvent::JobCompleted; tracing::info!(job_id,elapsed_ms). On Failed{job_id,error}: UPDATE jobs SET status=Failed,error; release VRAM; broadcast WsEvent::JobFailed; tracing::info!(job_id,error). cargo test -p anvilml-scheduler --features mock-har...

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
