# Implementation Report: P4-A5

| Field         | Value                                                      |
|---------------|------------------------------------------------------------|
| Task ID       | P4-A5                                                      |
| Phase         | 4 — Hardware Detection: Detectors                          |
| Description   | anvilml-hardware: DxgiDetector Windows fallback (cfg-gated) |
| Implemented   | 2026-06-29T08:15:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Implemented the `DxgiDetector` — a Windows-only GPU detector that enumerates adapters via the DXGI `IDXGIFactory1::EnumAdapters1` API. The detector maps each adapter to a `GpuDevice` using the shared `vendor_id_to_device_type` function, filtering to NVIDIA/CUDA and AMD/ROCm vendors only. The entire module is gated `#[cfg(target_os = "windows")]` at the `mod` statement in `lib.rs`, matching the established pattern used by `mock.rs`. The `windows` crate dependency was added with `default-features = false` and the two required DXGI features.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         | Notes                                   |
|--------|----------|------------------|----------------|-----------------------------------------|
| crate  | windows  | 0.58.0           | rust-docs MCP  | Plan specified 0.62.2 but it had broken transitive deps (windows-future 0.3.2 incompatible with windows-core 0.62.2) |

**Deviation from plan:** The plan specified `windows` crate v0.62.2, but this version has a known transitive dependency bug where `windows-future` 0.3.2 references `windows_core::imp::IMarshal` which does not exist in `windows-core` 0.62.2. Version 0.58.0 with `default-features = false` compiles correctly and provides the same DXGI API surface (`CreateDXGIFactory1`, `IDXGIFactory1`, `EnumAdapters1`, `GetDesc1`, `DXGI_ADAPTER_DESC1`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/dxgi.rs` | `DxgiDetector` unit struct with `DeviceDetector` impl (detect + refresh_vram) |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `#[cfg(target_os = "windows")] pub mod dxgi;` and `pub use dxgi::DxgiDetector;` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Added `windows = { version = "0.58", default-features = false, features = [...] }` dependency; bumped version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-hardware/tests/dxgi_tests.rs` | 4 integration tests gated `#[cfg(target_os = "windows")]` |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for dxgi tests |

## Commit Log

```
.forge/reports/P4-A5_plan.md                | 181 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  | 145 +++++++++++++++++++--
 crates/anvilml-hardware/Cargo.toml          |   6 +-
 crates/anvilml-hardware/src/dxgi.rs         | 192 ++++++++++++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs          |   5 +
 crates/anvilml-hardware/tests/dxgi_tests.rs |  83 ++++++++++++
 docs/TESTS.md                               |  48 +++++++
 9 files changed, 661 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running tests/dxgi_tests.rs (target/debug/deps/dxgi_tests-6bac4d8b557edf82)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

On Linux (the primary dev platform), the dxgi tests file compiles to 0 tests due to `#[cfg(target_os = "windows")]` gating. On Windows, all 4 tests would compile and run:
- `test_dxgi_nvidia_vendor_maps_to_cuda` — pure function test
- `test_dxgi_amd_vendor_maps_to_rocm` — pure function test
- `test_dxgi_detect_never_errors` — error resilience test
- `test_dxgi_refresh_vram_never_errors` — VRAM fallback test

All 100+ workspace tests passed with zero failures.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

=== Check 2: Mock-hardware Windows ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s

=== Check 3: Real-hardware Linux ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s

=== Check 4: Real-hardware Windows ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
```

All four platform cross-checks pass.

## Project Gates

None defined for this task (no ServerConfig changes, no handler signature changes, no node type changes).

## Public API Delta

```
+pub mod dxgi;
+pub use dxgi::DxgiDetector;
```

Two new `pub` items in `lib.rs`:
- `pub mod dxgi` — module declaration (cfg-gated)
- `pub use dxgi::DxgiDetector` — re-export (cfg-gated)

The `DxgiDetector` struct and its `DeviceDetector` trait methods are `pub` within the `dxgi` module, matching the established pattern of `VulkanDetector` and `MockDetector`.

## Deviations from Plan

- **Dependency version:** The plan specified `windows` crate v0.62.2, but this version has a broken transitive dependency tree — `windows-future` 0.3.2 is incompatible with `windows-core` 0.62.2 (references `windows_core::imp::IMarshal` which does not exist). Used `windows` v0.58.0 with `default-features = false` instead. The DXGI API surface is identical across versions.
- **Type annotation:** Added explicit `IDXGIFactory1` type annotation on the `factory` variable to resolve type inference on the Windows target.
- **Import path:** Used `crate::vendor_id_to_device_type` instead of `anvilml_hardware::vendor_id_to_device_type` for the self-referential import within the crate.
- **Import path:** Used `crate::detect::DeviceDetector` instead of `crate::detect::DeviceDetector` — matches the existing vulkan.rs pattern.
- **DXGI_ADAPTER_DESC1 path:** In v0.58.0, `DXGI_ADAPTER_DESC1` is in `Win32::Graphics::Dxgi` directly (not in `Win32::Graphics::Dxgi::Common` as the plan specified). The `use windows::Win32::Graphics::Dxgi::*;` wildcard import covers it.

## Blockers

None.
