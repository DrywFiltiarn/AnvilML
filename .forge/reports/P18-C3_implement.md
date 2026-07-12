# Implementation Report: P18-C3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-C3                          |
| Phase         | 18 — Startup Model Scan Wiring  |
| Description   | Wire ModelScanner::scan_dir() to run as a non-blocking background task during server startup so the model registry is populated before the server starts accepting requests. |
| Implemented   | 2026-07-12T10:22:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the shared `trigger_model_scan()` function in `anvilml-registry` and wired it
into both the server startup path (`backend/src/main.rs`) and the `/v1/models/rescan` HTTP
handler (`anvilml-server/src/handlers/models.rs`). Created two integration tests that spawn
the actual `anvilml` binary and verify the planted model appears via `GET /v1/models` and
that an empty directory returns `[]`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | reqwest   | 0.12             | rust-docs MCP  |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/scan_trigger.rs` | Shared `trigger_model_scan()` function |
| Modify | `crates/anvilml-registry/src/lib.rs` | Export `scan_trigger` module and `trigger_model_scan` |
| Modify | `crates/anvilml-registry/Cargo.toml` | Version 0.1.7 → 0.1.8 |
| Modify | `backend/src/main.rs` | Import and call `trigger_model_scan()` in startup path |
| Modify | `backend/Cargo.toml` | Version 0.1.15 → 0.1.16, added reqwest dev-dependency |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Replace inline scan logic with `trigger_model_scan()` call |
| Modify | `crates/anvilml-server/Cargo.toml` | Version 0.1.21 → 0.1.22 |
| Create | `backend/tests/startup_scan_tests.rs` | 2 integration tests for startup scan behavior |

## Commit Log

```
 .forge/reports/P18-C3_plan.md                | 222 +++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   | 478 ++++++++++++++++++++++++++-
 backend/Cargo.toml                           |   3 +-
 backend/src/main.rs                          |  11 +
 backend/tests/startup_scan_tests.rs          | 295 +++++++++++++++++
 crates/anvilml-registry/Cargo.toml           |   2 +-
 crates/anvilml-registry/src/lib.rs           |   2 +
 crates/anvilml-registry/src/scan_trigger.rs  |  82 +++++
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/models.rs |  59 +---
 12 files changed, 1095 insertions(+), 80 deletions(-)
```

## Test Results

```
running 27 tests
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

  startup_scan_tests:
    test_startup_scan_displays_planted_model ... ok
    test_startup_scan_empty_dir_lists_no_models ... ok

  All other tests (25 existing): all passed.
  Doc-tests: 3 passed (0 failed).
```

## Format Gate

```
cargo fmt --all --check
# Output: (empty — all files formatted)
FORMAT PASS 2: CLEAN
```

## Platform Cross-Check

```
cargo check --target x86_64-unknown-linux-gnu --features mock-hardware ... ok
cargo check --target x86_64-pc-windows-msvc --features mock-hardware ... ok
cargo check --workspace --features mock-hardware ... ok
cargo check --workspace ... ok
```

## Project Gates

```
cargo clippy --workspace --features mock-hardware -- -D warnings
# Output: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
```

## Public API Delta

```
+pub mod scan_trigger;
+pub use scan_trigger::trigger_model_scan;
```

Two new public items:
- `pub mod scan_trigger` — module in `anvilml_registry`
- `pub fn trigger_model_scan(pool, model_dirs, model_scan_depth)` — shared scan trigger

## Deviations from Plan

1. **No `backend/src/scan.rs` created** — The approved plan listed `backend/src/scan.rs` as a
   new file. During codebase inspection, it was determined that placing `trigger_model_scan()`
   in `anvilml-registry` avoids a circular dependency: `backend` depends on `anvilml-server`,
   so `anvilml-server` cannot import from `backend`. Since `anvilml-registry` is depended on
   by both, it is the correct location for shared scan logic. The plan's `## Files Affected`
   table was updated to reflect `crates/anvilml-registry/src/scan_trigger.rs` instead.

2. **`trigger_model_scan()` is sync, not async** — The function signature is `pub fn
   trigger_model_scan(...) { tokio::spawn(async move { ... }) }` rather than `pub async fn`.
   This avoids unused Future warnings at call sites and matches the fire-and-forget contract.

3. **Integration tests use `std::thread::spawn` for HTTP polling** — `tokio::task::block_in_place`
   requires a multi-threaded runtime which isn't available in the single-threaded test context.
   `std::thread::spawn` with `reqwest::blocking` is the correct approach.

## Blockers

None.
