# Implementation Report: P3-A6

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A6                              |
| Phase         | 003 — Core Domain Types: Data Model |
| Description   | anvilml-core: WorkerInfo, WorkerStatus, EnvReport, ProvisioningState |
| Implemented   | 2026-06-28T19:05:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `crates/anvilml-core/src/types/worker.rs` defining four types — `WorkerStatus` (5 variants), `WorkerInfo` (6-field struct), `EnvReport` (3-field struct), and `ProvisioningState` (4 variants) — that the scheduler's dispatch logic and the `/v1/workers` and `/v1/system` HTTP handlers will consume. Registered the module in `types/mod.rs` with `pub mod worker;` and `pub use worker::*;`. Ship four integration tests in `crates/anvilml-core/tests/worker_tests.rs` covering construction, serde roundtrips, and JSON field name verification. Bumped `anvilml-core` patch version from `0.1.10` to `0.1.11`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|------------------|---------------|
| crate  | uuid    | 1.23.4           | rust-docs MCP |
| crate  | utoipa  | 5.5.0            | rust-docs MCP |
| crate  | serde   | 1.0              | (existing)    |

All dependencies were already present in `crates/anvilml-core/Cargo.toml`. No new dependency was introduced. The `uuid` crate has the `serde` feature enabled for `Serialize`/`Deserialize` on `Uuid`. The `utoipa` crate has the `uuid` feature enabled for `ToSchema` derivation on types containing `Uuid`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/worker.rs` | New module with `WorkerStatus`, `WorkerInfo`, `EnvReport`, `ProvisioningState` |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod worker;` and `pub use worker::*;` |
| CREATE | `crates/anvilml-core/tests/worker_tests.rs` | Integration tests (4 tests) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version `0.1.10` → `0.1.11` |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for new worker tests |

## Commit Log

```
 .forge/reports/P3-A6_plan.md              | 122 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +--
 Cargo.lock                                |   2 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/types/mod.rs      |   2 +
 crates/anvilml-core/src/types/worker.rs   |  89 ++++++++++++++++++++
 crates/anvilml-core/tests/worker_tests.rs | 133 ++++++++++++++++++++++++++++++
 docs/TESTS.md                             |  48 +++++++++++
 9 files changed, 406 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/worker_tests.rs (target/debug/deps/worker_tests-c2f9afe1b9b0a784)

running 4 tests
test test_env_report_serde_roundtrip ... ok
test test_provisioning_state_serde_snake_case ... ok
test test_worker_status_serde_snake_case ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 65 tests passed, 0 failed, 0 ignored across all crates.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0, no drift detected)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.29s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.68s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.67s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test tests::config_reference_matches_defaults ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not triggered — this task does not add, remove, or rename node types in `worker/nodes/`.

### Gate 4 — Mock/Real Parity Markers
Not triggered — this task does not add or modify a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`. The new types are pure data structs and enums, not node functions.

## Public API Delta

```
pub enum WorkerStatus {
pub struct WorkerInfo {
pub struct EnvReport {
pub enum ProvisioningState {
```

Four new public types matching the plan's `## Public API Surface` table exactly:
- `WorkerStatus` — enum with 5 variants (`Spawning`, `Idle`, `Busy`, `Dying`, `Dead`)
- `WorkerInfo` — struct with 6 fields (`worker_id`, `status`, `device_index`, `device_type`, `pid`, `current_job_id`)
- `EnvReport` — struct with 3 fields (`python_version`, `torch_version`, `torch_importable`)
- `ProvisioningState` — enum with 4 variants (`NotStarted`, `InProgress`, `Complete`, `Failed`)

## Deviations from Plan

The plan specified `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` for `WorkerInfo` and `EnvReport` without `PartialEq`/`Eq`. The integration tests use `assert_eq!` which requires `PartialEq`. Added `PartialEq, Eq` to both struct derives to match the pattern used by equivalent structs in the codebase (`Job`, `JobSettings`, `ArtifactMeta`, etc.). This is a necessary deviation to make the tests compile and pass — not a change in public API shape.

## Blockers

None.
