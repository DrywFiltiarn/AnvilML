# Implementation Report: P5-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P5-A3                              |
| Phase         | 005 — SQLite Persistence           |
| Description   | anvilml-registry: open_in_memory for test isolation |
| Implemented   | 2026-06-15T17:05:00Z               |
| Status        | COMPLETE                           |

## Summary

Converted `test_ghost_job_reset` and `test_ghost_job_noop` in `crates/anvilml-registry/tests/db_tests.rs` to use `open_in_memory()` instead of file-backed temp directories. Both tests now open an in-memory pool, insert test jobs, execute the ghost-job reset SQL directly on the same connection (simulating what `open()` does after migrations), and verify the expected outcome. The two file-specific tests (`test_open_creates_file`, `test_open_wal_mode`) remain file-backed as they test `open()` file-specific behavior. Version bumped `anvilml-registry` from 0.1.2 to 0.1.3. Updated `docs/TESTS.md` entries for both converted tests.

## Resolved Dependencies

None. No new dependencies introduced. The task reuses the existing `open_in_memory()` function from `db.rs` and `sqlx::query` for direct SQL execution.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/tests/db_tests.rs` | Converted `test_ghost_job_reset` and `test_ghost_job_noop` to use `open_in_memory()` with same-connection reset SQL verification; updated doc comments |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bumped patch version 0.1.2 → 0.1.3 |
| Modify | `docs/TESTS.md` | Updated test catalogue entries for `test_ghost_job_reset` and `test_ghost_job_noop` to reflect in-memory pool implementation |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 +--
 .forge/state/state.json                   | 13 +++---
 Cargo.lock                                |  2 +-
 crates/anvilml-registry/Cargo.toml        |  2 +-
 crates/anvilml-registry/tests/db_tests.rs | 73 ++++++++++++++++++-------------
 docs/TESTS.md                             | 12 ++---
 6 files changed, 60 insertions(+), 48 deletions(-)
```

## Test Results

```
     Running tests/db_tests.rs (target/debug/deps/db_tests-f0d82d45dcae6351)

running 5 tests
test test_ghost_job_reset ... ok
test test_open_in_memory ... ok
test test_ghost_job_noop ... ok
test test_open_creates_file ... ok
test test_open_wal_mode ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/seed_loader_tests.rs (target/debug/deps/seed_loader_tests-cf620f0b77485c06)

running 2 tests
test test_seed_loader_skips_up_to_date ... ok
test test_seed_loader_applies_new_seed ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
---CHECK1: OK---

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.17s
---CHECK2: OK---

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.76s
---CHECK3: OK---

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
---CHECK4: OK---
```

## Project Gates

Gate 1 — Config Surface Sync:
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

Gate 2 — OpenAPI Drift: Not applicable — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

Gate 3 — Node Parity: Not applicable — task does not add, remove, or rename node types in `worker/nodes/` or modify `crates/anvilml-scheduler/src/node_registry.rs`.

## Public API Delta

```
(no output — grep returned nothing)
```

No new pub items introduced. The task only modified test code and a Cargo.toml version field.

## Deviations from Plan

None. Implementation followed the approved plan exactly:
- `test_open_creates_file` and `test_open_wal_mode` kept as-is (file-backed)
- `test_open_in_memory` already used `open_in_memory()` — no changes
- `test_ghost_job_reset` and `test_ghost_job_noop` converted to use `open_in_memory()` with same-connection reset SQL execution
- Doc comments updated for both converted tests
- Version bumped from 0.1.2 to 0.1.3
- `docs/TESTS.md` entries updated

## Blockers

None.
