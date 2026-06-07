# Tasks: Phase 902 — Stabilisation Retrofit

| Field | Value |
|-------|-------|
| Phase | 902 |
| Name | Stabilisation Retrofit |
| Milestone group | Retrofit |
| Project(s) | anvilml |
| Status | Draft |
| Depends on phases | 0–12 (via P901-A1) |
| Task file | `.forge/tasks/tasks_phase902.json` |
| Tasks | 6 |

---

## Overview

Phase 902 is a retrofit phase inserted between Phase 12 and Phase 13. It resolves three categories of accumulated debt before the dispatch loop (Phase 13) builds further on the scheduler and worker subsystems.

**What the audit found:**

Running `cargo clippy --workspace --features mock-hardware -- -D warnings -W dead_code -W unused_imports -W unused_variables` produced zero warnings and zero errors. All `#[allow(…)]` suppressions in the codebase are load-bearing (held references, cross-platform `mut`, phase-deferred infrastructure). None should be removed. The Python worker has 11 passing tests and no defects. There is no dead code cleanup work in this phase.

The actual debt is:

1. **Test isolation** — four spawning tests in `anvilml-worker` use `std::env::set_var` with `#[serial_test::serial]` as a workaround for process-global env-var contamination. `serial_test` serialises all four tests, bottlenecking future worker test suites and masking that the real fix is scoped env isolation.
2. **Logging gaps** — phases 9–12 added the IPC bridge, worker pool, job store, queue, and scheduler. The §11.5 mandatory DEBUG log points (IPC send/receive, job dispatch, job state transitions) were not added to these subsystems. Phase 900 covered phases 0–8; this phase covers phases 9–12.
3. **Rule 4.6 formalisation** — the `refactor` tag and its agent-facing rule do not yet exist in `FORGE_AGENT_RULES.md`. Adding this in the same phase that uses it ensures the rule is in place before any future refactor tasks run.

**Phase 13 dependency:** `P13-A1` currently prereqs `["P901-A1"]`. Before Phase 902 runs, update `tasks_phase013.json` so that `P13-A1` prereqs `["P902-D1"]`. This is a manual change made outside The Forge — it must be committed before The Forge picks up Phase 902.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker | P902-A1 … A3 | env isolation fix; IPC DEBUG log points; pool spawn/status DEBUG log points |
| B | anvilml-scheduler | P902-B1 … B2 | scheduler submit DEBUG point; job-store and queue DEBUG points |
| D | Gate | P902-D1 | Full workspace clean gate — no source changes, verbatim output only |

---

## Prerequisites

All tasks in phases 000 through 012 must be complete. `P12-A5` must be pushed. `tasks_phase013.json` must have `P13-A1.prereqs` updated to `["P902-D1"]` before The Forge starts this phase.

---

## Interfaces and Contracts

No external contract documents are required by this phase. All changes are internal implementation details (log calls and test scoping). No public API surface changes.

---

## Task Descriptions

### Group A — anvilml-worker

#### P902-A1: Replace serial_test env-var workaround with scoped env isolation

**File:** `crates/anvilml-worker/src/managed.rs`, `crates/anvilml-worker/Cargo.toml`

The four spawning tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) call `std::env::set_var("ANVILML_WORKER_MOCK", "1")` to enter mock mode. Because `set_var` is process-global, Cargo's parallel test runner can cause one test to read the env var set by another. P11-C2 applied `#[serial_test::serial]` as a workaround. The correct fix is scoped env mutation using the `temp_env` crate: each test wraps its body in `temp_env::with_var("ANVILML_WORKER_MOCK", Some("1"), async { … })`, making the env change visible only within that closure. The `serial_test` dep and all four `#[serial]` attributes are then removed.

**Acceptance criterion:** `env -i HOME=$HOME PATH=$PATH cargo test -p anvilml-worker --features mock-hardware` exits 0 with all 16 tests passing. The `-i` flag clears the ambient environment, proving no test relies on an externally set `ANVILML_WORKER_MOCK`.

#### P902-A2: Retrofit mandatory IPC DEBUG log points (managed.rs)

**File:** `crates/anvilml-worker/src/managed.rs`

The writer task (which sends `WorkerMessage` frames to the worker stdin) and the reader task (which deserialises `WorkerEvent` frames from stdout) are both missing the §11.5 mandatory IPC DEBUG log points. `msg_discriminant()` and `event_discriminant()` helper functions already exist in the file — the log calls reference them directly.

No logic changes. No new helper functions.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

#### P902-A3: Retrofit mandatory spawn and status-transition DEBUG log points (pool.rs)

**File:** `crates/anvilml-worker/src/pool.rs`

`spawn_all()` creates workers but logs nothing at DEBUG when each worker is created. `set_busy()` and `set_idle()` log at INFO (correct per §11.3) but are missing the DEBUG transition log showing the `from` and `to` status values required by §11.5. Both the GPU worker creation loop and the CPU fallback path need the spawn DEBUG call.

No logic changes. The existing INFO calls in `set_busy` and `set_idle` are not changed.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

### Group B — anvilml-scheduler

#### P902-B1: Retrofit mandatory job state-transition DEBUG log point (scheduler.rs)

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

`submit()` already has `tracing::info!(job_id = %job_id)` and is decorated with `#[tracing::instrument]`, satisfying the §11.3 INFO requirements. It is missing the §11.5 mandatory DEBUG job state-transition point (status transition to Queued). The dispatch loop's Running transition point belongs in Phase 13, not here.

No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0.

#### P902-B2: Retrofit mandatory job-store and queue DEBUG log points (job_store.rs, queue.rs)

**Files:** `crates/anvilml-scheduler/src/job_store.rs`, `crates/anvilml-scheduler/src/queue.rs`

`insert_job()` and `update_status()` in `job_store.rs` have no DEBUG log calls. `enqueue()` and `pop_next()` in `queue.rs` have no DEBUG log calls. These are all required by §11.5 (job scheduler: job dispatched, job state transition). `tracing` is already a declared dependency of `anvilml-scheduler` (added in P12-A3).

No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0.

---

### Group D — Gate

#### P902-D1: Full workspace stabilisation gate

**No files modified.**

Runs all four verification commands and records verbatim output as the implementation report. This task exists to produce a single auditable checkpoint before Phase 13 begins. It is the prereq that `P13-A1` chains through.

Commands:
```bash
# 1. Lint — zero warnings required
cargo clippy --workspace --features mock-hardware -- -D warnings

# 2. Tests — zero failures required, ambient env cleared
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware

# 3. Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 4. Python worker
python -m pytest worker/tests/ -v
```

**Acceptance criterion:** All four commands exit 0. Report contains verbatim output of each command.

---

## Phase Acceptance Criteria

```bash
# Rust lint
cargo clippy --workspace --features mock-hardware -- -D warnings

# Rust tests — ambient env cleared to prove env isolation
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware

# Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# Python worker
python -m pytest worker/tests/ -v
```

All four must exit 0. Phase is complete when P902-D1 is committed and the above pass locally.

---

## Known Constraints and Gotchas

- **P902-A1: temp_env async usage.** `temp_env::with_var` has both sync and async variants. The tests are `#[tokio::test]` async functions — use `temp_env::async_with_var` (from the `temp_env` crate's `async` feature) or restructure as `tokio::task::spawn_blocking` if the async variant is unavailable. Check the `temp_env` crate docs via `rust-docs` MCP before writing the implementation. If `temp_env` does not support async, the alternative is an inline RAII guard using `std::env::set_var` / `remove_var` in a `Drop` impl — document the choice in the plan report.
- **P902-A1: `-i` env clear test.** The acceptance criterion uses `env -i`, which is a Unix-only invocation. On Windows CI (which runs under GitHub Actions) the equivalent is not available — the Windows CI job relies on the test suite itself not setting `ANVILML_WORKER_MOCK` in the environment before running. This is acceptable: the fix is verified on Linux locally and on Windows via CI.
- **P902-A2: writer task location.** The writer task is the `run_loop` tokio task in `managed.rs`. The exact line where the frame is written to stdin is the call site of `framing::write_frame`. The DEBUG log call goes immediately before that write. Do not add it inside `framing::write_frame` itself — that function lives in `anvilml-ipc` and has its own logging scope.
- **P902-B2: queue_len after enqueue.** Call `self.len()` after the push so the logged count reflects the post-enqueue state.
- **P902-D1: ANVILML_VENV_PATH for test command.** The path `./worker/.venv` assumes the venv was created in the worker directory by the CI setup. If the local venv lives elsewhere, substitute the correct path. The gate command is documentation of intent — the agent substitutes the actual path from `ENVIRONMENT.md §2`.
- **P13-A1 prereq must be updated manually** from `["P12-A5"]` to `["P902-D1"]` in `tasks_phase013.json` before The Forge runs Phase 902. This is the human author's responsibility, not the agent's.