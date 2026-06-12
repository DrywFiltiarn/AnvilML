# Implementation Report: P20-A3

| Field | Value |
|-------|-------|
| Task ID | P20-A3 |
| Phase | 020 — OpenAPI & Launcher Polish |
| Description | anvilml: browser auto-open at startup (unless --no-browser/Headless) |
| Implemented | 2026-06-12T11:15:00Z |
| Status | COMPLETE |

## Summary

Added the `open` crate dependency and implemented browser auto-open in the AnvilML launcher binary (`backend/src/main.rs`). After the HTTP server TCP listener binds successfully, the code checks `args.no_browser` and `cfg.frontend.mode` before attempting to open the default browser to the server URL. Both skip conditions log at DEBUG level; browser-open failures log at WARN level without aborting startup.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|------------------|--------|
| crate | open | 5.3 (resolved to 5.3.5 by cargo) | N/A — no rust-docs MCP available; using approved plan version |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `open = "5.3"` to `[workspace.dependencies]` |
| Modify | `backend/Cargo.toml` | Add `open = { workspace = true }` to `[dependencies]`; bump version `0.1.12` → `0.1.13` |
| Modify | `backend/src/main.rs` | Add `FrontendMode` to import; add browser-open conditional logic after TCP bind |

## Commit Log

```
 .forge/reports/P20-A3_plan.md | 88 +++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |  6 +--
 .forge/state/state.json       | 13 ++++---
 Cargo.lock                    | 39 ++++++++++++++++++-
 Cargo.toml                    |  1 +
 backend/Cargo.toml            |  3 +-
 backend/src/main.rs           | 15 +++++++-
 7 files changed, 153 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-f3df55d7386c8396)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-395d68b7d76bba7d)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-5ce179a5e12f9aa5)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-07dc3a94706f3425)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-93fac82b827ddd80)
running 43 tests
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-c93827e5ac489a4a)
running 42 tests
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_save.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_serve.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-d4772059b303a4ca)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-764023c17f46904c)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_cancel.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_delete.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_lifecycle.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/preflight_check.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Total: 261 passed; 0 failed; 0 ignored
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.19s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.46s

# 3. Real-hardware Linux check
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.20s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.39s

All four checks exited 0.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.15s
     Running tests/config_reference.rs
running 0 tests (filtered to config_reference)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Deviations from Plan

None.

## Blockers

None.
