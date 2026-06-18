# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Project | anvilml |
| Status | Complete |
| Depends on phases | 9, 901 |

## Overview

Phase 010 implements automatic worker crash recovery and operator-initiated worker restart.
At the end of Phase 009 the server can spawn and manage workers, but a worker process that
exits unexpectedly is never restarted. This phase adds the policy type that decides when and
how often to respawn, the crash-detection logic inside `ManagedWorker`, the respawn cycle
that follows it, and the HTTP endpoint that lets operators force-restart a specific worker.

At phase start the binary can spawn workers and serve the health endpoint. At phase end:
a killed worker process is automatically detected, the worker respawns after a configurable
backoff, a heartbeat-timeout (no pong within `pong_timeout`) also triggers respawn, and a
manual `POST /v1/workers/:id/restart` override bypasses policy for immediate restart — all
observable via `GET /v1/workers` or the WebSocket event stream.

**Phase 010 was implemented in two tasks, not four:**

- `P10-A1` delivered `RespawnPolicy` as originally planned.
- `P10-A2` absorbed the work of the original `P10-A2` (crash detection), the original
  `P10-A3` (respawn cycle), and the original `P10-B1` (restart endpoint) into a single
  coordinated implementation, plus addressed several structural issues discovered during
  implementation (see P10-A2 implementation report for detail). The combination was more
  reliable than separate sequential tasks because all three features share the same
  `ManagedWorker::run()` loop and `WorkerPool` surface.
- `P10-A3` is a **verification-only task**: read the delivered source files, run the full
  test suite, and produce a report documenting what exists. No new code is written.

**In-flight job failure notification is out of scope for this phase.** The original task
text for the crash-detection subtask stated that the in-flight job tracking callback would
exist on the worker, but no such callback exists anywhere in the codebase at this point —
`JobScheduler` is not introduced until `P13-A3`, and jobs are not dispatched to workers
until `P14-A1`. A `// TODO(P14): notify JobScheduler of in-flight job failure once it exists`
comment is left at the `Dead` transition site.

Phase 011 (Dynamic Node Registry) depends on workers reaching Idle after respawn; the
respawn logic landed here is the prerequisite for that behaviour.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker/server | P10-A1, P10-A2, P10-A3 | RespawnPolicy; complete crash detection + respawn + restart; verification |

## Prerequisites

Phase 009 complete. `ManagedWorker` in `crates/anvilml-worker/src/managed.rs` exists with a
`run()` loop that processes IPC events continuously (corrected in Phase 901). `WorkerPool`
exists and manages one `ManagedWorker` per GPU. `RespawnPolicy::should_respawn` correctly
resets its crash counter against the configured time window (corrected in Phase 901). There
is no in-flight job tracking on `ManagedWorker` at this point in the codebase; do not assume
one exists.

## Task Descriptions

### P10-A1: anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard

**Goal:** Provide the `RespawnPolicy` type — pure Rust stdlib logic with no I/O — that
determines whether and how long to wait before respawning a dead worker. Consumed by
`ManagedWorker` in P10-A2.

**Files created or modified:**
- `crates/anvilml-worker/src/respawn.rs` — `RespawnPolicy` struct with production `should_respawn` and `next_delay_ms`
- `crates/anvilml-worker/tests/respawn_tests.rs` — four unit tests
- `crates/anvilml-worker/Cargo.toml` — version bump

**Key implementation notes:**
- `RespawnPolicy { delay_ms: u64, max_attempts: u32, window_s: u32 }` — all fields pub
- `pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool` — takes
  `crash_count` by mutable reference; performs window-reset internally (resets to 0 if
  `last_crash` is older than `window_s`), then increments and returns `true`, or returns
  `false` if `crash_count >= max_attempts` after reset. The mutable-reference signature
  was the Phase 901 retrofit; the P10-A1 plan described a value-taking signature that was
  subsequently corrected.
- `pub fn next_delay_ms(&self, attempt: u32) -> u64` — `delay_ms * 2^attempt` capped at 30 000 ms

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- respawn`
exits 0 with ≥ 4 tests.

---

### P10-A2: anvilml-worker/server: complete crash detection, respawn cycle, and operator restart

**Goal:** Implement the full crash recovery surface in one coordinated task: automatic crash
detection (child exit, heartbeat timeout), the `Dead → Respawning → Initializing → Idle`
respawn cycle via `do_respawn()`, and the `POST /v1/workers/:id/restart` operator endpoint.

**Files modified:**
- `crates/anvilml-worker/src/managed.rs` — seven new fields; `event_tx: Option<...>`;
  `do_respawn()`; six-arm `select!`; `loop_child` per-iteration pattern
- `crates/anvilml-worker/src/pool.rs` — `restart_tx: watch::Sender<u64>` in `WorkerHandle`;
  `restart_worker()` public method
- `crates/anvilml-worker/tests/managed_tests.rs` — new 18-arg `new()` signature; stub helpers;
  `test_child_exit_transitions_dead` (Dead\|\|Respawning assertion);
  `test_respawn_cycle_entered_after_child_exit`
- `crates/anvilml-worker/tests/pool_tests.rs` — new `new()` signature; async helpers
- `crates/anvilml-server/src/handlers/workers.rs` — `restart_worker` handler
- `crates/anvilml-server/src/lib.rs` — route registration
- `crates/anvilml-server/tests/workers_tests.rs` — new `new()` signature; stub helpers

**Key implementation notes:**

- `ManagedWorker::new()` gains five new parameters (in order after `heartbeat_handle`):
  `cfg: ServerConfig`, `device: GpuDevice`, `transport: Arc<RouterTransport>`,
  `timeout_rx: oneshot::Receiver<()>`, `restart_rx: watch::Receiver<u64>`.
  Total: 18 parameters.

- `ManagedWorker::spawn()` gains one new parameter: `restart_rx: watch::Receiver<u64>`,
  passed in by `WorkerPool::spawn_all()`. The watch channel pair is built once in
  `spawn_all()`; the receiver is cloned through every subsequent `do_respawn()` call using
  `self.restart_rx.clone()`. The sender stays in `WorkerHandle::restart_tx` for the pool's
  entire lifetime — no replacement needed after respawns.

- `event_tx` is `Option<broadcast::Sender<...>>` on the struct. `run()` calls
  `self.event_tx.as_ref().expect(...).subscribe()` then `self.event_tx.take()` to drop
  without a partial move (which would prevent `do_respawn(&mut self)` from compiling).

- The `on_timeout` closure captures `Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>`.
  `keepalive::start` requires `Fn()` (not `FnMut()`); `Mutex` provides the interior
  mutability needed for `Option::take()` inside a shared-reference closure. The closure
  fires the sender once; subsequent calls are no-ops.

- `do_respawn(consult_policy: bool)` is a private async method. When `consult_policy`:
  calls `should_respawn(&mut self.crash_count, self.last_crash)`, returns `Err(Internal)`
  on false, updates `last_crash`, computes delay, sleeps. Always: checks `self.routes` (if
  `None` returns `Err(Internal)` — test-only path, no RouteTable to register into), calls
  `Self::spawn(...)`, subscribes new `event_rx` from the new worker's `event_tx` before
  `*self = new_worker`, calls `self.event_tx.take()` after.

- Before each `select!` iteration: `let mut loop_child = self.child.take()`. After:
  `self.child = loop_child`. This prevents a borrow conflict between `self.child.as_mut()`
  in the child-wait async block and `&mut self.timeout_rx` as another select arm, which
  would otherwise cause the child-wait arm to silently use the `None`/`pending()` branch.

- Test assertions use `Dead || Respawning`: on the single-threaded `#[tokio::test]` runtime,
  `Dead` is written by the child-exit arm and immediately overwritten by `Respawning` inside
  `do_respawn` before the polling task is scheduled. Either status proves crash detection and
  respawn-entry occurred.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 with
12 passing managed tests; `cargo run` with a real Python worker confirms live respawn and
202 from `POST /v1/workers/:id/restart`.

---

### P10-A3: anvilml-worker/server: verify Phase 010 implementation and document findings

**Goal:** Verification-only task. Read the delivered source files, run the full test suite,
and produce an implementation report documenting what exists. No source code is written or
modified.

**Output:** `.forge/reports/P10-A3_implement.md` with verbatim test output and a full
description of the as-built architecture.

**Key notes for the agent:**
- Read every file listed in P10-A3's plan `## Scope` section before writing anything.
  The P10-A2 synthetic report describes what was intended; this task verifies what is
  actually present. Do not trust any prior report — inspect the source directly.
- If `cargo test --workspace --features mock-hardware` exits non-zero, write the failure
  under `## Blockers` and stop. Do not attempt to fix it.
- If the source differs from the P10-A2 plan in any way, document the difference under
  `## Deviations from Plan`. A difference is not automatically a defect — it may be an
  intentional structural adjustment; document it either way.

**Acceptance criterion:** `.forge/reports/P10-A3_implement.md` begins with
`# Implementation Report: P10-A3`; `grep "^## "` returns exactly 11 lines;
`## Test Results` shows 0 failures; `## Blockers` is present.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware   # exits 0
# Manual smoke test with a running server:
# Kill a Python worker in the OS task manager → observe respawn via GET /v1/workers
# curl -X POST http://localhost:8488/v1/workers/worker-0/restart → HTTP 202
```

## Known Constraints and Gotchas

- `child.wait()` in the `select!` arm uses a per-iteration `async {}` block wrapping
  `loop_child.as_mut()`. This is cancel-safe on Windows because the process-exit handle
  remains signalled after child exit, so re-polling on the next iteration resolves
  immediately. A persistent pinned future approach was investigated but is structurally
  incompatible with the requirement to kill the child from other arms; the per-iteration
  pattern is the correct and proven solution.
- `RespawnPolicy::should_respawn` takes `crash_count` by mutable reference (Phase 901
  corrected signature). The `do_respawn` call site passes `&mut self.crash_count`.
- `do_respawn` returns `Err` immediately when `self.routes` is `None` (test-only workers
  built via `new()` with no `RouteTable`). Tests observe `Respawning` status as the final
  observable state; full `Idle` recovery is only reachable in production with a real Python
  venv and a live `RouteTable`.
- The `WorkerPool` background status-monitor captures the original status `Arc` at spawn
  time. After `*self = new_worker` in `do_respawn`, the monitor's copy is frozen at `Dead`.
  WebSocket `WorkerStatusChanged` events for the respawned worker's lifecycle are not
  broadcast until Phase 011 adds a mechanism to update the monitor's reference.