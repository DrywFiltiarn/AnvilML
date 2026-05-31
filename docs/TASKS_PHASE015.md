# Tasks: Phase 015 — Live Job Events

| Field | Value |
|-------|-------|
| Phase | 015 |
| Name | Live Job Events |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-14 |
| Task file | `forge/tasks/tasks_phase015.json` |
| Tasks | 3 |

## Overview

Phase 15 wires the full job lifecycle onto the WebSocket: `job.queued`, `job.started`, `job.progress`, `job.image_ready`, and `job.completed`, plus an integration test that asserts the ordered sequence. After this phase a client watching `/v1/events` sees a job's entire life unfold in real time.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P15-A1 | `crates/anvilml-scheduler/src/scheduler.rs` | anvilml-scheduler: emit JobProgress events from worker Progress |
| P15-A2 | `backend/tests/api_ws_lifecycle.rs` | anvilml: integration test asserting full WS lifecycle for a mock job |
| P15-A3 | `docs/PROOF_phase015.md` | anvilml: documented websocat/browser proof of live job events |

## Task details

#### P15-A1: anvilml-scheduler: emit JobProgress events from worker Progress

- **Prereqs:** P14-A5
- **Tags:** —

In dispatch loop event handling: on WorkerEvent::Progress{job_id,node_index,node_total,node_type} broadcaster.send(WsEvent::JobProgress{...}) (step/step_total None in MVP). Ensure JobQueued (submit), JobStarted (dispatch), JobProgress, JobImageReady, JobCompleted are all wired through the broadcaster. cargo test --workspace --features mock-hardware exits 0.

#### P15-A2: anvilml: integration test asserting full WS lifecycle for a mock job

- **Prereqs:** P15-A1
- **Tags:** reasoning

Create backend/tests/api_ws_lifecycle.rs: bind app on 127.0.0.1:0 with ANVILML_WORKER_MOCK=1 + mock-hardware + in-memory or tempfile DB + a venv python on PATH (skip test gracefully if absent). Connect tokio-tungstenite to /v1/events, POST a valid ZiT job, assert frames received in order: job.queued, job.started, job.progress (>=1), job.image_ready, job.completed within 20s. cargo test --features mock-hardware --test api_ws_lifecycle exits 0.

#### P15-A3: anvilml: documented websocat/browser proof of live job events

- **Prereqs:** P15-A2
- **Tags:** —

No new code. Add a short docs/PROOF_phase015.md (or section) with exact commands: terminal 1 run server (mock); terminal 2 `websocat ws://127.0.0.1:8488/v1/events`; terminal 3 curl POST a ZiT job. Document the expected JSON frame sequence the user will see. This task is complete when a human following the steps observes queued->started->progress->image_ready->completed in the websocat stream.


## Runnable Proof

Watch a job's events stream live while it runs.

```bash
# terminal 1:
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --features mock-hardware
# terminal 2:
websocat ws://127.0.0.1:8488/v1/events
# terminal 3:
curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'content-type: application/json' -d @valid_zit_job.json
```

Expected: terminal 2 prints, in order, frames with `event` values `job.queued`, `job.started`, one or more `job.progress`, `job.image_ready` (carrying `artifact_hash`), and `job.completed` — interleaved with the periodic `system.stats`. Phase done when the ordered lifecycle is observed live and `cargo test --features mock-hardware --test api_ws_lifecycle` is green.
