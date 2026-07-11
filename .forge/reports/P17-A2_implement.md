# Implementation Report: P17-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-A2                          |
| Phase         | 17 — Cancellation               |
| Description   | anvilml-scheduler: cancel() sends WorkerMessage::CancelJob for Running jobs |
| Implemented   | 2026-07-11T12:30:00Z           |
| Status        | COMPLETE                        |

## Summary

Completed the `Running` branch of `JobScheduler::cancel()` by adding a `transport: Arc<RouterTransport>` field to the scheduler and implementing the cooperative `WorkerMessage::CancelJob` IPC send. The scheduler now sends a cancellation signal to the job's assigned worker via the ZeroMQ transport, handles the edge case of a Running job with no `worker_id`, and returns `Ok(true)` regardless of send success/failure (cancellation is accepted even if the signal doesn't reach the worker). Added 4 new tests and updated the existing `test_cancel_running_job_returns_true_no_ipc` test (renamed to `test_cancel_running_job_sends_cancel_signal`). All 32 scheduler tests pass, plus all workspace tests.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | anvilml-ipc | (existing dep) | —            |

No new external dependencies introduced. The task uses existing `anvilml-ipc::RouterTransport` already in the dependency graph.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `transport` field, update `new()` signature (4th param), implement Running branch IPC send, remove defers_to stub comment |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 4 new tests, update existing cancel test, refactor all `JobScheduler::new` calls to pass transport |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Update all `JobScheduler::new` calls to pass pool transport (14 occurrences) |
| Modify | `backend/src/main.rs` | Pass pool transport to `JobScheduler::new()` |
| Modify | `crates/anvilml-server/tests/health_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/nodes_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/cors_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/artifacts_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/handler_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Update `JobScheduler::new` call |
| Modify | `crates/anvilml-server/tests/state_tests.rs` | Update `JobScheduler::new` calls (2 occurrences) |

## Commit Log

```
 .forge/reports/P17-A2_plan.md                      | 222 ++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  11 +-
 Cargo.lock                                         |   2 +-
 backend/src/main.rs                                |   1 +
 crates/anvilml-scheduler/Cargo.toml                |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs          |  78 ++-
 crates/anvilml-scheduler/tests/event_loop_tests.rs | 146 ++++-
 crates/anvilml-scheduler/tests/scheduler_tests.rs  | 609 +++++++++++++++++----
 crates/anvilml-server/tests/artifacts_tests.rs     |  12 +-
 crates/anvilml-server/tests/cors_tests.rs          |  12 +-
 crates/anvilml-server/tests/handler_tests.rs       |  12 +-
 crates/anvilml-server/tests/health_tests.rs        |  12 +-
 crates/anvilml-server/tests/jobs_tests.rs          |  12 +-
 crates/anvilml-server/tests/nodes_tests.rs         |  12 +-
 crates/anvilml-server/tests/state_tests.rs         |  38 +-
 16 files changed, 990 insertions(+), 197 deletions(-)
```

## Test Results

```
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

New tests added:
- test_cancel_running_sends_cancel_job
- test_cancel_running_status_stays_running
- test_cancel_running_no_worker_id_errors
- test_cancel_running_send_failure_handled

Updated test:
- test_cancel_running_job_returns_true_no_ipc → test_cancel_running_job_sends_cancel_signal

All workspace tests: ok (32 scheduler + all other crates)
```

## Format Gate

```
(cargo fmt --all -- --check exits 0 — no output)
```

## Platform Cross-Check

```
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.63s
```

## Project Gates

None defined for this task (no config surface changes, no OpenAPI changes, no node parity changes).

## Public API Delta

No new `pub` items introduced. The only public API change is the constructor signature:
- `JobScheduler::new()` — added 4th parameter: `transport: Arc<RouterTransport>`

The `transport` field on `JobScheduler` is private (no `pub` modifier).

## Deviations from Plan

- The plan called for 4 new tests; implemented exactly 4 new tests as specified.
- The plan mentioned updating the existing `test_cancel_running_job_returns_true_no_ipc` test's doc comment; renamed it to `test_cancel_running_job_sends_cancel_signal` to better reflect that the IPC send is now active (not deferred).
- The plan's `Files Affected` table listed only 3 files; in practice, 12 files were modified because `JobScheduler::new()`'s signature change required updating all callers across the codebase (scheduler tests, event loop tests, backend/main.rs, and 7 server test files).
- The `event_loop_tests.rs` file had 14 `JobScheduler::new` calls that required reordering (pool creation before scheduler) to access the transport — this was not mentioned in the plan but was necessary for compilation.

## Blockers

None.
