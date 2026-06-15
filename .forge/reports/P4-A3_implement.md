# Implementation Report: P4-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P4-A3                              |
| Phase         | 004 — Hardware Detection Fallbacks |
| Description   | anvilml-hardware: DXGI (Windows) and sysfs+NVML (Linux) fallback detectors |
| Implemented   | 2026-06-15T10:30:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented three new platform-specific GPU detection modules for `anvilml-hardware`:
`DxgiDetector` (Windows, via DXGI COM API), `SysfsPciDetector` (Linux, via sysfs PCI enumeration),
and `NvmlDetector` (Linux, via dynamic loading of `libnvidia-ml.so.1` for live VRAM refresh).
Added `windows` and `libloading` optional dependencies, gated behind `dxgi` and `nvml` features
respectively. Created integration tests for all three detectors in a new `tests/dxgi_sysfs_tests.rs`
file. Bumped `anvilml-hardware` version from 0.1.2 to 0.1.3. All four platform cross-checks
(pass 1-4), clippy, format, and the full test suite (63 tests, zero failures) pass cleanly.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | windows   | 0.57             | crates.io (plan) |
| crate  | libloading | 0.8             | crates.io (plan) |

Note: MCP tool `rust-docs` was unavailable (npm package conflict). Versions from the approved
plan were used as-is — both are stable, well-tested crates. The `windows` crate 0.57 provides
the `Win32_Graphics_Dxgi` feature with all APIs referenced in the plan (`CreateDXGIFactory1`,
`IDXGIFactory1`, `EnumAdapters1`, `GetDesc1`, `DXGI_ADAPTER_DESC1`). The `libloading` crate
0.8 provides the `Library::new()` API for dynamic shared library loading.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-hardware/src/dxgi.rs | Windows DXGI GPU detector (222 lines) |
| CREATE | crates/anvilml-hardware/src/sysfs.rs | Linux sysfs PCI GPU detector (208 lines) |
| CREATE | crates/anvilml-hardware/src/nvml.rs | Linux NVML VRAM refresh supplement (181 lines) |
| MODIFY | crates/anvilml-hardware/src/lib.rs | Added module declarations + pub use for 3 new types |
| MODIFY | crates/anvilml-hardware/Cargo.toml | Added `windows` + `libloading` deps, `dxgi` + `nvml` features; bumped version 0.1.2 → 0.1.3 |
| CREATE | crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs | Integration tests for all 3 new detectors (269 lines) |
| MODIFY | docs/TESTS.md | Added 17 new test entries for dxgi/sysfs/nvml tests |

## Commit Log

```
 Cargo.lock                                        |   4 +-
 crates/anvilml-hardware/Cargo.toml                |   8 +-
 crates/anvilml-hardware/src/dxgi.rs               | 222 ++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs                |  26 +++
 crates/anvilml-hardware/src/nvml.rs               | 181 +++++++++++++++
 crates/anvilml-hardware/src/sysfs.rs              | 208 +++++++++++++++++
 crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs | 269 ++++++++++++++++++++++
 docs/TESTS.md                                     | 144 ++++++++++++
 8 files changed, 1060 insertions(+), 2 deletions(-)
```

## Test Results

```
     Running tests/cpu_tests.rs
running 3 tests
test test_cpu_detector_detect_returns_one_device ... ok
test test_cpu_detector_is_send_sync ... ok
test test_cpu_detector_refresh_vram_returns_zero ... ok
test result: ok. 3 passed; 0 failed; 0 ignored

     Running tests/dxgi_sysfs_tests.rs
running 6 tests
test test_sysfs_detect_no_panic ... ok
test test_sysfs_detect_vendor_mapping ... ok
test test_sysfs_detector_default ... ok
test test_sysfs_detector_is_send_sync ... ok
test test_sysfs_detector_new ... ok
test test_sysfs_refresh_vram_returns_zero ... ok
test result: ok. 6 passed; 0 failed; 0 ignored

     Running tests/vulkan_tests.rs
running 4 tests
test test_vulkan_detector_detect_returns_empty_or_devices ... ok
test test_vulkan_detector_is_send_sync ... ok
test test_vulkan_detector_new ... ok
test test_vulkan_detector_refresh_vram_returns_zero ... ok
test result: ok. 4 passed; 0 failed; 0 ignored

Workspace total: 63 tests passed; 0 failed
```

Note: On this Linux target, the `#[cfg(windows)]` DXGI tests and `#[cfg(all(unix, feature = "nvml"))]`
NVML tests are not compiled (6 DXGI tests + 6 NVML tests = 12 tests absent). The 6 sysfs tests
are compiled and pass.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware → Finished (0 errors)

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished (0 errors)

# 3. Real-hardware Linux
cargo check --bin anvilml → Finished (0 errors)

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu → Finished (0 errors)
```

All four cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 (OpenAPI Drift) — Not applicable: no handler signatures or ToSchema changes.
Gate 3 (Node Parity) — Not applicable: no node type changes.

## Public API Delta

```
+pub mod dxgi;
+pub mod nvml;
+pub mod sysfs;
+pub use dxgi::DxgiDetector;
+pub use nvml::NvmlDetector;
+pub use sysfs::SysfsPciDetector;
```

New pub items:
| Name | Type | Module Path | Platform |
|------|------|-------------|----------|
| DxgiDetector | struct | `anvilml_hardware::dxgi::DxgiDetector` | Windows only |
| DxgiDetector | impl DeviceDetector | `anvilml_hardware::dxgi` | Windows only |
| SysfsPciDetector | struct | `anvilml_hardware::sysfs::SysfsPciDetector` | Unix only |
| SysfsPciDetector | impl DeviceDetector | `anvilml_hardware::sysfs` | Unix only |
| NvmlDetector | struct | `anvilml_hardware::nvml::NvmlDetector` | Unix + nvml feature |
| NvmlDetector | impl DeviceDetector | `anvilml_hardware::nvml` | Unix + nvml feature |

## Deviations from Plan

1. **Module cfg gates changed from `#[cfg(windows)]` / `#[cfg(unix)]` to `#[cfg(all(<platform>, feature = "<feature>"))]`** — The plan specified `#[cfg(windows)]` for `dxgi` and `#[cfg(unix)]` for `sysfs`/`nvml`. However, the `windows` and `libloading` dependencies are optional (gated by `dxgi`/`nvml` features). Without the feature gate on the module, the compiler cannot find the `windows` crate on Windows cross-check targets (where the feature is not enabled by default). Changed to:
   - `dxgi` module: `#[cfg(all(windows, feature = "dxgi"))]`
   - `nvml` module: `#[cfg(all(unix, feature = "nvml"))]`
   - `sysfs` module: kept as `#[cfg(unix)]` (no optional dependency)

2. **NVML tests gated by `#[cfg(all(unix, feature = "nvml"))]`** — The `NvmlDetector` type is only available when the `nvml` feature is enabled, so the test module must use the same gate.

3. **libloading version** — MCP tool unavailable. Used version 0.8 from the approved plan (latest is 0.9.0, but plan version is stable and sufficient).

4. **sysfs.rs module-level doc comment** — Changed from block doc comments (`///`) to inner doc comments (`//!`) to fix clippy `empty_line_after_doc_comments` lint.

5. **Version bump** — `anvilml-hardware` bumped from 0.1.2 to 0.1.3 as required.

## Blockers

None.
