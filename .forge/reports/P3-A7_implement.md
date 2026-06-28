# Implementation Report: P3-A7

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P3-A7                                       |
| Phase         | 003 — Core Domain Types: Data Model         |
| Description   | anvilml-core: NodeTypeDescriptor, SlotDescriptor, SlotType |
| Implemented   | 2026-06-28T20:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented three pure-data types for node type descriptors in `anvilml-core`: `SlotType` (11-variant closed enum with SCREAMING_SNAKE_CASE serde), `SlotDescriptor` (name, type, optional slot descriptor), and `NodeTypeDescriptor` (full node type shape with typed input/output slots). Added the `node` submodule to `types/mod.rs`, created 4 integration tests in `node_tests.rs`, and updated `docs/TESTS.md`. All 96 workspace tests pass, clippy is clean, and all four platform cross-checks succeed.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|------------------|---------------|
| crate  | utoipa  | 5.5.0            | Cargo.lock    |

No new dependencies added. The `utoipa` crate v5.5.0 (already declared with features `["uuid", "chrono"]`) provides `ToSchema` via its default `macros` feature.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/node.rs` | `SlotType`, `SlotDescriptor`, `NodeTypeDescriptor` types (70 lines) |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod node;` and `pub use node::*;` in alphabetical order |
| CREATE | `crates/anvilml-core/tests/node_tests.rs` | 4 integration tests (161 lines) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped version 0.1.11 → 0.1.12 |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for node_tests |

## Commit Log

```
 .forge/reports/P3-A7_plan.md            | 122 ++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 +--
 Cargo.lock                              |   2 +-
 crates/anvilml-core/Cargo.toml          |   2 +-
 crates/anvilml-core/src/types/mod.rs    |   2 +
 crates/anvilml-core/src/types/node.rs   |  70 ++++++++++++++
 crates/anvilml-core/tests/node_tests.rs | 161 ++++++++++++++++++++++++++++++++
 docs/TESTS.md                           |  48 ++++++++++
 9 files changed, 415 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/node_tests.rs (target/debug/deps/node_tests-b6bd1b34363f4097)

running 4 tests
test test_node_type_descriptor_construction ... ok
test test_slot_descriptor_serde_roundtrip ... ok
test test_node_type_descriptor_empty_slots ... ok
test test_slot_type_screaming_snake_case_serde ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 96 workspace tests passed (including the 4 new node tests). No failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.64s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.26s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.05s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 (Config Surface Sync) and Gate 2 (OpenAPI Drift) are not triggered — this task does not modify `ServerConfig`, handler signatures, `#[utoipa::path]` annotations, or `AppState` fields. Gate 3 (Node Parity) and Gate 4 (Mock/Real Parity Markers) are not triggered — this task adds pure data types, not node implementations.

## Public API Delta

```
+pub mod node;
+pub use node::*;
```

New public items in `crates/anvilml-core/src/types/node.rs`:

| Item | Type | Module Path |
|------|------|-------------|
| `SlotType` | enum | `anvilml_core::types::SlotType` |
| `SlotDescriptor` | struct | `anvilml_core::types::SlotDescriptor` |
| `NodeTypeDescriptor` | struct | `anvilml_core::types::NodeTypeDescriptor` |

All three match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

- **Added `PartialEq, Eq` derives to `SlotDescriptor` and `NodeTypeDescriptor`:** The plan's approved plan listed only `Debug, Clone, Serialize, Deserialize, ToSchema` for both structs. However, the integration tests assert equality via `assert_eq!`, which requires `PartialEq`. The `PartialEq` derive was added to both structs to enable test assertions. This is consistent with the established pattern in the codebase — all other type structs (`ModelMeta`, `ArtifactMeta`, `WorkerInfo`, `Job`, `HardwareInfo`, `GpuDevice`, `InferenceCaps`, `EnvReport`) also derive `PartialEq, Eq`.

## Blockers

None.
