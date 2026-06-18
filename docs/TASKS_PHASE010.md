# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 9, 901, 902 (P10-A3 only — see Overview) |

## Overview

Phase 010 implements automatic worker crash recovery. At the end of Phase 009 the server
can spawn and manage workers, but a worker process that exits unexpectedly is never
restarted. This phase adds the policy type that decides when and how often to respawn, the
crash-detection logic inside `ManagedWorker`, the respawn cycle that follows it, and the
HTTP endpoint that lets operators force-restart a specific worker.

At phase start the binary can spawn workers and serve the health endpoint. At phase end a
killed worker process is automatically detected, the worker respawns after a configurable
backoff, and a manual `POST /v1/workers/:id/restart` override path exists — all observable
via the WebSocket event stream.

`P10-A2` was originally a single task combining crash detection and the respawn cycle. It
is now two tasks (`P10-A2`, `P10-A3`) because the combination depends on `ManagedWorker::run()`
actually looping continuously and `RespawnPolicy::should_respawn` actually resetting its
crash counter — neither of which was true in the codebase this phase started from. Both
defects are corrected in the **Phase 901** retrofit, which sits between `P10-A1` and the
renumbered `P10-A2`. Splitting the work this way also means crash detection (Dead) and the
respawn cycle (Respawning → Idle) can each be verified independently before being composed,
rather than debugged together as one unit — the combined version previously stalled for
several hours without producing a working result.

**In-flight job failure notification is out of scope for this phase.** The original task
text for `P10-A2` stated that "the in-flight job tracking callback exists on the worker," but
no such callback exists anywhere in the codebase at this point — `JobScheduler`, the type
that would own that callback, is not introduced until `P13-A3`, and no job is ever dispatched
to a worker until `P14-A1`. The crash-detection task leaves a `// TODO(P14)` comment at the
`Dead` transition site instead. Whichever task in Phase 14 wires worker crash events into job
failure should reference this comment.

Phase 011 (Dynamic Node Registry) depends on workers reaching Idle after respawn; the
respawn logic landed here is the prerequisite for that behaviour.

**`P10-A3` depends on the Phase 902 retrofit, not directly on `P10-A2`.** Phase 902 is a
three-task retrofit, authored and completed after `P10-A2` finished, that fixed two
unrelated defects in `crates/anvilml-worker/src/managed.rs` and `src/keepalive.rs`
discovered by running the actual binary: a keepalive ping-before-Ready race, and a
shutdown sequence that could stall for several seconds. See `docs/TASKS_PHASE902.md` for
the full account. The practical consequence for `P10-A3` is described in its own Key
implementation notes below — read `managed.rs` and `keepalive.rs` directly before
planning, rather than assuming their shape from `P10-A2`'s own reports.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker | P10-A1 … P10-A3 | RespawnPolicy type; crash detection (Dead); respawn cycle (Respawning → Idle) |
| B | anvilml-server | P10-B1 | POST /v1/workers/:id/restart handler |

## Prerequisites

Phase 009 complete. `ManagedWorker` in `crates/anvilml-worker/src/managed.rs` exists with a
`run()` loop that processes IPC events continuously (corrected in Phase 901 — see above).
`WorkerPool` exists and manages one `ManagedWorker` per GPU. `WorkerStatusChanged` events are
broadcast via `EventBroadcaster`. `RespawnPolicy::should_respawn` correctly resets its crash
counter against the configured time window (corrected in Phase 901 — see above). There is no
in-flight job tracking on `ManagedWorker` at this point in the codebase; do not assume one
exists. For `P10-A3` specifically, Phase 902 must also be complete — it changed both
`managed.rs` and `keepalive.rs` after `P10-A2`; see Overview above.

## Task Descriptions

### Group A — anvilml-worker

#### P10-A1: anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard

**Goal:** Create `crates/anvilml-worker/src/respawn.rs` providing the `RespawnPolicy` type
that determines whether a crashed worker should be restarted and how long to wait before
doing so. This type is pure logic with no I/O; it is consumed by `ManagedWorker` in `P10-A3`.

**Files to create or modify:**
- `crates/anvilml-worker/src/respawn.rs` — new file; `RespawnPolicy` struct and both public methods
- `crates/anvilml-worker/src/lib.rs` — add `pub mod respawn`

**Key implementation notes:**
- `RespawnPolicy { delay_ms: u64, max_attempts: u32, window_s: u32 }` — all fields pub
- `pub fn should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool` — returns false if `crash_count >= max_attempts`; resets count if `last_crash` is older than `window_s`. **Note:** Phase 901 changes this signature to take `crash_count` by mutable reference so the reset is performed by the function itself rather than left to caller discretion — implement to the signature in this task's acceptance criterion, and expect the Phase 901 retrofit to revise it.
- `pub fn next_delay_ms(&self, attempt: u32) -> u64` — exponential backoff starting at `delay_ms`, capped at 30 000 ms

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 with ≥ 4 tests (max exceeded → false; within window → true; outside window resets count; delay sequence correct and capped).

---

#### P10-A2: anvilml-worker: managed.rs detect unexpected child exit and transition to Dead

**Goal:** Extend `ManagedWorker`'s run loop to detect an unexpected child process exit and
mark the worker `Dead`. This task does not implement respawn — see `P10-A3`.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — add a `child.wait()` branch to the `run()` loop's `select!`
- `crates/anvilml-worker/tests/managed_tests.rs` — extend; crash-detection test

**Key implementation notes:**
- This task assumes `run()` already loops continuously and prerequisites the final task of Phase 901 (`P901-A3`) for exactly that reason — do not attempt this against the pre-Phase-901 `run()`, which exits after one event and cannot host a concurrent `child.wait()` branch.
- `child.wait()` is selected alongside the IPC event channel inside the existing loop, as a third arm — not a separate function, not a separate task that supersedes the loop.
- On unexpected exit: broadcast `WorkerStatusChanged(Dead)`; `tracing::info!(worker_id, exit_code, "worker exited unexpectedly")`.
- Leave a `// TODO(P14): notify JobScheduler of in-flight job failure once it exists` comment at the Dead transition site. Do not invent a callback mechanism — there is nothing in the codebase yet for it to call.
- `run()` must continue looping after the Dead transition (it does not return) so that `P10-A3` can add the respawn branch without re-touching this control flow.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0; `tests/managed_tests.rs` includes a test that kills the spawned subprocess and asserts the `Dead` transition and broadcast are observed.

---

#### P10-A3: anvilml-worker: managed.rs respawn cycle after Dead using RespawnPolicy

**Goal:** Extend the `Dead` path added in `P10-A2` with the respawn cycle: wait the policy's
backoff delay, consult `RespawnPolicy::should_respawn`, and either respawn or remain `Dead`.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — add respawn branch following the `Dead` transition
- `crates/anvilml-worker/tests/managed_tests.rs` — extend; full crash-respawn-cycle test

**Key implementation notes:**
- Before planning, read `crates/anvilml-worker/src/managed.rs` and
  `src/keepalive.rs` directly on disk in full. Both were modified by the Phase 902
  retrofit after `P10-A2` completed (ready-gated keepalive, abort-on-Dead, a
  watch-channel-based prompt shutdown) — do not infer their current shape from `P10-A2`'s
  plan or implementation reports, which describe an earlier version of both files.
- `ManagedWorker::spawn()`'s current signature, as of Phase 902, is:
  `spawn(cfg: &ServerConfig, device: &GpuDevice, transport: Arc<RouterTransport>,
  worker_id: String, routes: crate::demux::RouteTable) -> Result<Self, AnvilError>` —
  five arguments, not three. `spawn()` registers its own route into `routes` internally
  before returning (it does not return a separate route to register elsewhere), and also
  constructs a fresh `ready_tx`/`ready_rx` pair for the new keepalive gate — do not
  attempt to reuse or carry over the dead worker's old `ready_tx`.
- `ManagedWorker` does not currently store owned `cfg`/`device` anywhere — only `routes`
  is retained as a struct field (`Option<crate::demux::RouteTable>`). Both
  `ServerConfig` and `GpuDevice` derive `Clone`, so storing an owned clone of each on the
  struct (populated once in `spawn()`, alongside the existing `routes` field) is the
  straightforward way to make them available again at respawn time. This is a structural
  decision this task's plan step must state explicitly, not discover mid-implementation.
- When respawning, reuse the existing `routes` field (the same `RouteTable` handle the
  dead worker was already registered in) rather than constructing a new one — the new
  `ManagedWorker::spawn()` call registers the respawned worker's fresh wire identity into
  that same shared table, replacing the dead worker's now-stale entry at the same logical
  slot. Constructing a fresh, empty `RouteTable` for the respawn would silently disconnect
  the respawned worker from the demux task that is still running against the original
  table.
- Await the delay with `tokio::time::sleep(Duration::from_millis(policy.next_delay_ms(attempt)))`
  — never `std::thread::sleep`, which would block the async runtime.
- Call `should_respawn(&mut crash_count, last_crash)` (Phase 901's corrected signature). If
  `true`: transition `Respawning`, broadcast `WorkerStatusChanged(Respawning)`, re-run
  `spawn()` with the five-argument call shape above, and continue the same `run()` loop —
  `run()` must not return after a successful respawn. If `false`: remain `Dead` and log
  `tracing::info!(worker_id, "respawn attempts exhausted")`.
- `tracing::info!(worker_id, attempt, delay_ms, "respawning worker")` on the `Respawning`
  transition.
- The respawned worker's keepalive will not send its first ping until its own fresh
  `Ready` event is processed (Phase 902's ready-gate) — this is normal, not a regression
  to investigate; do not add code to bypass or pre-fire the new worker's gate.
- The integration test in `managed_tests.rs` is the first test in this crate to kill a
  *real* spawned subprocess (not a pre-built channel) and observe the full
  `Dead → Respawning → Idle` sequence — budget test time accordingly and use a generous
  but bounded timeout.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0; `tests/managed_tests.rs` includes a test that kills the worker process and asserts the `Dead → Respawning → Idle` event sequence within the test's timeout.

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

- `P10-A3` depends on the Phase 902 retrofit (`docs/TASKS_PHASE902.md`), which changed
  both `managed.rs` and `keepalive.rs` after `P10-A2` completed — including
  `ManagedWorker::spawn()`'s signature (now five arguments, including a `routes` handle
  the function registers itself into) and a new per-worker `ready_tx`/keepalive-gate
  pair. Read both files fresh from disk before planning `P10-A3`; do not assume their
  shape from `P10-A2`'s plan or implementation reports.
- `child.wait()` must be polled inside the `run()` loop's `tokio::select!` alongside the IPC channel, as one arm among several in a continuing loop — not a separate blocking call, and not in a function that returns after handling it once.
- The respawn delay must be awaited with `tokio::time::sleep`, not `std::thread::sleep`.
- `RespawnPolicy::should_respawn` takes `crash_count` by mutable reference as of Phase 901 — it performs the window-reset itself; do not re-implement reset logic at the call site.
- There is no in-flight job tracking on `ManagedWorker` at this point in the codebase. Do not invent a job-failure callback in `P10-A2` — leave the `// TODO(P14)` comment instead.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every `pub` item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO log points must be present on the Dead and Respawning transitions.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
