# Implementation Report: P14-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-C1                                        |
| Phase       | 14 — Dispatch & Execute                      |
| Description | anvilml-server: AppState gains scheduler/workers/db fields |
| Implemented | 2026-07-07T22:30:00Z                         |
| Status      | COMPLETE                                     |

## Summary

Extended the `AppState` struct in `crates/anvilml-server/src/state.rs` with three new fields — `scheduler: Arc<JobScheduler>`, `workers: Arc<WorkerPool>`, and `db: SqlitePool` — and wired them into the backend binary's startup sequence. Added `sqlx` as a compile-time dependency and `chrono`/`uuid` as dev-dependencies. Updated all existing integration tests in `health_tests.rs` and `nodes_tests.rs` to construct the new fields, and wrote three new integration tests in `state_tests.rs` covering construction, clone semantics, and Arc-sharing. Bumped `anvilml-server` crate version from `0.1.4` to `0.1.5`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | sqlx    | 0.9.0            | rust-docs MCP  |
| crate  | chrono  | 0.4 (workspace)  | Cargo.lock     |
| crate  | uuid    | 1.23.4 (workspace)| Cargo.lock    |

The `sqlx` version matches the version used by `anvilml-registry`'s `Cargo.toml`. Feature flags `sqlite`, `runtime-tokio`, `migrate`, and `chrono` confirmed via rust-docs MCP.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/Cargo.toml | Add `sqlx` dependency; add `chrono` and `uuid` dev-deps; bump version 0.1.4 → 0.1.5 |
| Modify | crates/anvilml-server/src/state.rs | Add `scheduler`, `workers`, `db` fields to `AppState` with doc comments |
| Modify | crates/anvilml-server/tests/state_tests.rs | Add 3 new tests + refactor existing tests for new fields |
| Modify | crates/anvilml-server/tests/health_tests.rs | Add `make_test_state()` helper for new fields |
| Modify | crates/anvilml-server/tests/nodes_tests.rs | Add `make_test_state()` helper for new fields |
| Modify | backend/Cargo.toml | Add `anvilml-worker` dependency |
| Modify | backend/src/main.rs | Wire `scheduler`, `workers`, `db` into `AppState` construction |
| Modify | docs/TESTS.md | Add 3 entries for new tests |
| Modify | Cargo.lock | Updated by cargo |

## Commit Log

```
 .forge/reports/P14-C1_plan.md               | 144 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   7 +-
 backend/Cargo.toml                          |   3 +-
 backend/src/main.rs                         |  28 +++-
 crates/anvilml-server/Cargo.toml            |   6 +-
 crates/anvilml-server/src/state.rs          |  26 ++++
 crates/anvilml-server/tests/health_tests.rs |  57 +++++++-
 crates/anvilml-server/tests/nodes_tests.rs  |  90 ++++++++-----
 crates/anvilml-server/tests/state_tests.rs  | 194 +++++++++++++++++++++++++++-
 docs/TESTS.md                               |  36 ++++++
 12 files changed, 551 insertions(+), 59 deletions(-)
```

## Test Results

```
Running tests/state_tests.rs (target/debug/deps/state_tests-5703f2c573ea5960)

running 5 tests
test test_app_state_with_new_fields ... ok
test test_app_state_scheduler_arc_sharing ... ok
test test_app_state_clone_preserves_all_fields ... ok
test test_app_state_clone_shares_node_registry ... ok
test test_app_state_constructs ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

All 180+ workspace tests pass. The `state_tests.rs` file now contains 5 tests (2 pre-existing + 3 new).

## Format Gate

```
(cargo fmt --all -- --check exits 0 — no output means clean)
```

## Platform Cross-Check

All four checks passed:

1. `cargo check --workspace --features mock-hardware` — OK
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — OK
3. `cargo check --bin anvilml` — OK
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — OK

## Project Gates

**Gate 1 — Config Surface Sync:** `cargo test -p anvilml --features mock-hardware -- config_reference` — OK (1 passed)

**Gate 2 — OpenAPI Drift:** Not triggered — this task does not modify handler signatures, `#[utoipa::path]` annotations, or `ToSchema` derives.

**Gate 3 — Node Parity:** Not triggered — no node types added/removed/renamed.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()` modified.

## Public API Delta

```
+    pub scheduler: Arc<JobScheduler>,
+    pub workers: Arc<WorkerPool>,
+    pub db: SqlitePool,
```

Three new `pub` fields on `pub struct AppState` in module `anvilml_server::state`. No new functions, traits, or types introduced.

## Deviations from Plan

1. **Backend wiring was not deferred to P14-C2.** The plan stated "Wiring `backend/main.rs` to construct and spawn a real `WorkerPool` and `JobScheduler` — explicitly deferred to P14-C2." However, adding fields to `AppState` makes the struct incomplete without construction values, causing a compile error. The backend was updated to construct minimal stub values for `scheduler`, `workers`, and `db` at startup. This is a compile-time requirement, not a feature — the values are minimal stubs (empty pool, in-memory DB) and the real wiring (device-based worker spawning, dispatch loop) remains deferred to P14-C2.

2. **Existing test files required updates.** The plan's `## Files Affected` table listed only `state_tests.rs` for test modifications. However, `health_tests.rs` and `nodes_tests.rs` also construct `AppState` inline, so they required the same field additions. Helper functions (`make_test_state()`) were added to each file to avoid repetition.

3. **`anvilml-worker` dependency added to backend.** The backend's `Cargo.toml` did not previously include `anvilml-worker`. Adding it was necessary for the `WorkerPool` construction in `main.rs`.

4. **`test_app_state_constructs` test updated.** The original synchronous test could not construct `WorkerPool::new()` without an async runtime. A synchronous helper using `tokio::runtime::Builder::new_current_thread()` was added to support this.

## Blockers

None.
