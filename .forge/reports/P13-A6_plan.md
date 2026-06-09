# Plan Report: P13-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-A6                                      |
| Phase       | 013 — Dispatch & Execute                    |
| Description | anvilml: start dispatch loop at startup; verify job reaches Completed |
| Depends on  | P13-A1, P13-A2, P13-A3, P13-A4, P13-A5      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-09T12:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the dispatch loop into the AnvilML server startup sequence so that submitted jobs
are automatically dispatched to idle workers. The dispatch loop was fully implemented in
P13-A3 (queue polling, worker selection, Execute IPC send) and P13-A5 (Completed/Failed
handling), and the mock executor in the Python worker was implemented in P13-A4 — but
`main.rs` never actually called `scheduler.start_dispatch_loop()`. This task adds that
single call, enabling the end-to-end flow: POST a ZiT graph → Queued → Running → Completed.

## Scope

### In Scope
- Add `scheduler.start_dispatch_loop()` call in `backend/src/main.rs` after the
  `JobScheduler` is constructed and before `build_router()`.
- Store the returned `JoinHandle<()>` to keep the task alive for the lifetime of the
  server process (the handle is dropped when `main()` returns, which is the desired
  lifecycle — the loop runs alongside the server).
- Verify that the existing logging (§11.3/INFO: "dispatch loop started", §11.5/DEBUG:
  "job dispatched to worker") is present in the scheduler code.

### Out of Scope
- Any changes to the dispatch loop logic itself (already complete in P13-A3/A5).
- Any changes to the mock executor (already complete in P13-A4).
- Any changes to worker pool spawning, IPC, or hardware detection.
- Adding new tests — the dispatch loop's unit tests already live in
  `crates/anvilml-scheduler/src/scheduler.rs` (`test_dispatch_sends_execute` and
  `test_complete`), which exercise the full queue → dispatch → execute path.

## Approach

1. **Read `backend/src/main.rs`** to confirm the current location where the
   `JobScheduler` is constructed (line 255–262) and where `build_router()` is called
   (line 275).

2. **Insert the dispatch loop start** between the scheduler construction and the
   `build_router()` call. The pattern mirrors `spawn_system_stats_tick()` — the
   dispatch loop is a `tokio::spawn`-ed task whose `JoinHandle` is stored in a
   variable (not awaited or dropped early).

   ```rust
   // After: let scheduler = Arc::new(JobScheduler::new(...));
   let _dispatch_handle = scheduler.start_dispatch_loop();
   tracing::info!("dispatch loop started");
   ```

   Note: `start_dispatch_loop()` already emits its own `tracing::info!("dispatch loop
   started")` at line 159 of `scheduler.rs`, so the explicit log in main.rs is optional.
   We keep it minimal — just the call itself. The `_dispatch_handle` binding prevents
   premature drop.

3. **Verify logging compliance** (§11.3 and §11.5 of FORGE_AGENT_RULES):
   - §11.3/INFO: `"dispatch loop started"` — present in `scheduler.rs:159` ✓
   - §11.5/DEBUG: `"job dispatched to worker"` with `job_id=`, `worker_id=` — present in
     `scheduler.rs:276-280` ✓
   - §11.5/DEBUG: `"job status transition"` with `job_id=`, `from=`, `to=` — present in
     `scheduler.rs:103` ✓
   - No new log calls are needed.

4. **Build and test** — `cargo test --workspace --features mock-hardware` must pass.
   The existing `test_dispatch_sends_execute` and `test_complete` tests in
   `scheduler.rs` already cover the dispatch → execute → complete path with the mock
   worker pool.

5. **End-to-end verification** (Runnable Proof from TASKS_PHASE013.md):
   ```bash
   ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv \
     cargo run --features mock-hardware
   JOB=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs \
     -H 'content-type: application/json' \
     -d @valid_zit_job.json | python3 -c 'import sys,json;print(json.load(sys.stdin)["job_id"])')
   for i in $(seq 1 10); do
     curl -s http://127.0.0.1:8488/v1/jobs/$JOB | python3 -c 'import sys,json;print(json.load(sys.stdin)["status"])'
     sleep 1
   done
   ```
   Expected: status prints `Queued` → `Running` → `Completed` within a few seconds.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add `scheduler.start_dispatch_loop()` call after scheduler construction (1 line + 1 binding) |

No crate version bump required — the modified file is in `backend/` (not a `crates/*`
crate), and the version bump convention (§12 of FORGE_AGENT_RULES, §10 of ENVIRONMENT.md)
applies to crates under `crates/`. The `backend` crate's patch version is NOT bumped for
this task because `backend` is the binary crate, not a library crate in the `crates/`
directory. (Verification: `backend/Cargo.toml` exists but `backend/src/` contains only
`main.rs`, `cli.rs`, `shutdown.rs`, `preflight.rs` — no library code.)

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/scheduler.rs` | `test_dispatch_sends_execute` | Queued job → dispatch → Running status, worker Busy, queue empty |
| `crates/anvilml-scheduler/src/scheduler.rs` | `test_complete` | Running job + Completed event → Completed status, worker Idle |

No new test files are added. The existing tests in the scheduler module already exercise
the dispatch loop end-to-end with a mock worker pool. The only change in this task is
starting the loop in `main.rs`, which cannot be unit-tested in isolation (it requires a
full server lifecycle) but is verified by the Runnable Proof.

## CI Impact

No CI changes required. The existing CI gates (`cargo test --workspace --features
mock-hardware`, `cargo clippy`, `cargo fmt`) will validate the change. The new code path
is a single function call with no new dependencies, no new cfg attributes, and no new
public API. The `--features mock-hardware` flag already covers this code path.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The dispatch loop starts before workers are ready (race condition) | Low | Medium | Workers are spawned at line 236 (before scheduler construction at 255), so they are always ready. The dispatch loop's `select_worker` returns `None` when no idle worker exists, safely queuing the job. |
| The dispatch loop consumes CPU in a tight loop when idle | Low | Low | The loop uses `tokio::select!` with a 100 ms timeout (scheduler.rs:195) and `Notify` on job submission — no busy-wait. |
| WorkerPool not yet spawned when dispatch loop starts | Very low | High | The spawn order in main.rs is: (1) workers at line 236, (2) scheduler at line 255, (3) dispatch loop at the new call. Workers are always available. |
| Existing tests regress due to the new call | Very low | Medium | The call is a no-op for tests that don't submit jobs; the `_dispatch_handle` is simply dropped at end of `main()`. |

## Acceptance Criteria

- [ ] `backend/src/main.rs` calls `scheduler.start_dispatch_loop()` after the scheduler is constructed
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (zero failures)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (zero warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
- [ ] Runnable Proof passes: submitted ZiT job transitions Queued → Running → Completed within ~5 seconds under `ANVILML_WORKER_MOCK=1`
- [ ] No new files created, no source code written outside `backend/src/main.rs`
