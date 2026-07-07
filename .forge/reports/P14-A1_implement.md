# Implementation Report: P14-A1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P14-A1                                      |
| Phase         | 14 — Dispatch & Execute                     |
| Description   | anvilml-scheduler: JobScheduler struct + submit() |
| Implemented   | 2026-07-07T14:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `JobScheduler` struct and its `submit()` method in `crates/anvilml-scheduler/src/scheduler.rs`. The scheduler owns an in-memory `JobQueue`, a `VramLedger`, a `JobStore` for persistence, a `NodeTypeRegistry` for graph validation, and a `Notify` for waking the dispatch loop. `submit()` enforces the "no workers = reject" guard, validates the graph, constructs a `Queued` job, persists it via `JobStore::upsert()`, enqueues it, notifies the dispatch loop, and returns the job ID. Four integration tests verify the full submit flow.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | tokio   | 1.52.3           | rust-docs MCP  |

The `tokio` version 1.52.3 was verified via `rust-docs get_crate_version` — it is the latest stable release (2026-05-08, 38.4M downloads, MIT license, MSRV 1.71). Features confirmed: `sync` (provides `tokio::sync::Mutex` and `tokio::sync::Notify`), `macros` (enables `#[tokio::test]`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/scheduler.rs` | `JobScheduler` struct, `new()` constructor, `submit()` async method with full doc comments and structured logging |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod scheduler;` and `pub use scheduler::JobScheduler;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Added `tokio`, `chrono`, `tracing` dependencies; added `sqlx` dev-dependency; bumped version 0.1.9 → 0.1.10 |
| CREATE | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | 4 integration tests for `submit()` |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for scheduler tests |

## Commit Log

```
 .forge/reports/P14-A1_plan.md                     | 188 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   5 +-
 crates/anvilml-scheduler/Cargo.toml               |   7 +-
 crates/anvilml-scheduler/src/lib.rs               |   2 +
 crates/anvilml-scheduler/src/scheduler.rs         | 219 ++++++++++++++++++++++
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 216 +++++++++++++++++++++
 docs/TESTS.md                                     |  48 +++++
 9 files changed, 693 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-5d81900f58f97f0c)

running 4 tests
test test_submit_empty_registry_returns_workers_unavailable ... ok
test test_submit_invalid_graph_returns_validation_error ... ok
test test_submit_valid_persists_and_queues ... ok
test test_two_submits_get_distinct_ids ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Full workspace test suite: 282 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

1. Mock-hardware Linux: `cargo check --workspace --features mock-hardware` — **exit 0**
2. Mock-hardware Windows: `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — **exit 0**
3. Real-hardware Linux: `cargo check --bin anvilml` — **exit 0**
4. Real-hardware Windows: `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — **exit 0**

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` — **exit 0**, 1 test passed.

## Public API Delta

```
+pub mod scheduler;
+pub use scheduler::JobScheduler;
```

New `pub` items in `scheduler.rs`:
- `pub struct JobScheduler` — the central async dispatcher
- `pub fn new(job_store: JobStore, node_registry: Arc<NodeTypeRegistry>) -> Self` — constructor
- `pub async fn submit(&self, graph: serde_json::Value, settings: JobSettings) -> Result<Uuid, AnvilError>` — submit method

All match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- `tokio` version 1.52.3 confirmed via MCP (same as planned)
- `JobScheduler` struct fields match the plan's definition
- `new()` constructor matches the plan
- `submit()` follows the exact 7-step sequence (workers check → validate → construct → persist → enqueue → notify → return)
- `#[allow(dead_code)]` on the struct for the `ledger` field (deferred to P14-A3 dispatch loop) — this is a necessary deviation since the field exists but is not yet used

## Blockers

None.
