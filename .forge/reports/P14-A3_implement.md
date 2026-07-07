# Implementation Report: P14-A3

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P14-A3                                            |
| Phase         | 14 — Dispatch & Execute                           |
| Description   | anvilml-scheduler: dispatch loop skeleton, notify-driven wake |
| Implemented   | 2026-07-07T16:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented the dispatch loop skeleton for the `JobScheduler` in `anvilml-scheduler`. Added `start_dispatch_loop()` — a public async method that spawns a tokio task looping on `dispatch_notify.notified()`, collecting queued jobs, and calling `dispatch_one()` for each. The `dispatch_one()` stub always returns `false` (deferred to P14-A4). Added `#[allow(dead_code)]` on the `ledger` field (deferred VRAM operations in P14-A4). Added 3 new tests proving the loop returns a live JoinHandle, wakes on submit, and survives multiple consecutive wakes. All 176 workspace tests pass (11 in scheduler_tests).

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | tokio   | 1.52.3           | rust-docs MCP  |

The `rt` feature was added to the tokio dependency in `Cargo.toml` (previously only `sync` and `macros`). Confirmed via rust-docs MCP that tokio 1.52.3 includes the `rt` feature which provides `tokio::task::spawn`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Added `rt` feature to tokio dependency; bumped version 0.1.11 → 0.1.12 |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `dispatch_one()` stub (private, defers_to P14-A4); added `start_dispatch_loop()` (public); added `JoinHandle` import; moved `#[allow(dead_code)]` from struct to `ledger` field |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Added 3 new tests: `test_dispatch_loop_returns_join_handle`, `test_submit_wakes_dispatch_loop`, `test_dispatch_loop_survives_multiple_wakes` |
| Modify | `docs/TESTS.md` | Added entries for the 3 new tests |

## Commit Log

```
 .forge/reports/P14-A3_plan.md                     | 517 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   2 +-
 crates/anvilml-scheduler/Cargo.toml               |   4 +-
 crates/anvilml-scheduler/src/scheduler.rs         | 102 ++++-
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 151 +++++++
 docs/TESTS.md                                     |  36 ++
 8 files changed, 818 insertions(+), 13 deletions(-)
```

## Test Results

```
Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-c0fbf6d2c918d733)

running 11 tests
test test_cancel_unknown_id_returns_false ... ok
test test_submit_empty_registry_returns_workers_unavailable ... ok
test test_cancel_queued_job_returns_true ... ok
test test_get_job_unknown_id_returns_none ... ok
test test_submit_invalid_graph_returns_validation_error ... ok
test test_submit_valid_persists_and_queues ... ok
test test_get_job_returns_persisted_job ... ok
test test_two_submits_get_distinct_ids ... ok
test test_dispatch_loop_returns_join_handle ... ok
test test_submit_wakes_dispatch_loop ... ok
test test_dispatch_loop_survives_multiple_wakes ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 176 tests passed, 0 failed.

## Format Gate

```
(Not applicable — task wrote no source files that would trigger unformatted code)
cargo fmt --all -- --check exited 0 with no output.
```

## Platform Cross-Check

```
1. Mock-hardware Linux:    cargo check --workspace --features mock-hardware — Finished
2. Mock-hardware Windows:  cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu — Finished
3. Real-hardware Linux:    cargo check --bin anvilml — Finished
4. Real-hardware Windows:  cargo check --bin anvilml --target x86_64-pc-windows-gnu — Finished
All four checks exited 0.
```

## Project Gates

```
Gate 1 (config_reference): cargo test -p anvilml --features mock-hardware -- config_reference — ok. 1 passed
Gate 2 (OpenAPI drift): Not triggered — no handler signatures or ToSchema derives modified.
Gate 3 (Node parity): Not triggered — no node types added/removed/renamed.
Gate 4 (Mock/Real parity markers): Not triggered — no node execute() or arch module load()/sample()/decode() modified.
```

## Public API Delta

```
+    pub fn start_dispatch_loop(
```

One new `pub` item: `start_dispatch_loop` on `anvilml_scheduler::JobScheduler`. Signature:
`pub fn start_dispatch_loop(self: Arc<Self>, workers: Arc<anvilml_worker::WorkerPool>) -> JoinHandle<()>`

The `dispatch_one` method is private (not `pub`), as specified in the plan.

## Deviations from Plan

- **`#[allow(dead_code)]` placement:** The plan stated to remove `#[allow(dead_code)]` from the `ledger` field. However, the dispatch loop stub does not actually use the ledger (VRAM operations are deferred to P14-A4). Clippy reported `field 'ledger' is never read`. I moved the `#[allow(dead_code)]` annotation from the struct level (where it was) to the `ledger` field level specifically, keeping the struct free of the annotation while suppressing the warning on the field. This is a minimal correct fix.
- **Test Arc wrapping:** The plan's test code used `scheduler.start_dispatch_loop(...)` directly. Since `start_dispatch_loop` takes `self: Arc<Self>`, the tests needed to wrap the scheduler in `Arc::new(scheduler)` and clone the Arc for the call. This was corrected during implementation.
- **dispatch_one loop design:** The plan's initial approach collected all jobs, dispatched them, and pushed remaining back. I refined the implementation to use `dispatched_count` tracking (iterating by reference) to avoid consuming the Vec prematurely, then using `skip(dispatched_count)` to push only un-dispatched jobs back. This is functionally equivalent but avoids a borrow-of-moved-value error.

## Blockers

None.
