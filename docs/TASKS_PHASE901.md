# Tasks: Phase 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit

| Field | Value |
|-------|-------|
| Phase | 901 |
| Name | ManagedWorker Run-Loop and RespawnPolicy Retrofit |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 10 (partially — after P10-A1, before the original P10-A2 work resumes) |

## Overview

Phase 901 is a three-task retrofit correcting two independent defects discovered while
P10-A2 was in progress. Both defects predate Phase 10: they were introduced during Phase 9
(`run()`) and Phase 10 (`should_respawn`), and both went undetected because the test suites
written alongside them were shaped around the broken behaviour rather than the intended
behaviour. P10-A2 could not be completed without first correcting both, because it needed
a `run()` that actually loops and a `should_respawn` that actually resets its crash counter.

**P901-A1** fixes `ManagedWorker::run()` in `crates/anvilml-worker/src/managed.rs`. The
function currently wraps a single `tokio::select! { ready_timeout, event_rx.recv() }` with
no enclosing `loop`. It processes exactly one event from the broadcast channel and then
returns, dropping the bridge and keepalive task handles. A worker that reaches `Idle` after
its first `Ready` event has its supervision torn down immediately — no subsequent `Busy`,
`Completed`, `Dying`, or crash event is ever observed by `run()` again, because the function
has already exited. This is the root cause of the multi-hour stall observed when P10-A2
attempted to add a `child.wait()` branch and a respawn cycle to this function: a one-shot
`select!` cannot host two concurrent, ongoing concerns (event processing and crash
detection) without a wrapping `loop`. The fix wraps the existing `select!` in `loop { ... }`
and removes the `ready_timeout` branch once the worker has left `Initializing`, so it cannot
misfire against an already-running worker.

**P901-A2** updates `managed_tests.rs` to prove the loop fix actually works. The existing
tests send exactly one event then `drop(event_tx)` to force `run()` to exit — a pattern that
was written to fit the one-shot behaviour and does not distinguish a real loop from the
old single-iteration `select!`. A new test sends two sequential events on a single `run()`
invocation and asserts both transitions were applied, which only passes if the loop is real.

**P901-A3** fixes `RespawnPolicy::should_respawn` in `crates/anvilml-worker/src/respawn.rs`.
The current implementation accepts `last_crash: Instant` but never reads it — the parameter
is renamed `_last_crash` — and unconditionally returns `true` whenever `crash_count <
max_attempts`. This contradicts the function's own doc comment, `ANVILML_DESIGN.md §18.4`
(which documents a time-windowed crash budget: a worker that crashes infrequently should
never be permanently barred from respawning), and P10-A1's original acceptance criteria
("outside window resets count"). The existing `test_should_respawn_window_reset` cannot
catch this defect because it only asserts the boolean return value, which is identical
whether or not the reset logic exists. The fix changes the signature to take `crash_count`
by mutable reference so the function performs the reset itself — `should_respawn` becomes
authoritative for the reset contract instead of leaving it as caller-inferred behaviour,
which is exactly the ambiguity that let the original defect ship unnoticed.

All three fixes are confined to `anvilml-worker`. No HTTP-visible behaviour changes; the
respawn cycle that consumes `RespawnPolicy::should_respawn`'s corrected output is built in
the renumbered `P10-A2`/`P10-A3`, not here.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker | P901-A1 … P901-A3 | Fix `run()` to loop continuously; update tests to prove it; fix `should_respawn` window-reset logic |

## Prerequisites

P10-A1 complete. `crates/anvilml-worker/src/managed.rs` exists with the single-iteration
`run()` defect described above. `crates/anvilml-worker/src/respawn.rs` exists with
`should_respawn` implemented per P10-A1 but ignoring `last_crash`.

## Task Descriptions

### Group A — anvilml-worker

#### P901-A1: anvilml-worker: managed.rs fix run() to loop continuously instead of returning after one event

**Goal:** Wrap the existing `tokio::select!` in `run()` in a `loop { ... }` so the function
continues processing broadcast events for the worker's full lifetime instead of returning
after the first one.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — wrap `select!` in `loop`; scope `ready_timeout` to the `Initializing` state only

**Key implementation notes:**
- The `ready_timeout` branch must not be live once the worker has left `Initializing` — a worker that has been `Idle` for an hour must not suddenly transition to `Dead` because a stale 60-second timer fires on a later loop iteration. Either re-arm the timeout only while `Initializing`, or gate the branch's effect on the current status read at fire time.
- `break` only on `Err(broadcast::error::RecvError::Closed)`. Do not `break` after successfully processing an event — that is the exact defect being fixed.
- This task does not touch `child.wait()` or respawn logic — those are P10-A2/P10-A3, sequenced after this retrofit completes.
- It is expected and acceptable for existing `managed_tests.rs` tests to behave differently after this change (some relied on single-event exit timing); P901-A2 addresses the test suite.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0. (If an existing test's timing assumption breaks here, it is corrected in P901-A2, not reverted here.)

---

#### P901-A2: anvilml-worker: managed_tests.rs update tests for the now-continuous run() loop

**Goal:** Add a test that proves `run()` processes more than one event per invocation, since no existing test in the suite can distinguish "loops correctly" from "happened to work once."

**Files to create or modify:**
- `crates/anvilml-worker/tests/managed_tests.rs` — add `test_run_processes_multiple_sequential_events`

**Key implementation notes:**
- Spawn `run()` once. Send a `Ready` event, wait briefly, assert `Idle`. Then send a second event that should transition status again (e.g. a `Completed`/`Failed`/`Cancelled` event from a manually-set `Busy` status, mirroring the existing `test_managed_worker_processes_completed_event` pattern). Assert the second transition is observed, all on the same `run()` invocation, before dropping `event_tx`.
- Do not modify `managed.rs` in this task — it is test-only.
- Existing tests' `drop(event_tx)`-to-exit pattern remains valid (the loop still breaks on `Closed`); no rewrite is required there unless a test's timing assumption broke in P901-A1, in which case fix the timing here.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0, including the new multi-event test.

---

#### P901-A3: anvilml-worker: respawn.rs fix should_respawn to honor last_crash/window_s instead of ignoring them

**Goal:** Make `RespawnPolicy::should_respawn` actually use its `last_crash` parameter to reset the crash counter once the time window has elapsed, instead of silently ignoring it.

**Files to create or modify:**
- `crates/anvilml-worker/src/respawn.rs` — change `should_respawn` signature and implementation
- `crates/anvilml-worker/tests/respawn_tests.rs` — update `test_should_respawn_window_reset` to assert the counter reset, not just the boolean

**Key implementation notes:**
- New signature: `pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool`. The function becomes the single source of truth for the reset decision — callers no longer infer or separately implement "should I reset the counter."
- Logic: if `last_crash.elapsed() >= Duration::from_secs(self.window_s)`, set `*crash_count = 0`. Then if `*crash_count >= self.max_attempts`, return `false`. Otherwise increment `*crash_count` and return `true`.
- `test_should_respawn_window_reset` must construct a policy, call with a `crash_count` close to `max_attempts` and a `last_crash` older than `window_s`, and assert both that the call returns `true` and that `crash_count` was reset to a low value (e.g. `1`, post-increment) — not merely that the call returned `true`, which the old defect also satisfied.
- No production caller exists yet outside tests (the caller is added in the renumbered `P10-A3`), so this is a contained signature change with no other call sites to update.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 with ≥ 4 tests, and `test_should_respawn_window_reset` is written so it would fail against the prior always-`true`, never-reset implementation.

---

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-worker --features mock-hardware
cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings
cargo fmt --all -- --check
```

## Known Constraints and Gotchas

- This phase intentionally does not touch `child.wait()`, crash detection, or the respawn cycle — those remain in the renumbered `P10-A2` and new `P10-A3`, which prereq the final task of this phase (`P901-A3`).
- The `ready_timeout`-scoping note in P901-A1 is the most likely place for a subtle regression: test manually that a worker sitting `Idle` for longer than 60 seconds does not spuriously transition to `Dead`.
- Follow `FORGE_AGENT_RULES.md §12` for inline documentation and `§11` for logging on any new branch added to `run()`.
