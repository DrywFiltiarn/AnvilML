# Plan Report: P4-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A5                                       |
| Phase       | 4 — Hardware Detection: Detectors           |
| Description | anvilml-hardware: DxgiDetector Windows fallback (cfg-gated) |
| Depends on  | P4-A4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T07:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the Windows-only `DxgiDetector` — a `DeviceDetector` implementation that enumerates GPUs via DXGI `IDXGIFactory1::EnumAdapters1` and maps each adapter to a `GpuDevice` using the same vendor-ID-to-`DeviceType` function (`vendor_id_to_device_type`) already defined in `VulkanDetector`. This detector serves as Phase 5's fallback when Vulkan enumeration returns empty on Windows. A companion test file `dxgi_tests.rs` with ≥3 tests validates vendor mapping, error resilience, and device structure. The entire module is gated `#[cfg(target_os = "windows")]` at the `mod` declaration in `lib.rs`.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/dxgi.rs` with `DxgiDetector: DeviceDetector` implementation.
- Add `mod dxgi;` gated `#[cfg(target_os = "windows")]` in `crates/anvilml-hardware/src/lib.rs`.
- Re-export `DxgiDetector` and the `dxgi` module from `lib.rs` under the same `#[cfg(target_os = "windows")]` gate.
- Add the `windows` crate dependency (with `Win32_Graphics_Dxgi` and `Win32_Graphics_Dxgi_Common` features) to `crates/anvilml-hardware/Cargo.toml`.
- Create `crates/anvilml-hardware/tests/dxgi_tests.rs` gated `#[cfg(target_os = "windows")]` at the file level.
- Bump `anvilml-hardware` patch version from `0.1.3` to `0.1.4`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. No deferred functionality.

## Existing Codebase Assessment

The `anvilml-hardware` crate already has four detector implementations: `CpuDetector` (always returns one CPU device), `MockDetector` (env-var driven, feature-gated), and `VulkanDetector` (headless Vulkan enumeration via `ash`). The `DeviceDetector` trait is defined in `detect.rs` with two methods: `detect()` and `refresh_vram()`.

Established patterns to follow:
- **Struct naming**: `XxxDetector` — a unit struct with no fields.
- **Error resilience**: `detect()` never returns `Err` or panics; loader/API absence returns `Ok(vec![])`.
- **Vendor-ID mapping**: Reuses the shared `vendor_id_to_device_type()` function re-exported from `vulkan.rs` — this ensures all three real/fallback detectors use the identical mapping (`0x10de`→`Cuda`, `0x1002`→`Rocm`).
- **GpuDevice construction**: Same field-by-field construction as in `vulkan.rs` — `enumeration_source: EnumerationSource::Dxgi`, `capabilities_source: CapabilitySource::DeviceTable` (pre-spawn hint), `vram_total_mib: 0` / `vram_free_mib: 0` (filled by `refresh_vram` later).
- **Test style**: Integration tests in `tests/` directory use `anvilml_core::types::*`, import the trait and detector, and use `#[test]` with doc comments describing what each test verifies.
- **cfg-gating pattern**: The `mock.rs` file has no internal `#[cfg(...)]` — the gate is at the `mod mock;` statement in `lib.rs`. This task follows the same pattern for `dxgi.rs`.

No gap between design doc and current source affects this approach. The design doc (§6.4) specifies the detection priority chain; the existing `VulkanDetector` already implements the same pattern with the same `GpuDevice` fields.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | windows | 0.62.2          | rust-docs MCP  | Win32_Graphics_Dxgi, Win32_Graphics_Dxgi_Common |

The `windows` crate v0.62.2 was verified via rust-docs MCP (latest version). The required feature flags `Win32_Graphics_Dxgi` and `Win32_Graphics_Dxgi_Common` were confirmed present in the feature list returned by `rust-docs_get_crate_features`.

API shape confirmed for `windows` 0.62.2:
- `CreateDXGIFactory1` — creates an `IDXGIFactory1` instance (feature: `Win32_Graphics_Dxgi`).
- `IDXGIFactory1::EnumAdapters1` — enumerates adapters as `IDXGIAdapter1` (feature: `Win32_Graphics_Dxgi`).
- `IDXGIAdapter1::GetDesc1` — returns `DXGI_ADAPTER_DESC1` (feature: `Win32_Graphics_Dxgi_Common`).
- `DXGI_ADAPTER_DESC1::Description` — `u16[128]` wide string containing the adapter name.
- `DXGI_ADAPTER_DESC1::VendorId` — `u32` (same as Vulkan's `vendor_id`).

No type named in the task context was found missing in the resolved version.

## Approach

1. **Add `windows` crate dependency to `Cargo.toml`.**
   Add the `windows` crate with `version = "0.62.2"` and enable the two required features: `Win32_Graphics_Dxgi` and `Win32_Graphics_Dxgi_Common`. These features are needed because the `windows` crate uses a feature-gated module structure — `Win32_Graphics_Dxgi` provides the DXGI interface types (`IDXGIFactory1`, `IDXGIAdapter1`, `CreateDXGIFactory1`) and `Win32_Graphics_Dxgi_Common` provides the descriptor type (`DXGI_ADAPTER_DESC1`).

2. **Create `crates/anvilml-hardware/src/dxgi.rs`.**
   Implement the following in the new file (no internal `#[cfg(...)]` — the gate is at the `mod` statement):

   a. **Struct definition**: `pub struct DxgiDetector;` — a unit struct, matching the pattern of `VulkanDetector` and `CpuDetector`.

   b. **Imports**: `use crate::detect::DeviceDetector;`, `use anvilml_core::{AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};`, `use anvilml_hardware::vendor_id_to_device_type;`, and the Windows types:
   ```rust
   use windows::{
       core::Result as WinResult,
       Win32::Graphics::Dxgi::*,
       Win32::Graphics::Dxgi::Common::DXGI_ADAPTER_DESC1,
   };
   ```

   c. **`detect()` implementation**:
      - Call `CreateDXGIFactory1()` to obtain an `IDXGIFactory1`. On failure (access denied, COM initialization failure, etc.), log at DEBUG level and return `Ok(vec![])` — never panic or return `Err`.
      - Call `factory.EnumAdapters1(i)` in a loop starting from `i = 0`. When `EnumAdapters1` returns an error (indicating no more adapters), break the loop. Each successful call yields an `IDXGIAdapter1`.
      - For each adapter, call `adapter.GetDesc1()` to obtain `DXGI_ADAPTER_DESC1`. On failure, log at DEBUG and skip that adapter (continue to next).
      - Map `desc.VendorId` using `vendor_id_to_device_type(desc.VendorId)`. If `None`, skip the adapter (not a compute backend we target).
      - Convert the `u16[128]` `Description` field to a `String` by finding the first null byte and decoding as UTF-16LE. If no null byte is found, use the full 128 elements. If decoding fails, use `"Unknown device"`.
      - Extract `pci_vendor_id = (desc.VendorId & 0xFFFF) as u16` (same masking as Vulkan). The `DeviceId` field from `DXGI_ADAPTER_DESC1` is not available in `GetDesc1` (only in `GetDesc` which returns the older struct) — set `pci_device_id = 0` for now, noting this limitation.
      - Construct `GpuDevice` with `index: i as u32`, `enumeration_source: EnumerationSource::Dxgi`, `capabilities_source: CapabilitySource::DeviceTable`, `vram_total_mib: 0`, `vram_free_mib: 0`.
      - Log at DEBUG level: vendor_id, device_name, index.
      - Push to the devices vector and continue to next adapter.
      - Return `Ok(devices)`.

   d. **`refresh_vram()` implementation**:
      - DXGI does not provide a direct VRAM query API equivalent to Vulkan's memory budget extension. Return `Ok((0, 0))` — same as Vulkan's fallback when memory budget is unavailable. This is consistent with the design: VRAM refresh is best-effort; `(0, 0)` signals "unknown" to the caller.

   e. **Doc comments**: Every `pub` item gets a `///` doc comment. The `detect()` method documents the DXGI API contract and the "never panic" guarantee. The `refresh_vram()` method documents why it returns `(0, 0)`.

3. **Update `crates/anvilml-hardware/src/lib.rs`.**
   Add two lines under the existing `pub mod` declarations:
   ```rust
   #[cfg(target_os = "windows")]
   pub mod dxgi;
   #[cfg(target_os = "windows")]
   pub use dxgi::DxgiDetector;
   ```
   These go after the `vulkan` module exports (keeping platform-specific modules grouped together). The `dxgi.rs` file itself has no `#[cfg(...)]` — the gate is only at the `mod` statement, matching the pattern used by `mock.rs`.

4. **Create `crates/anvilml-hardware/tests/dxgi_tests.rs`.**
   Create a test file gated `#[cfg(target_os = "windows")]` at the file level (the first line of the file). Include ≥3 tests:

   a. **`test_dxgi_nvidia_vendor_maps_to_cuda`**: Import `vendor_id_to_device_type` and verify `0x10de` → `Some(DeviceType::Cuda)`. This is a pure function test — no Windows API calls needed. (Same pattern as vulkan tests.)

   b. **`test_dxgi_amd_vendor_maps_to_rocm`**: Verify `0x1002` → `Some(DeviceType::Rocm)`. Pure function test.

   c. **`test_dxgi_detect_never_errors`**: Construct `DxgiDetector`, call `detect()`, assert `result.is_ok()`. On Windows with GPUs, this returns detected devices; on headless/CI Windows, it returns `Ok(vec![])`. The invariant is: no panic, no `Err`.

   d. **`test_dxgi_refresh_vram_never_errors`**: Call `refresh_vram(0)`, assert `result.is_ok()` and the result is `Ok((0, 0))` (DXGI has no VRAM query API).

5. **Bump version.** Update `crates/anvilml-hardware/Cargo.toml`: `version = "0.1.3"` → `version = "0.1.4"`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `DxgiDetector` | `anvilml_hardware::dxgi::DxgiDetector` | `pub struct DxgiDetector;` |
| `DeviceDetector impl` | `anvilml_hardware::dxgi` | `impl DeviceDetector for DxgiDetector` |
| `detect()` | `anvilml_hardware::dxgi::DxgiDetector` | `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` |
| `refresh_vram()` | `anvilml_hardware::dxgi::DxgiDetector` | `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` |

No new types or traits are introduced. `DxgiDetector` implements the existing `DeviceDetector` trait.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/dxgi.rs` | `DxgiDetector` implementation |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `mod dxgi;` and `pub use dxgi::DxgiDetector;` gated `#[cfg(target_os = "windows")]` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Add `windows` crate dependency + bump patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-hardware/tests/dxgi_tests.rs` | Integration tests, gated `#[cfg(target_os = "windows")]` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/dxgi_tests.rs` | `test_dxgi_nvidia_vendor_maps_to_cuda` | `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)` | None | Vendor ID `0x10de` | `Some(DeviceType::Cuda)` | `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_nvidia_vendor_maps_to_cuda` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_tests.rs` | `test_dxgi_amd_vendor_maps_to_rocm` | `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)` | None | Vendor ID `0x1002` | `Some(DeviceType::Rocm)` | `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_amd_vendor_maps_to_rocm` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_tests.rs` | `test_dxgi_detect_never_errors` | `DxgiDetector::detect()` returns `Ok(...)` — never panics or returns `Err` | None | `DxgiDetector` constructed | `result.is_ok()` | `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_detect_never_errors` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_tests.rs` | `test_dxgi_refresh_vram_never_errors` | `DxgiDetector::refresh_vram(0)` returns `Ok((0, 0))` — DXGI has no VRAM query API | None | index `0` | `Ok((0, 0))` | `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_refresh_vram_never_errors` exits 0 |

## CI Impact

The test file `dxgi_tests.rs` is gated `#[cfg(target_os = "windows")]` at the file level. On Linux CI (`rust-linux` job), this compiles to an empty test binary — no tests are collected, no failures occur. On the `rust-windows` job (`windows-latest` runner), the tests compile and run for real. No CI workflow changes are needed — the existing `rust-windows` job already runs `cargo test --workspace --features mock-hardware`, which includes this test file.

## Platform Considerations

This task introduces `#[cfg(target_os = "windows")]` gating at two locations:
1. `lib.rs`: `mod dxgi;` and `pub use dxgi::DxgiDetector;` are gated.
2. `tests/dxgi_tests.rs`: The entire file is gated `#[cfg(target_os = "windows")]` on the first line.

On Linux (the primary development platform), the `dxgi.rs` module is not compiled and the test file is a no-op. The Windows cross-check (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) exercises the Windows compilation path and will catch any platform-specific issues (e.g., missing `extern crate windows`, incorrect feature flags).

The `windows` crate v0.62.2 uses COM internally — `CreateDXGIFactory1` calls `CoCreateInstance` under the hood. On Windows, COM must be initialized before use. The `windows` crate handles this via `windows::core::Connect` or the caller must call `CoInitializeEx`. For this detector, `CreateDXGIFactory1` works without explicit COM initialization in most cases (the `windows` crate auto-initializes COM on first use), but if a `CoInitialize` error occurs, it propagates as a `WinResult` error, which we handle by returning `Ok(vec![])`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `windows` crate v0.62.2 may require explicit COM initialization (`CoInitializeEx`) before `CreateDXGIFactory1` succeeds. Without it, the call may fail with `RPC_E_WRONG_THREAD` or `CO_E_NOTINITIALIZED`, causing `detect()` to return `Ok(vec![])` even on a Windows system with GPUs. | Low | Medium | The `windows` crate's `CreateDXGIFactory1` internally calls `CoCreateInstance` which auto-initializes COM on first use. If this assumption breaks, the fallback to CPU detection (Phase 5) still works — the worst case is DXGI finds zero devices and Vulkan+CPU handle detection. Document this in a `// TODO` comment if needed. |
| `DXGI_ADAPTER_DESC1` does not include `DeviceId` — only `VendorId`, `Description`, `SubSysId`, `Revision`, `DedicatedVideoMemory`, `DedicatedSystemMemory`, `SharedSystemMemory`, `AdapterLuid`. The Vulkan detector sets `pci_device_id` from `props.device_id`. Without `DeviceId`, we must set `pci_device_id = 0`. | High | Low | This is an inherent limitation of DXGI's `GetDesc1` API. The `AdapterLuid` can serve as a unique device identifier if needed in the future. Document this gap in a `// TODO: pci_device_id unavailable from DXGI_ADAPTER_DESC1` comment. |
| The `windows` crate feature set (`Win32_Graphics_Dxgi` + `Win32_Graphics_Dxgi_Common`) may have transitive dependencies that increase compile time or binary size on Windows. | Low | Low | The `windows` crate is already used by `anvilml-worker`'s `job_object.rs` (Windows Job Object wrapper). If it's already a transitive dependency, adding it explicitly is free. If not, the compile-time cost is minimal (~1-2s on a cold build). |
| UTF-16 decoding of `DXGI_ADAPTER_DESC1::Description` (a `u16[128]` array) may produce unexpected characters if the adapter name contains non-UTF-16 sequences. | Low | Low | Use `String::from_utf16_lossy()` on the slice up to the first null byte — this handles invalid sequences gracefully by replacing them with the replacement character, matching the approach used in `vulkan.rs` for `CStr::to_string_lossy()`. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-hardware --features mock-hardware` exits 0 (Linux compilation — dxgi module not compiled)
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check — dxgi module compiled)
- [ ] `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_nvidia_vendor_maps_to_cuda` exits 0 (on Windows)
- [ ] `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_amd_vendor_maps_to_rocm` exits 0 (on Windows)
- [ ] `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_detect_never_errors` exits 0 (on Windows)
- [ ] `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_refresh_vram_never_errors` exits 0 (on Windows)
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
