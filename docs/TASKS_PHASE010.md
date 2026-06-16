# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 9 |

## Overview

Phase 010 implements automatic worker crash recovery. At the end of Phase 009 the server can spawn and manage workers, but a worker process that exits unexpectedly is never restarted. This phase adds the policy type that decides when and how often to respawn, the crash-detection loop inside `ManagedWorker`, and the HTTP endpoint that lets operators force-restart a specific worker.

At phase start the binary can spawn workers and serve the health endpoint. At phase end a killed worker process is automatically detected, the in-flight job is marked Failed, and the worker respawns after a configurable backoff — all observable via the WebSocket event stream. The `POST /v1/workers/:id/restart` endpoint provides a manual override path.

Phase 011 (Dynamic Node Registry) depends on workers reaching Idle after respawn; the respawn logic landed here is the prerequisite for that behaviour.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker | P10-A1 … P10-A2 | RespawnPolicy type, crash detection and automatic respawn |
| B | anvilml-server | P10-B1 | POST /v1/workers/:id/restart handler |

## Prerequisites

Phase 009 complete. `ManagedWorker` in `crates/anvilml-worker/src/managed.rs` exists with a running loop that handles IPC events. `WorkerPool` exists and manages one `ManagedWorker` per GPU. `WorkerStatusChanged` events are broadcast via `EventBroadcaster`. The in-flight job tracking callback exists on the worker so the scheduler can be notified on unexpected exit.

## Task Descriptions

### Group A — anvilml-worker

#### P10-A1: anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard

**Goal:** Create `crates/anvilml-worker/src/respawn.rs` providing the `RespawnPolicy` type that determines whether a crashed worker should be restarted and how long to wait before doing so. This type is pure logic with no I/O; it is consumed by `ManagedWorker` in P10-A2.

**Files to create or modify:**
- `crates/anvilml-worker/src/respawn.rs` — new file; `RespawnPolicy` struct and both public methods
- `crates/anvilml-worker/src/lib.rs` — add `pub mod respawn`

**Key implementation notes:**
- `RespawnPolicy { delay_ms: u64, max_attempts: u32, window_s: u32 }` — all fields pub
- `pub fn should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool` — returns false if `crash_count >= max_attempts`; resets count if `last_crash` is older than `window_s`
- `pub fn next_delay_ms(&self, attempt: u32) -> u64` — exponential backoff starting at `delay_ms`, capped at 30 000 ms

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 with ≥ 4 tests (max exceeded → false; within window → true; outside window resets count; delay sequence correct and capped).

---

#### P10-A2: anvilml-worker: managed.rs crash detection and automatic respawn

**Goal:** Extend `ManagedWorker`'s run loop to detect unexpected child process exits, mark the worker `Dead`, fail any in-flight job, and then attempt a respawn according to `RespawnPolicy`.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — add crash-detection branch and respawn logic
- `crates/anvilml-worker/tests/managed_tests.rs` — new or extend; crash cycle test

**Key implementation notes:**
- `child.wait()` is selected alongside the IPC event channel; unexpected exit (non-zero or signal) triggers the Dead path, not graceful shutdown
- On Dead: broadcast `WorkerStatusChanged(Dead)`; if in-flight job exists, call the job-store callback with `error = "worker_crashed"`
- After `respawn_delay`: call `RespawnPolicy::should_respawn`; if true, transition to Respawning, re-run spawn, broadcast `WorkerStatusChanged(Respawning)`
- `tracing::info!(worker_id, exit_code, "worker exited unexpectedly")` on Dead
- `tracing::info!(worker_id, attempt, delay_ms, "respawning worker")` on Respawning

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0; `tests/managed_tests.rs` includes a test that kills the worker process and asserts the Dead → Respawning → Idle event sequence.

---

### Group B — anvilml-server

#### P10-B1: anvilml-server: POST /v1/workers/:id/restart handler

**Goal:** Expose `POST /v1/workers/:id/restart` so operators can force-restart a worker without waiting for natural crash detection. The handler sends a graceful Shutdown IPC message and force-kills if the worker has not exited within 5 seconds, then triggers a respawn.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/workers.rs` — add `restart_worker` handler
- `crates/anvilml-server/src/lib.rs` — mount route in `build_router`

**Key implementation notes:**
- Lookup worker by id in `WorkerPool`; return 404 if not found
- Send `WorkerMessage::Shutdown`; set a 5-second deadline; force-kill on timeout
- Return 202 immediately — the respawn is asynchronous; the caller polls `GET /v1/workers` to observe Idle
- Integration test: restart a running worker via the endpoint; assert `GET /v1/workers` shows Idle within 30 s

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; integration test verifies 202 response and subsequent Idle status.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- `child.wait()` must be polled inside a `tokio::select!` alongside the IPC channel. Do not block the async runtime with a synchronous wait.
- The respawn delay must be awaited with `tokio::time::sleep`, not `std::thread::sleep`.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every `pub` item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO log points must be present on the Dead and Respawning transitions.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
