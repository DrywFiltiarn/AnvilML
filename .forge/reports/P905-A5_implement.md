# Implementation Report: P905-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A5                                           |
| Phase       | 905 - FP8 dtype + model metadata patching         |
| Description | anvilml-registry: ModelMetaPatch type and store patch_meta method |
| Implemented | 2026-06-12T15:10:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Added ModelMetaPatch struct in anvilml-core with dtype_hint: Option<DType> and kind: Option<ModelKind> fields, re-exported it from the crate root, and implemented the patch_meta async method on ModelRegistry that applies partial updates, recomputes VRAM via scanner::vram_estimate_mib, and persists the result. Added 4 integration tests covering dtype change with VRAM recomputation, kind-only update, missing-ID None return, and all-none no-op. Bumped anvilml-registry patch version from 0.1.3 to 0.1.4.

## Resolved Dependencies

No new dependencies added. All types (DType, ModelKind, ModelMeta, ModelMetaPatch, ToSchema, Deserialize) were already available in existing workspace dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-core/src/types/model.rs | Add ModelMetaPatch struct with Debug, Clone, Deserialize, ToSchema derives |
| Modify | crates/anvilml-core/src/lib.rs | Re-export ModelMetaPatch |
| Modify | crates/anvilml-registry/src/store.rs | Add patch_meta async method + use anvilml_core::ModelMetaPatch import |
| Modify | crates/anvilml-registry/Cargo.toml | Bump patch version 0.1.3 to 0.1.4 |
| Create | crates/anvilml-registry/tests/patch_meta.rs | 4 integration tests for patch_meta |

## Commit Log

 .forge/reports/P905-A5_plan.md              | 113 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +--
 Cargo.lock                                  |   2 +-
 crates/anvilml-core/src/lib.rs              |   2 +-
 crates/anvilml-core/src/types/model.rs      |  11 ++
 crates/anvilml-registry/Cargo.toml          |   2 +-
 crates/anvilml-registry/src/store.rs        |  36 +++++++
 crates/anvilml-registry/tests/patch_meta.rs | 150 ++++++++++++++++++++++++++++
 9 files changed, 323 insertions(+), 12 deletions(-)

## Test Results

=== anvilml-registry patch_meta tests ===
running 4 tests
test patch_meta_all_none_is_noop ... ok
test patch_meta_missing_returns_none ... ok
test patch_meta_updates_dtype_recomputes_vram ... ok
test patch_meta_updates_kind_keeps_dtype ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

=== Full workspace test suite (summary) ===
anvilml_core:        75 passed; 0 failed
anvilml_hardware:    56 passed; 0 failed
anvilml_ipc:         18 passed; 0 failed
anvilml_registry:    28 passed; 0 failed
anvilml_registry (integration tests): 16 passed; 0 failed
anvilml_scheduler:   43 passed; 0 failed
anvilml_server:      42 passed; 0 failed
anvilml_server (integration tests): 8 passed; 0 failed
anvilml_worker:      19 passed; 0 failed
backend (anvilml binary): 17 passed; 0 failed
backend (integration tests): 12 passed; 0 failed
Doc-tests:           2 passed; 0 failed
TOTAL: 278 passed; 0 failed

## Format Gate

cargo fmt --all -- --check
(No output — exit 0, no formatting drift)

## Platform Cross-Check

# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished dev profile [unoptimized + debuginfo] target(s) in 9.41s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished dev profile [unoptimized + debuginfo] target(s) in 16.54s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished dev profile [unoptimized + debuginfo] target(s) in 19.91s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished dev profile [unoptimized + debuginfo] target(s) in 23.29s

All four checks exited 0.

## Project Gates

Gate 1 - Config Surface Sync:
cargo test -p backend --features mock-hardware --test config_reference
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Gate 2 - OpenAPI Drift:
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
(exit 0 — no diff, ModelMetaPatch is not yet referenced in any handler path)

## Deviations from Plan

- Test VRAM assertion value corrected: plan specified 13_400 for F32 on 6_700_000_000 bytes, but actual vram_estimate_mib calculation yields 12_778 (integer MiB division: 6_700_000_000 / 1_048_576 = 6389, * 2.0 = 12778). Both test assertions updated to 12_778.
- Added 3 additional tests beyond the 1 required by the plan: patch_meta_updates_kind_keeps_dtype, patch_meta_missing_returns_none, patch_meta_all_none_is_noop. These cover all code branches in patch_meta and follow the project test conventions.

## Blockers

None.
