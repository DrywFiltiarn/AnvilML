# Implementation Report: P3-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P3-A1                           |
| Phase         | 3 — Core Domain Types: Data Model |
| Description   | anvilml-core: Job, JobStatus, JobSettings types |
| Implemented   | 2026-06-28T15:10:00Z            |
| Status        | COMPLETE                        |

## Summary

Created the `types/` submodule in `anvilml-core` with three domain types — `Job`, `JobStatus`, and `JobSettings` — as specified in ANVILML_DESIGN.md §5.3. Added the `chrono` dependency with the `serde` feature for `DateTime<Utc>` serialization. Created an integration test crate (`job_tests.rs`) with four serde roundtrip tests. All compile, lint, cross-check, and gate checks pass.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | chrono   | 0.4.45           | rust-docs MCP  |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/mod.rs` | Module declaration file for `types` submodule tree; re-exports `job` module items. |
| CREATE | `crates/anvilml-core/src/types/job.rs` | `Job`, `JobStatus`, `JobSettings` type definitions with doc comments and serde derives. |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `chrono = { version = "0.4", features = ["serde"] }` dependency; bumped version from 0.1.5 to 0.1.6. |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod types;` and `pub use types::*;` after existing `mod error;`. |
| CREATE | `crates/anvilml-core/tests/job_tests.rs` | Integration tests for serde roundtrips (4 tests). |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for new job tests. |

## Commit Log

```
 .forge/reports/P3-A1_plan.md           | 189 +++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md           |   6 +-
 .forge/state/state.json                |  13 +--
 Cargo.lock                             | 109 ++++++++++++++++++-
 crates/anvilml-core/Cargo.toml         |   4 +-
 crates/anvilml-core/src/lib.rs         |   3 +
 crates/anvilml-core/src/types/job.rs   |  60 +++++++++++
 crates/anvilml-core/src/types/mod.rs   |   3 +
 crates/anvilml-core/tests/job_tests.rs | 131 +++++++++++++++++++++++
 docs/TESTS.md                          |  48 +++++++++
 10 files changed, 555 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/job_tests.rs (target/debug/deps/job_tests-320b3a8fa757ca2a)

running 4 tests
test test_job_status_all_variants_roundtrip ... ok
test test_job_settings_default ... ok
test test_job_with_nulls_roundtrip ... ok
test test_job_serde_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: all 62 tests across all crates passed with 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.17s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.44s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not triggered — task does not add, remove, or rename node types in `worker/nodes/`, nor modify `crates/anvilml-core/src/node_registry.rs`.

### Gate 4 — Mock/Real Parity Markers
Not triggered — `Job`, `JobStatus`, and `JobSettings` are pure data types with no `execute()`, `load()`, `sample()`, `decode()`, or `compute_latent_shape()` methods.

## Public API Delta

```
+pub mod types;
+pub use types::*;
```

New `pub` items introduced by this task:

| Item | Type | Module Path |
|------|------|-------------|
| `types` | `pub mod` | `anvilml_core::types` |
| `Job` | `pub struct` | `anvilml_core::types::Job` |
| `JobStatus` | `pub enum` | `anvilml_core::types::JobStatus` |
| `JobSettings` | `pub struct` | `anvilml_core::types::JobSettings` |

All three types are `pub` and re-exported via `pub use types::*;` in `lib.rs`, matching the plan's Public API Surface table.

## Deviations from Plan

- Added `pub use job::*;` to `types/mod.rs` (not in the original plan) — required so that `pub use types::*;` in `lib.rs` actually re-exports `Job`, `JobStatus`, and `JobSettings` at the crate root. Without this, the test file's `use anvilml_core::types::*;` import would resolve only to the `job` module, not the types themselves.
- Added `PartialEq` and `Eq` derives to `Job` and `JobSettings` (not in the original plan) — required so that `assert_eq!` works in integration tests. The plan's tests use `assert_eq!(job, roundtripped)` which requires `PartialEq`.
- Added `use chrono::DateTime;` to the imports in `job.rs` — the plan only listed `use chrono::Utc;` but `DateTime<Utc>` requires the `DateTime` type to be in scope.

## Blockers

None.
