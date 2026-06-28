# Implementation Report: P3-A4

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A4                              |
| Phase         | 003 — Core Domain Types: Data Model |
| Description   | anvilml-core: HardwareInfo, GpuDevice, DeviceType types |
| Implemented   | 2026-06-28T17:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the hardware snapshot types in `anvilml-core/src/types/hardware.rs`: `DeviceType` (enum), `HostInfo` (struct), `InferenceCaps` (struct), `GpuDevice` (struct), `EnumerationSource` (enum), `CapabilitySource` (enum), and `HardwareInfo` (struct). Updated `types/mod.rs` to export the new module. Created 7 integration tests in `tests/hardware_tests.rs` covering serde roundtrips and snake_case JSON serialisation for all types. Bumped `anvilml-core` version from 0.1.8 to 0.1.9.

## Resolved Dependencies

None. No new external dependencies were added. All types use existing dependencies (`serde`, `utoipa`, `chrono`, `uuid`) already declared in `crates/anvilml-core/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/hardware.rs` | 7 hardware types: DeviceType, HostInfo, InferenceCaps, GpuDevice, EnumerationSource, CapabilitySource, HardwareInfo |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod hardware;` and `pub use hardware::*;` |
| CREATE | `crates/anvilml-core/tests/hardware_tests.rs` | 7 integration tests for hardware type serde roundtrips |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped version 0.1.8 → 0.1.9 |
| MODIFY | `docs/TESTS.md` | Added 7 test entries for hardware_tests |

## Commit Log

```
 .forge/reports/P3-A4_plan.md                | 276 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   2 +-
 crates/anvilml-core/Cargo.toml              |   2 +-
 crates/anvilml-core/src/types/hardware.rs   | 136 ++++++++++++++
 crates/anvilml-core/src/types/mod.rs        |   2 +
 crates/anvilml-core/tests/hardware_tests.rs | 273 +++++++++++++++++++++++++++
 docs/TESTS.md                               |  84 +++++++++
 9 files changed, 783 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-d254f530b621dccc)

running 7 tests
test test_capability_source_serde_snake_case ... ok
test test_device_type_serde_snake_case ... ok
test test_enumeration_source_serde_snake_case ... ok
test test_gpu_device_construction_and_serde ... ok
test test_host_info_serde_roundtrip ... ok
test test_hardware_info_serde_roundtrip ... ok
test test_inference_caps_default_roundtrip ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 113 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.92s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.84s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.18s
```

All four platform cross-checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not triggered — this task does not add, remove, or rename a node type in `worker/nodes/` or modify `crates/anvilml-core/src/node_registry.rs`.

### Gate 4 — Mock/Real Parity Markers
Not triggered — this task adds data types, not node `execute()` or arch module `load()`/`sample()`/`decode()` functions. The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention does not apply to pure data types.

## Public API Delta

```
+pub mod hardware;
+pub use hardware::*;
```

The `pub mod hardware;` and `pub use hardware::*;` in `types/mod.rs` expose all 7 types at the `anvilml_core::types` module level, which are already re-exported at the crate root via `pub use types::*;` in `lib.rs`.

New public items re-exported from `hardware`:
- `DeviceType` — enum (Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `HostInfo` — struct (Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `InferenceCaps` — struct (Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `GpuDevice` — struct (Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `EnumerationSource` — enum (Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `CapabilitySource` — enum (Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)
- `HardwareInfo` — struct (Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)

## Deviations from Plan

1. **Added `PartialEq, Eq` derives to all structs.** The plan specified `GpuDevice` without `PartialEq` because `InferenceCaps` would not derive it until P3-A5. Since both tasks define types in the same file and both land before any build, adding `PartialEq, Eq` to `InferenceCaps`, `HostInfo`, `GpuDevice`, and `HardwareInfo` enables the `assert_eq!` assertions in the tests. This is a minimal, correct fix — the tests require equality comparison and all types have natural field-level equality.

2. **Added `#[serde(rename = "pytorch")]` on `CapabilitySource::PyTorch`.** The `#[serde(rename_all = "snake_case")]` convention converts `PyTorch` to `"py_torch"` (splitting on the capital `T`), but the design doc and downstream consumers expect `"pytorch"`. This is a correctness fix to ensure the JSON key matches the expected value.

3. **Added 3 additional tests beyond the plan's minimum of 4.** The plan specified 4 tests. The implementation provides 7: `test_device_type_serde_snake_case`, `test_host_info_serde_roundtrip`, `test_gpu_device_construction_and_serde`, `test_hardware_info_serde_roundtrip`, `test_inference_caps_default_roundtrip`, `test_enumeration_source_serde_snake_case`, and `test_capability_source_serde_snake_case`. Each enum variant group gets its own dedicated test, and `InferenceCaps` gets a default-value test.

4. **Dual-mode parity markers not applicable.** The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention (ANVILML_DESIGN.md §10.6) applies only to node `execute()` and arch module `load()`/`sample()`/`decode()` functions in `worker/nodes/`. This task creates pure data types in `anvilml-core`, which are not in scope for the marker convention.

## Blockers

None.
