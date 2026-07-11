# Implementation Report: P18-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-B1                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | GET /v1/system, /v1/system/env handlers |
| Implemented   | 2026-07-12T01:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `GET /v1/system` and `GET /v1/system/env` HTTP handlers that expose the hardware snapshot and Python environment report over the REST API. Both handlers are thin, one-line delegations to read-locked `AppState` fields, per `ANVILML_DESIGN.md §3.3`. Created 4 integration tests verifying response status, sentinel field values, and that updates written through the `RwLock` between requests are reflected in subsequent handler responses. All workspace tests pass (280+ tests), all clippy warnings resolved, all 4 platform cross-checks pass, and both project gates pass.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|------------------|----------------|
| crate  | axum       | 0.8.9            | Cargo.lock     |
| crate  | serde      | 1.0              | Cargo.lock     |
| crate  | serde_json | 1.0              | Cargo.lock     |

No new external dependencies are introduced. The task reuses existing crate dependencies already declared in `anvilml-server/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/system.rs` | New handler module with `get_system()` and `get_system_env()` async functions |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod system;` module declaration |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Registered `/v1/system` and `/v1/system/env` routes in `build_router()` |
| CREATE | `crates/anvilml-server/tests/system_tests.rs` | 4 integration tests for both endpoints |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.17 → 0.1.18 |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new system handler tests |

## Commit Log

```
 .forge/reports/P18-B1_plan.md                | 196 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/mod.rs    |   1 +
 crates/anvilml-server/src/handlers/system.rs |  41 +++++
 crates/anvilml-server/src/lib.rs             |  10 ++
 crates/anvilml-server/tests/system_tests.rs  | 255 +++++++++++++++++++++++++++
 docs/TESTS.md                                |  48 +++++
 10 files changed, 563 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/system_tests.rs (target/debug/deps/system_tests-90208fab43ac9b46)

running 4 tests
test test_get_system_returns_200 ... ok
test test_get_system_env_returns_200 ... ok
test test_get_system_env_reflects_env_report_update ... ok
test test_get_system_reflects_hardware_update ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Full workspace test suite: 280+ tests across all crates, 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.31s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.85s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.76s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.10s
```

All four exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `ToSchema` derives. The response types (`HardwareInfo`, `EnvReport`) are from `anvilml-core` and already have `ToSchema` derives.

## Public API Delta

```
+pub mod system;
```

No new `pub` items introduced. Both handler functions are `pub(crate)` — accessible within the crate but not part of the public library API. This matches the plan's Public API Surface table.

## Deviations from Plan

None. All implementation followed the approved plan exactly:
- Both handler functions are one-line delegations per `ANVILML_DESIGN.md §3.3`.
- Routes registered in order: health → system → system/env → jobs (per plan).
- 4 integration tests matching the plan's test table.
- Version bumped from 0.1.17 to 0.1.18.
- No dual-mode parity markers needed — these handlers are not node `execute()` or arch module `load()` functions per `ANVILML_DESIGN.md §10.6`.

## Blockers

None.
