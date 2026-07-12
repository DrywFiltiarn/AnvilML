# Implementation Report: P18-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-D1                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | anvilml-server: GET /v1/workers list handler |
| Implemented   | 2026-07-12T12:45:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented the `GET /v1/workers` HTTP handler that returns the current state of all
Python worker subprocesses as a JSON array of `WorkerInfo` objects. Created the
`workers.rs` handler module with `list_workers()`, registered the route in
`build_router()`, added `pub mod workers;` to the handlers module, wrote three
integration tests verifying pool state reflection, empty-pool handling, and response
shape correctness, and bumped the `anvilml-server` crate version to 0.1.23. All 270
workspace tests pass (267 pre-existing + 3 new).

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | axum      | 0.8.9            | Cargo.lock     |
| crate  | anvilml-worker | 0.1.34 (local) | Local path dep |

No new external dependencies introduced. The task uses only existing types:
`WorkerPool::list()`, `WorkerInfo`, `WorkerStatus`, `DeviceType`, and the
`test-utils` feature on `anvilml-worker` (already declared in Cargo.toml).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/workers.rs` | New handler module with `list_workers()` function |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod workers;` module declaration |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Registered `GET /v1/workers` route in `build_router()` |
| CREATE | `crates/anvilml-server/tests/workers_tests.rs` | 3 integration tests for the workers handler |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped version 0.1.22 → 0.1.23 |
| MODIFY | `docs/TESTS.md` | Added 3 test catalogue entries |

## Commit Log

```
 crates/anvilml-server/Cargo.toml          |  2 +-
 crates/anvilml-server/src/handlers/mod.rs |  1 +
 crates/anvilml-server/src/lib.rs          |  6 ++++++
 crates/anvilml-server/src/handlers/workers.rs  | 33 +++++++++++++++++++
 crates/anvilml-server/tests/workers_tests.rs | 382 +++++++++++++++++++++++++++
 docs/TESTS.md                             | 36 +++++++++++++
 6 files changed, 458 insertions(+), 2 deletions(-)
```

## Test Results

```
running 3 tests
test test_workers_list_empty_returns_empty_array ... ok
test test_workers_response_shape_matches_workerinfo ... ok
test test_workers_list_returns_current_pool_state ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 270 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
CHECK1_PASS

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.92s
CHECK2_PASS

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.49s
CHECK3_PASS

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.75s
CHECK4_PASS
```

All four platform cross-checks passed.

## Project Gates

- **Gate 1 — Config Surface Sync:** Not triggered (no `ServerConfig` field changes).
- **Gate 2 — OpenAPI Drift:** `api/openapi.json` does not yet exist — gate skipped per
  `ENVIRONMENT.md §8`.
- **Gate 3 — Node Parity:** Not triggered (no node type changes).
- **Gate 4 — Mock/Real Parity Markers:** Not triggered (no node `execute()` or arch
  `load()`/`sample()`/`decode()`/`compute_latent_shape()` changes).

## Public API Delta

```
+pub mod workers;
```

Only one new public item: `pub mod workers;` in `crates/anvilml-server/src/handlers/mod.rs`.
The `list_workers` function is `pub(crate)` per established convention — not exposed in
the crate's public API. No new `pub` items in test files.

## Deviations from Plan

- **Field order assertion fix:** The plan's `test_workers_response_shape_matches_workerinfo`
  test asserts exact field order. `serde_json` serializes object keys alphabetically, so
  the assertion was updated to compare keys as a `HashSet` rather than by order. This is
  a test correctness fix, not a deviation from the plan's intent (verifying the six
  expected fields).
- **Removed unused import:** The `WorkerInfo` import was unused in the test file (the test
  uses `serde_json::Value` for JSON assertions, not the Rust type directly). Removed to
  satisfy `clippy -D warnings`.

## Blockers

None.
