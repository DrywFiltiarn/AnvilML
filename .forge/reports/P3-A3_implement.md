# Implementation Report: P3-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A3                              |
| Phase         | 003 — Core Domain Types            |
| Description   | anvilml-core: hardware types (HardwareInfo, GpuDevice, InferenceCaps) |
| Implemented   | 2026-06-14T18:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Created seven hardware domain types in `crates/anvilml-core/src/types/hardware.rs`: three enums (`DeviceType`, `EnumerationSource`, `CapabilitySource`) and four structs (`InferenceCaps`, `HostInfo`, `GpuDevice`, `HardwareInfo`). Updated `types/mod.rs` to declare the module and re-export all seven types. Updated `lib.rs` to add crate-level re-exports. Created `crates/anvilml-core/tests/hardware_tests.rs` with four integration tests. Updated `docs/TESTS.md` with test catalogue entries. Bumped `anvilml-core` patch version from 0.1.4 to 0.1.5. All workspace tests pass (33 total, 4 new), all format and lint gates pass, all four platform cross-checks pass.

## Resolved Dependencies

None. All types use existing workspace dependencies already declared in `crates/anvilml-core/Cargo.toml`: `serde`, `serde_json`, `utoipa`, `chrono`, `uuid`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/hardware.rs` | Seven hardware domain types with full doc comments |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod hardware;` and re-exports for 7 types |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added re-exports for 7 new types to crate root |
| CREATE | `crates/anvilml-core/tests/hardware_tests.rs` | 4 integration tests for JSON roundtrip, enum variants, default |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |
| MODIFY | `docs/TESTS.md` | Added 5 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P3-A3_plan.md                | 254 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   2 +-
 crates/anvilml-core/Cargo.toml              |   2 +-
 crates/anvilml-core/src/lib.rs              |   5 +-
 crates/anvilml-core/src/types/hardware.rs   | 161 ++++++++++++++++++
 crates/anvilml-core/src/types/mod.rs        |   9 +-
 crates/anvilml-core/tests/hardware_tests.rs | 238 ++++++++++++++++++++++++++
 docs/TESTS.md                               |  32 ++++
 10 files changed, 708 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-34fa657c6ff58aec)

running 4 tests
test test_device_type_variants ... ok
test test_enum_variants_roundtrip ... ok
test test_inference_caps_default ... ok
test test_hardware_info_json_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 33 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
Checking anvilml-core v0.1.5 (/home/dryw/AnvilML/crates/anvilml-core)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.27s
---CHECK1_OK---

# Check 2: Mock-hardware Windows
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.33s
---CHECK2_OK---

# Check 3: Real-hardware Linux
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.66s
---CHECK3_OK---

# Check 4: Real-hardware Windows
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.70s
---CHECK4_OK---
```

All four cross-checks passed.

## Project Gates

Gate 1 (Config Surface Sync): Not applicable — task does not modify `ServerConfig` fields or nested config structs.

Gate 2 (OpenAPI Drift): Not applicable — `backend/openapi.json` does not yet exist in the repository (prior to the phase that introduces the `anvilml-openapi` binary).

Gate 3 (Node Parity): Not applicable — task does not add, remove, or rename node types.

## Public API Delta

New `pub` items in `crates/anvilml-core/src/types/hardware.rs`:
- `pub enum DeviceType` — `types::hardware`
- `pub enum EnumerationSource` — `types::hardware`
- `pub enum CapabilitySource` — `types::hardware`
- `pub struct InferenceCaps` — `types::hardware`
- `pub struct HostInfo` — `types::hardware`
- `pub struct GpuDevice` — `types::hardware`
- `pub struct HardwareInfo` — `types::hardware`

Re-exported from crate root (`anvilml_core::...`): all seven types above.

## Deviations from Plan

- Added `PartialEq` derive to `InferenceCaps` (not in plan). The plan listed derives as `Debug, Clone, Default, Serialize, Deserialize, ToSchema` but the tests use `assert_eq!` on `InferenceCaps` fields, which requires `PartialEq`. This is a minimal addition consistent with how other struct types in the crate (e.g., `JobSettings`) derive `PartialEq` for testability.

## Blockers

None.
