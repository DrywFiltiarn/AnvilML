# Implementation Report: P3-A4

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P3-A4                                         |
| Phase       | 003 — Core Domain Types                       |
| Description | anvilml-core: node and worker types            |
| Implemented | 2026-06-14T21:25:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Implemented the node and worker type definitions for the anvilml-core crate per the approved plan. Created two new source files (`types/node.rs` and `types/worker.rs`) containing 7 public types: `SlotType`, `SlotDescriptor`, `NodeTypeDescriptor`, `WorkerStatus`, `ProvisioningState`, `WorkerInfo`, and `EnvReport`. Updated `types/mod.rs` and `lib.rs` with module declarations and re-exports. Added 6 integration tests across two test files. Bumped `anvilml-core` version from 0.1.6 to 0.1.7. Also fixed a pre-existing test isolation defect in `config_load_tests.rs` where `test_missing_file_uses_defaults` was not marked `#[serial]`, causing a race condition with env-var-mutating tests.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | serde     | 1.0.228          | Cargo.toml     |
| crate  | utoipa    | 5.5.0            | Cargo.toml     |
| crate  | uuid      | 1.23.3           | Cargo.toml     |
| crate  | chrono    | 0.4.45           | Cargo.toml     |

No new external dependencies introduced. All types use only existing workspace dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/node.rs` | NodeTypeDescriptor, SlotDescriptor, SlotType types (84 lines) |
| CREATE | `crates/anvilml-core/src/types/worker.rs` | WorkerInfo, WorkerStatus, EnvReport, ProvisioningState types (106 lines) |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod node`, `pub mod worker`, and re-exports |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added 7 new types to `pub use types::{...}` |
| CREATE | `crates/anvilml-core/tests/node_tests.rs` | 3 integration tests for node types (148 lines) |
| CREATE | `crates/anvilml-core/tests/worker_tests.rs` | 3 integration tests for worker types (115 lines) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Version bump 0.1.6 → 0.1.7 |
| MODIFY | `crates/anvilml-core/tests/config_load_tests.rs` | Added `#[serial]` to `test_missing_file_uses_defaults` to fix pre-existing test isolation defect |
| MODIFY | `docs/TESTS.md` | Added 6 new test entries for node and worker tests |

## Commit Log

```
.forge/reports/P3-A4_plan.md                   | 198 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   4 +-
 crates/anvilml-core/Cargo.toml                 |   2 +-
 crates/anvilml-core/src/lib.rs                 |   7 +-
 crates/anvilml-core/src/types/mod.rs           |   4 +
 crates/anvilml-core/src/types/node.rs          |  84 +++++++++++
 crates/anvilml-core/src/types/worker.rs        | 106 +++++++++++++
 crates/anvilml-core/tests/config_load_tests.rs |   5 +
 crates/anvilml-core/tests/node_tests.rs        | 148 ++++++++++++++++++
 crates/anvilml-core/tests/worker_tests.rs      | 115 ++++++++++++++
 docs/TESTS.md                                  |  48 ++++++
 13 files changed, 725 insertions(+), 15 deletions(-)
```

## Test Results

```
     Running tests/node_tests.rs (target/debug/deps/node_tests-08b017158566f94f)

running 3 tests
test test_node_type_descriptor_json_roundtrip ... ok
test test_slot_descriptor_optional_field ... ok
test test_slot_type_variants ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/worker_tests.rs (target/debug/deps/worker_tests-9151709388b075a4)

running 3 tests
test test_env_report_default_preflight ... ok
test test_worker_info_json_roundtrip ... ok
test test_worker_status_variants ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Full workspace test suite: 37 tests, 0 failures
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.65s

# 3. Real-hardware Linux
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s

All four checks passed.
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
Gate not applicable — this task does not modify `ServerConfig`. No `config_reference` test exists in the current workspace (the `backend` package has only `cli_tests.rs`).

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
→ Exit 0 — no diff (openapi.json is up to date)
```
Gate passed — no OpenAPI drift.

### Gate 3 — Node Parity
```
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_parity.py -v
→ ERROR: file or directory not found: worker/tests/test_parity.py
```
Gate not applicable — `test_parity.py` does not exist yet in the repository.

## Public API Delta

```
+pub mod node;
+pub mod worker;
+pub use node::{NodeTypeDescriptor, SlotDescriptor, SlotType};
+pub use worker::{EnvReport, ProvisioningState, WorkerInfo, WorkerStatus};
```

New pub items introduced by this task:

| Name | Type | Module Path |
|------|------|-------------|
| `SlotType` | enum | `types::node` |
| `SlotDescriptor` | struct | `types::node` |
| `NodeTypeDescriptor` | struct | `types::node` |
| `WorkerStatus` | enum | `types::worker` |
| `ProvisioningState` | enum | `types::worker` |
| `WorkerInfo` | struct | `types::worker` |
| `EnvReport` | struct | `types::worker` |

All 7 items match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Pre-existing test isolation fix**: Added `#[serial]` to `test_missing_file_uses_defaults` in `crates/anvilml-core/tests/config_load_tests.rs`. This test mutates process-global `std::env` state but was not annotated with `#[serial]`, causing it to race with parallel env-var-mutating tests (`test_env_var_beats_toml`, `test_nested_env_var`). The fix is required to prevent intermittent test failures and follows the project's test isolation rules (ENVIRONMENT.md §11.3).
- **Removed unused imports** from `crates/anvilml-core/tests/worker_tests.rs`: initially imported `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType` from `anvilml_core` but they were not used in the test file. Removed after clippy warning.

## Blockers

None.
