# Implementation Report: P7-G4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-G4                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | Fix name resolution priority and --print-hardware display |
| Implemented | 2026-06-05T19:45:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Implemented GPU name resolution priority fix: generic/empty driver names are replaced by the database model label, while specific Vulkan SKU names are preserved with the DB group name shown in parentheses. Updated `print_hardware_table` to display the composite name. Added four unit tests covering all hit/miss and generic/specific combinations across the affected crates.

## Resolved Dependencies

No new dependencies introduced. All changes use existing crate types (`anvilml_core::GpuDevice`, `anvilml_registry::DeviceCapabilityRow`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add `db_group_name: Option<String>` field to `GpuDevice` struct with `#[serde(default)]`; update test struct initializations and assertions |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Add `is_generic_driver_name()` helper; rewrite `resolve_caps_from_row` hit/miss paths; add 4 unit tests |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Add `db_group_name: None` to override GpuDevice construction |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Add `db_group_name: None` to CPU device construction |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Add `db_group_name: None` to Vulkan device construction |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Add `db_group_name: None` to both sysfs device constructions |
| Modify | `crates/anvilml-hardware/src/nvml.rs` | Add `db_group_name: None` to NVML device construction |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Add `db_group_name: None` to mock device construction |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Add `db_group_name: None` to DXGI device construction (Windows) |
| Modify | `crates/anvilml-server/src/ws/stats_tick.rs` | Add `db_group_name: None` to both test GpuDevice constructions |
| Modify | `backend/src/main.rs` | Update `print_hardware_table` to compute display name with `(db_group_name)` suffix |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 backend/src/main.rs                        |   6 +-
 crates/anvilml-core/src/types/hardware.rs  |  11 ++
 crates/anvilml-hardware/src/cpu.rs         |   1 +
 crates/anvilml-hardware/src/device_db.rs   | 208 +++++++++++++++++++++++++----
 crates/anvilml-hardware/src/dxgi.rs        |   1 +
 crates/anvilml-hardware/src/lib.rs         |   1 +
 crates/anvilml-hardware/src/mock.rs        |   1 +
 crates/anvilml-hardware/src/nvml.rs        |   1 +
 crates/anvilml-hardware/src/sysfs.rs       |   2 +
 crates/anvilml-hardware/src/vulkan.rs      |   1 +
 crates/anvilml-server/src/ws/stats_tick.rs |   2 +
 13 files changed, 216 insertions(+), 38 deletions(-)
```

## Test Results

```
running 74 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_default_server_config ... ok
... (all 74 anvilml-core tests passed)
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 56 tests
test device_db::tests::generic_name_replaced_by_group_label ... ok
test device_db::tests::miss_with_empty_name_shows_unknown ... ok
test device_db::tests::miss_with_specific_name_preserved ... ok
test device_db::tests::specific_vulkan_name_preserved ... ok
... (all 56 anvilml-hardware tests passed)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 19 tests (anvilml-registry) ... ok
running 8 tests (anvilml-server) ... ok
running 8 tests (backend binary) ... ok
running 1 test (config_reference) ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests: 2 passed, 0 failed.

Total: 183 tests, 0 failures.
```

## Platform Cross-Check

```
# 1. cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.53s

# 2. cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.57s

# 3. cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s
```

All three checks exited 0.

## Project Gates

```
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Config drift gate passed.

## Deviations from Plan

None. All six plan steps were implemented exactly as specified. The following mechanical changes were required to keep the codebase compiling after adding `db_group_name` to `GpuDevice`:
- Added `db_group_name: None` to 9 additional GpuDevice struct initializations across the hardware crate (lib.rs, cpu.rs, vulkan.rs, sysfs.rs ×2, nvml.rs, mock.rs, dxgi.rs) and server test fixture (stats_tick.rs).
- Added `db_group_name` assertions to existing roundtrip tests in `hardware.rs`.

## Blockers

None.
