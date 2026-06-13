# Tasks: Phase 016 — Live Job Events

| Field | Value |
|-------|-------|
| Phase | 016 |
| Name | Live Job Events |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 15 |

## Overview

Phase 016 implements live job events as the next vertical slice. All tasks in this phase build on Phase 15 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Live Job Events | P16-A1…P16-A2 | Live Job Events implementation |

## Prerequisites

Phase 15 complete. Refer to `docs/TASKS_PHASE015.md` for the terminal task and Runnable Proof of Phase 15.

## Task Descriptions

### P16-A1: anvilml-worker: progress reporting in executor.py + worker_main.py

**Context:** Extend worker/executor.py: after each node executes, if step-based sampler: emit Progress{job_id,step,total_steps,preview_b64:None} via ctx.emit. In mock mode: emit 3 Progress events then ImageReady then Completed. Update worker_main.py: pass emit=ipc.send_event into NodeContext. ANVILML_WORKER_MOCK=1 pytest worker/tests/test_executor.py exits 0; test verifies Progress events emitted in order.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P16-A2: anvilml-scheduler: relay Progress and ImageReady events to WebSocket

**Context:** Extend scheduler.rs: on WorkerEvent::Progress: broadcast WsEvent::JobProgress{job_id,step,total_steps,preview_b64}. On ImageReady already handled in P15-A2. Verify event order with integration test: submit mock job; collect WS events; assert order JobQueued->JobStarted->JobProgress(x3)->JobImageReady->JobCompleted. cargo test -p anvilml-scheduler --features mock-hardware exits 0.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every pub item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO and DEBUG log points must be present before a task is marked complete.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
