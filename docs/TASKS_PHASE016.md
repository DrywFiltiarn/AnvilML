# Tasks: Phase 016 — Live Job Events

| Field | Value |
|-------|-------|
| Phase | 016 |
| Name | Live Job Events |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 15 |

## Overview

Phase 016 adds per-step progress reporting to the job lifecycle. Before this phase, a job transitions silently from Running to Completed with no intermediate updates. After this phase, the worker emits `Progress` events after each node execution (when the node is a step-based sampler), and those events are relayed to all connected WebSocket clients as `WsEvent::JobProgress`.

In mock mode the executor emits exactly 3 Progress events, then `ImageReady`, then `Completed` — giving the complete observable event sequence that integration tests and frontend developers can rely on.

The `ImageReady` relay from scheduler to WebSocket was added in Phase 015 (P15-A2). Phase 016 only adds Progress relaying; `ImageReady` handling is intentionally not duplicated here.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Python worker + anvilml-scheduler | P16-A1 … P16-A2 | Progress emission in executor, relay to WebSocket |

## Prerequisites

Phase 015 complete. `NodeContext` carries an `emit` callable. `WorkerEvent::Progress` variant exists in `anvilml-core`. `WsEvent::JobProgress` variant exists in `anvilml-core`. The scheduler subscribes to the worker event broadcast. Mock job execution via `run_graph` is functional.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §8.2` | P16-A1 | `WorkerEvent::Progress { job_id, step, total_steps, preview_b64: Option<String> }` |
| `ANVILML_DESIGN.md §5.8` | P16-A2 | `WsEvent::JobProgress { job_id, step, total_steps, preview_b64 }` |

## Task Descriptions

### Group A — Python worker and anvilml-scheduler

#### P16-A1: anvilml-worker: progress reporting in executor.py and worker_main.py

**Goal:** Extend `executor.py` so that after each node executes, if the node class declares it is step-based (via a `EMITS_PROGRESS = True` class attribute or similar convention), a `Progress` event is emitted via `ctx.emit`. In mock mode the executor emits exactly 3 Progress events before `ImageReady` to produce a predictable test sequence.

**Files to create or modify:**
- `worker/executor.py` — extend node execution loop to emit Progress via `ctx.emit`
- `worker/worker_main.py` — pass `emit = ipc.send_event` into `NodeContext` at Execute handling

**Key implementation notes:**
- `ctx.emit` was added to `NodeContext` in Phase 011; it is now populated in `worker_main.py` rather than being a stub
- In mock mode: `run_graph` unconditionally emits 3 Progress events (`step=1,2,3`, `total_steps=3`, `preview_b64=None`) after the Sampler node (or any node that sets `EMITS_PROGRESS = True`)
- `WorkerEvent::Progress { job_id, step, total_steps, preview_b64: None }` is the IPC message shape

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py` exits 0; test verifies that exactly 3 Progress events are emitted in order (step 1, 2, 3) before ImageReady for a mock Sampler node.

---

#### P16-A2: anvilml-scheduler: relay Progress and ImageReady events to WebSocket

**Goal:** Extend the scheduler's worker event subscription to handle `WorkerEvent::Progress` and relay it as `WsEvent::JobProgress`. Verify the full mock job event sequence end-to-end.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — add `WorkerEvent::Progress` arm to event subscription handler

**Key implementation notes:**
- On `WorkerEvent::Progress { job_id, step, total_steps, preview_b64 }`: broadcast `WsEvent::JobProgress { job_id, step, total_steps, preview_b64 }`; no DB write needed for progress events
- Integration test: subscribe to WS before submitting mock job; collect events until Completed; assert order is `JobQueued → JobStarted → JobProgress(step=1) → JobProgress(step=2) → JobProgress(step=3) → JobImageReady → JobCompleted`
- `tracing::debug!(job_id, step, total_steps, "progress event relayed")`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0; integration test asserts the full 7-event sequence in the correct order.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- The integration test for event sequence ordering must account for the fact that `JobStarted` is broadcast by the dispatch loop at the moment the Execute message is sent to the worker. If this event is not yet broadcast in Phase 014, it must be added as part of P16-A2.
- Progress events are not persisted to SQLite — only the terminal statuses (Completed, Failed, Cancelled) are rows. Do not add a DB write for Progress.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
