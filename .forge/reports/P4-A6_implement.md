# Implementation Report: P4-A6

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P4-A6                           |
| Phase         | 004 — Hardware Detection: Detectors |
| Description   | anvilml-hardware: SysfsPciDetector Linux fallback (cfg-gated) |
| Implemented   | 2026-06-29T10:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented `SysfsPciDetector` as the Linux fallback GPU detector. The struct reads PCI config space files from `/sys/bus/pci/devices/`, filters for display controllers (class prefix `0x03`), maps vendor IDs to `DeviceType` via the shared `vendor_id_to_device_type()` function, and constructs `GpuDevice` structs with `enumeration_source: EnumerationSource::Sysfs`. The detector never panics — missing paths or permission errors return `Ok(vec![])`. Seven integration tests verify: missing path handling, synthetic AMD device parsing, class filtering, NVIDIA vendor mapping, multi-device filtering, and `refresh_vram()` returning `(0, 0)`.

## Resolved Dependencies

None. This task introduces no new external crates. It uses only `std::fs`, `std::path`, and existing `anvilml-core` types.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/sysfs.rs` | `SysfsPciDetector` struct and `DeviceDetector` impl with `detect_from_path` helper |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add cfg-gated `mod sysfs` and `pub use sysfs::{SysfsPciDetector, detect_from_path}` |
| CREATE | `crates/anvilml-hardware/tests/sysfs_tests.rs` | 7 integration tests for `SysfsPciDetector` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |
| MODIFY | `docs/TESTS.md` | Add 7 catalogue entries for new sysfs tests |

## Commit Log

```
 .forge/reports/P4-A6_plan.md                 | 213 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-hardware/Cargo.toml           |   2 +-
 crates/anvilml-hardware/src/lib.rs           |   5 +
 crates/anvilml-hardware/src/sysfs.rs         | 258 +++++++++++++++++++++++++++
 crates/anvilml-hardware/tests/sysfs_tests.rs | 255 ++++++++++++++++++++++++++
 docs/TESTS.md                                |  84 +++++++++
 9 files changed, 827 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/sysfs_tests.rs (target/debug/deps/sysfs_tests-16af88c9e41fe5b2)

running 7 tests
test test_sysfs_detect_missing_path_returns_empty ... ok
test test_sysfs_refresh_vram_returns_zero ... ok
test test_sysfs_detect_nvidia_vendor ... ok
test test_sysfs_detect_synthetic_display_device ... ok
test test_sysfs_filter_non_display_class ... ok
test test_sysfs_multi_device_filter ... ok
test test_sysfs_detect_never_errors ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all 113 tests passed (0 failed) across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
cargo check --workspace --features mock-hardware → Finished `dev` profile

# Check 2: Mock-hardware Windows (x86_64-pc-windows-gnu)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished `dev` profile

# Check 3: Real-hardware Linux
cargo check --bin anvilml → Finished `dev` profile

# Check 4: Real-hardware Windows (x86_64-pc-windows-gnu)
cargo check --bin anvilml --target x86_64-pc-windows-gnu → Finished `dev` profile

All 4 checks passed.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
→ running 1 test
→ tests::config_reference_matches_defaults ... ok
→ test result: ok. 1 passed; 0 failed
```

Gate 2 (OpenAPI Drift) is not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

## Public API Delta

```
+pub mod sysfs;
+pub use sysfs::{SysfsPciDetector, detect_from_path};
```

New public items:
- `pub mod sysfs` — Linux-only module declaration
- `pub use sysfs::{SysfsPciDetector, detect_from_path}` — re-exports
- `pub struct SysfsPciDetector` — zero-sized struct, implements `DeviceDetector`
- `pub fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` — enumerate Linux PCI display controllers
- `pub fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` — returns `(0, 0)`
- `pub fn detect_from_path(base_path: &Path) -> Result<Vec<GpuDevice>, AnvilError>` — test helper

## Deviations from Plan

1. **`detect_from_path` is `pub` instead of private**: The plan stated "No new `pub` functions or types beyond the struct itself." However, integration tests (compiled as separate crates) cannot access `pub(crate)` functions. Making `detect_from_path` `pub` and re-exporting it from `lib.rs` is necessary for the synthetic sysfs tree tests to work. This matches the existing pattern where `vendor_id_to_device_type` is `pub` and re-exported from `lib.rs` for test access.

2. **6 tests instead of 4**: The plan specified ≥ 3 tests and listed 4 in the Tests table. The implementation includes 6 tests: `test_sysfs_detect_missing_path_returns_empty`, `test_sysfs_detect_synthetic_display_device`, `test_sysfs_filter_non_display_class`, `test_sysfs_detect_nvidia_vendor`, `test_sysfs_detect_never_errors`, `test_sysfs_refresh_vram_returns_zero`, and `test_sysfs_multi_device_filter`. The extra tests (`detect_never_errors`, `refresh_vram_returns_zero`, `multi_device_filter`) follow the established pattern from other detector test files (cpu_tests.rs, dxgi_tests.rs, vulkan_tests.rs) which each include `*_never_errors` and `refresh_vram*` tests.

3. **Removed unused `DeviceType` import from sysfs.rs**: The initial implementation imported `DeviceType` directly, but it is only obtained via `vendor_id_to_device_type()`. Removed to suppress the clippy unused import warning.

## Blockers

None.
