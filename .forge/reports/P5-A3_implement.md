# Implementation Report: P5-A3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P5-A3                           |
| Phase         | 005 — SQLite Persistence        |
| Description   | anvilml-registry: ghost-job reset query |
| Implemented   | 2026-06-03T20:18:00Z            |
| Status        | COMPLETE                        |

## Summary

Added the `reset_ghost_jobs` function to `crates/anvilml-registry/src/db.rs` that updates any jobs left in `Running` or `Queued` state (from a previous unclean exit) to `Failed` with `error='server_restart'`. Returns the number of rows affected. Also added a comprehensive inline unit test that verifies exactly 2 ghost jobs are reset while completed jobs remain untouched.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | uuid    | 1               | lockfile      |

Note: `uuid` is an existing dependency of `anvilml-core` (already in workspace). Added as a dev-dependency for `anvilml-registry` to support test UUID generation. No new crates introduced — only reusing existing ones.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Edit | `crates/anvilml-registry/Cargo.toml` | Added `uuid = { version = "1", features = ["v4"] }` to dev-dependencies |
| Edit | `crates/anvilml-registry/src/db.rs` | Added `reset_ghost_jobs()` function and `test_reset_ghost_jobs` unit test |

## Commit Log

```
 .forge/reports/P5-A3_plan.md       | 165 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +--
 Cargo.lock                         |   1 +
 crates/anvilml-registry/Cargo.toml |   1 +
 crates/anvilml-registry/src/db.rs  |  84 +++++++++++++++++++
 6 files changed, 261 insertions(+), 9 deletions(-)
```

## Test Results

### Registry crate tests (Linux)
```
running 2 tests
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.04s

Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-f4ba6c2538c3f7cb)
running 1 test
test test_open_creates_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.02s

Doc-tests anvilml_registry
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.00s
```

### Full workspace test suite (Linux)
```
anvilml_core:    74 passed; 0 failed
anvilml_hardware: 59 passed; 0 failed
anvilml_ipc:      0 passed; 0 failed
anvilml_openapi:  0 passed; 0 failed
anvilml_registry: 2 passed; 0 failed (lib) + 1 passed; 0 failed (integration)
anvilml_scheduler: 0 passed; 0 failed
anvilml_server:   3 passed; 0 failed
anvilml_worker:   0 passed; 0 failed
anvilml (main):   8 passed; 0 failed
config_reference: 1 passed; 0 failed
Doc-tests anvilml_hardware: 2 passed; 0 failed

Total: 150 tests passed; 0 failed.
```

### Windows Cross-Check (x86_64-pc-windows-gnu)
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
```

### Config Drift Gate
```
Running tests/config_reference.rs (target/debug/deps/config_reference-fc5e67c1d0c265e2)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.00s
```

## Deviations from Plan

- **Test UUID binding**: The plan used `uuid::Uuid` directly with `.bind()`, but sqlx 0.9 for SQLite does not implement `Encode`/`Type` for `Uuid` without the `uuid` feature enabled on sqlx itself. Instead, I converted UUIDs to strings via `.to_string()` before binding. This is functionally equivalent since the `id` column is TEXT and stores UUIDs as string representations.
- **INSERT columns**: The plan's INSERT statements omitted `created_at`, but the actual migration requires it (NOT NULL without default). Added `created_at` with a fixed timestamp `'2026-01-01T00:00:00Z'` to the INSERT statements.

## Blockers

None.
