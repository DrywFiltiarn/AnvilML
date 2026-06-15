# Plan Report: P4-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A3                                       |
| Phase       | 004 — Hardware Detection                    |
| Description | anvilml-hardware: DXGI (Windows) and sysfs+NVML (Linux) fallback detectors |
| Depends on  | P4-A1, P4-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T09:30:00Z                        |
| Attempt     | 1                                             |

## Objective

Create three new modules in `anvilml-hardware` — `dxgi.rs` (Windows DXGI fallback), `sysfs.rs` (Linux PCI sysfs fallback), and `nvml.rs` (Linux NVML VRAM refresh) — each implementing the `DeviceDetector` trait. This enables the hardware detection pipeline to fall back to platform-native APIs when Vulkan returns no devices, ensuring GPU detection works on systems without Vulkan drivers. The observable state after completion is that `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0, confirming the Windows code path compiles cleanly under cross-compilation from Linux.

## Scope

### In Scope
- **CREATE** `crates/anvilml-hardware/src/dxgi.rs`: `DxgiDetector` struct implementing `DeviceDetector` using `windows` crate DXGI COM APIs (`IDXGIFactory1::EnumAdapters1`, `IDXGIAdapter1::GetDesc1`, `DXGI_ADAPTER_DESC1`).
- **CREATE** `crates/anvilml-hardware/src/sysfs.rs`: `SysfsPciDetector` struct implementing `DeviceDetector` by reading `/sys/bus/pci/devices/*/vendor` and `/sys/bus/pci/devices/*/device` files, mapping vendor IDs to `DeviceType`, and constructing `GpuDevice` entries.
- **CREATE** `crates/anvilml-hardware/src/nvml.rs`: `NvmlDetector` struct implementing `DeviceDetector` with a `refresh_vram` that dynamically loads `libnvidia-ml.so` via `libloading` and queries `nvmlDeviceGetMemoryInfo`. On systems where `libnvidia-ml.so` is absent, returns `(0, 0)` gracefully.
- **MODIFY** `crates/anvilml-hardware/src/lib.rs`: Add `pub mod dxgi`, `pub mod sysfs`, `pub mod nvml` declarations with `#[cfg(windows)]` and `#[cfg(unix)]` guards. Add `pub use` for the new types.
- **MODIFY** `crates/anvilml-hardware/Cargo.toml`: Add `windows = { version = "0.57", features = ["Win32_Graphics_Dxgi"], optional = true }` behind a `dxgi` feature, and `libloading` dependency (optional, unix only).
- **CREATE** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`: Integration tests for `DxgiDetector` and `SysfsPciDetector`.
- **BUMP** `crates/anvilml-hardware` version from `0.1.2` to `0.1.3`.

### Out of Scope
- `device_db.rs` (PCI-ID capability table resolution) — handled by P4-A4.
- `detect_all_devices` orchestration logic — handled by P4-A5.
- NVML `detect` method: this task only implements `refresh_vram` for NVML; `detect` returns `Ok(vec![])` since NVML is a VRAM refresh supplement, not a primary enumerator.
- Windows real-hardware compilation (P4-A3's acceptance criterion is the mock-hardware cross-check only).
- Platform-specific tests for real hardware (those require physical GPUs).

## Existing Codebase Assessment

The `anvilml-hardware` crate currently has three source files: `lib.rs` (exports `DeviceDetector` trait, `CpuDetector`, and `VulkanDetector`), `cpu.rs` (synthesises a CPU device via `sysinfo`), and `vulkan.rs` (SDK-free Vulkan enumeration via `ash`). Both existing detectors follow the same pattern: zero-sized unit structs, `Default` + `new()` constructors, and `DeviceDetector` trait implementations that log detected devices at INFO level and internal state at DEBUG level.

The `DeviceDetector` trait (defined in `lib.rs` lines 23–45) requires `Send + Sync`, and declares two methods: `detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` and `refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>`. All existing implementations return `Err(AnvilError::Io(...))` only for truly unrecoverable failures; the `detect` method returns `Ok(vec![])` (empty list) when no devices are found — never panics.

The `GpuDevice` type from `anvilml-core` (in `types/hardware.rs`) has 13 fields including `enumeration_source: EnumerationSource` and `capabilities_source: CapabilitySource` — both enums are defined in the same file with variants `Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Mock`, `Override` and `PyTorch`, `DeviceTable`, `Fallback` respectively.

No discrepancy between the design doc and current source: the `DeviceDetector` trait signature matches the design spec exactly, and the `EnumerationSource::Dxgi` and `EnumerationSource::Sysfs` variants already exist in `anvilml-core`.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | windows      | 0.57            | Cargo.lock fallback (MCP rust-docs unavailable) | Win32_Graphics_Dxgi |
| crate  | libloading   | 0.8             | Cargo.lock fallback (MCP rust-docs unavailable) | n/a |

**Note:** MCP `rust-docs` tool was not invoked (not available as a direct tool call). Versions taken from Cargo.lock for `windows` (0.57.0) and crates.io latest for `libloading` (0.8.x). The ACT agent MUST verify both versions via `rust-docs` MCP before writing any `Cargo.toml` entry. The `windows` crate 0.57 already has `IDXGIFactory1`, `EnumAdapters1`, `IDXGIAdapter1`, and `DXGI_ADAPTER_DESC1` types in the `Win32_Graphics_Dxgi` feature.

## Approach

1. **Add dependencies to `Cargo.toml`.** Add two optional dependencies:
   - `windows = { version = "0.57", features = ["Win32_Graphics_Dxgi"], optional = true }` — the `optional = true` allows the crate to compile without the Windows crate on non-Windows hosts, and the `dxgi` feature gate controls whether it's linked.
   - `libloading = { version = "0.8", optional = true }` — for dynamic loading of `libnvidia-ml.so` at runtime. Marked optional so it doesn't pull in on non-unix or CI builds where the library is absent.
   - Add a new feature `nvml = ["libloading"]` to gate the NVML code.
   - The `windows` dependency is gated by a `dxgi` feature: `dxgi = ["windows"]`. This way, when compiling for `x86_64-pc-windows-gnu` on Linux, the `dxgi` feature can be enabled (or the `#[cfg(windows)]` in lib.rs ensures the module is included), and the `windows` crate will resolve correctly.

   Rationale: Making `windows` optional with a feature gate avoids pulling the Windows SDK into non-Windows builds, which would otherwise fail on Linux/Mac even though the code is `#[cfg(windows)]`-gated. Some transitive dependencies of the `windows` crate may not compile on non-Windows targets.

2. **Create `src/dxgi.rs`** (`#[cfg(windows)]`).
   - Define `pub struct DxgiDetector;` (zero-sized unit struct).
   - Implement `new()` and `Default` (following the pattern from `cpu.rs` and `vulkan.rs`).
   - Implement `DeviceDetector`:
     - `detect()`: Initialize COM via `windows::Win32::Graphics::Dxgi::CreateDXGIFactory1` to get `IDXGIFactory1`. Call `EnumAdapters1(0)`, `EnumAdapters1(1)`, etc. in a loop until `S_FALSE` (no more adapters). For each adapter, call `GetDesc1()` to get `DXGI_ADAPTER_DESC1`. Extract `Description` (convert from `u16` wide string), `VendorId`, `DeviceId`, `DedicatedVideoMemory` (VRAM total in bytes → MiB). Map vendor ID: `0x10de` → `Cuda`, `0x1002` → `Rocm`, else → `Cpu`. Set `enumeration_source = EnumerationSource::Dxgi` and `capabilities_source = CapabilitySource::Fallback`. Log each device at INFO. Return `Ok(vec![])` if COM initialization fails.
     - `refresh_vram()`: Returns `(0, 0)` — live VRAM refresh via DXGI is not supported without a device context. NVML handles this on NVIDIA systems.
   - Log "DXGI detection fallback used" at DEBUG per mandatory log point for Hardware subsystem.

3. **Create `src/sysfs.rs`** (`#[cfg(unix)]`).
   - Define `pub struct SysfsPciDetector;` (zero-sized unit struct).
   - Implement `new()` and `Default`.
   - Implement `DeviceDetector`:
     - `detect()`: Walk `/sys/bus/pci/devices/` using `std::fs::read_dir`. For each device directory, read `vendor` and `device` files. Strip "0x" prefix and parse as hex u16. Skip entries where vendor is `0x0000` (no device) or `0xffff` (placeholder). Map vendor ID: `0x10de` → `Cuda`, `0x1002` → `Rocm`, else → `Cpu`. Use the directory name (e.g., "0000:01:00.0") as a partial device identifier. Since sysfs doesn't provide device name or driver version directly, set `name` to `"PCI GPU (vendor={vendor_id:04x}, device={device_id:04x})"` and `driver_version` to `"unknown"`. Set `vram_total_mib` and `vram_free_mib` to `0` (VRAM comes from NVML). Set `enumeration_source = EnumerationSource::Sysfs`. Log each device at INFO. Return `Ok(vec![])` if the sysfs path doesn't exist or is unreadable.
     - `refresh_vram()`: Returns `(0, 0)` — VRAM refresh is handled by `NvmlDetector`.
   - Log "sysfs detection fallback used" at DEBUG.

4. **Create `src/nvml.rs`** (`#[cfg(unix)]`).
   - Define `pub struct NvmlDetector;` (zero-sized unit struct).
   - Implement `new()` and `Default`.
   - Implement `DeviceDetector`:
     - `detect()`: Returns `Ok(vec![])` — NVML is a VRAM refresh supplement, not a primary device enumerator. The actual device enumeration happens via Vulkan, DXGI, or sysfs. NVML only provides live VRAM data.
     - `refresh_vram()`: Attempt to load `libnvidia-ml.so.1` via `libloading::Library::new("libnvidia-ml.so.1")`. If the library is absent (common on non-NVIDIA systems), return `Ok((0, 0))` gracefully — never error. If loaded successfully, resolve the `nvmlDeviceGetMemoryInfo` symbol. Build a fake `nvmlDevice_t` handle (the NVML API uses an opaque pointer; for a single-device system, we can use a non-null dummy pointer like `0x1 as *mut _`). Call `nvmlDeviceGetMemoryInfo(handle)` and extract `total` and `free` from the returned `nvmlMemory_t` struct. Convert from bytes to MiB. Return `(total_mib, free_mib)`. If any symbol resolution or library call fails, return `Ok((0, 0))`.
   - Log "NVML VRAM refresh unavailable (libnvidia-ml.so not found)" at DEBUG when the library is absent.

5. **Modify `src/lib.rs`**. Add three module declarations with platform guards:
   ```rust
   #[cfg(windows)]
   pub mod dxgi;
   #[cfg(unix)]
   pub mod sysfs;
   #[cfg(unix)]
   pub mod nvml;
   ```
   Add `pub use` statements for the new types. Keep the file under 80 lines.

6. **Create `tests/dxgi_sysfs_tests.rs`**. Write integration tests:
   - `test_dxgi_detector_new`: Verify `DxgiDetector::new()` and `Default` construct successfully.
   - `test_dxgi_detect_empty_on_non_windows`: On non-Windows targets, this test is cfg-gated and skipped.
   - `test_sysfs_detector_new`: Verify `SysfsPciDetector::new()` and `Default` construct.
   - `test_sysfs_detect_no_pci`: On systems without `/sys/bus/pci/devices/` (e.g., WSL2, VMs), `detect()` returns `Ok(vec![])`.
   - `test_sysfs_detect_vendor_mapping`: Verify vendor ID `0x10de` maps to `DeviceType::Cuda`, `0x1002` to `Rocm`.
   - `test_nvml_detector_new`: Verify `NvmlDetector::new()` and `Default` construct.
   - `test_nvml_refresh_vram_no_library`: On systems without `libnvidia-ml.so`, `refresh_vram()` returns `Ok((0, 0))`.

   Rationale: Integration tests in `tests/` use the crate's public API, forcing correct visibility. Unit tests for the platform-specific detection logic (actual device enumeration) require physical hardware and are deferred to later tasks.

7. **Bump version** of `anvilml-hardware` from `0.1.2` to `0.1.3` in `Cargo.toml`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `DxgiDetector` | struct | `anvilml_hardware::dxgi::DxgiDetector` | `pub struct DxgiDetector;` |
| `DxgiDetector::new` | fn | `anvilml_hardware::dxgi::DxgiDetector::new` | `pub const fn new() -> Self` |
| `DxgiDetector` impl DeviceDetector | trait impl | `anvilml_hardware::dxgi` | `impl DeviceDetector for DxgiDetector` |
| `SysfsPciDetector` | struct | `anvilml_hardware::sysfs::SysfsPciDetector` | `pub struct SysfsPciDetector;` |
| `SysfsPciDetector::new` | fn | `anvilml_hardware::sysfs::SysfsPciDetector::new` | `pub const fn new() -> Self` |
| `SysfsPciDetector` impl DeviceDetector | trait impl | `anvilml_hardware::sysfs` | `impl DeviceDetector for SysfsPciDetector` |
| `NvmlDetector` | struct | `anvilml_hardware::nvml::NvmlDetector` | `pub struct NvmlDetector;` |
| `NvmlDetector::new` | fn | `anvilml_hardware::nvml::NvmlDetector::new` | `pub const fn new() -> Self` |
| `NvmlDetector` impl DeviceDetector | trait impl | `anvilml_hardware::nvml` | `impl DeviceDetector for NvmlDetector` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/dxgi.rs` | DxgiDetector: Windows DXGI COM-based GPU enumeration |
| CREATE | `crates/anvilml-hardware/src/sysfs.rs` | SysfsPciDetector: Linux PCI sysfs-based GPU enumeration |
| CREATE | `crates/anvilml-hardware/src/nvml.rs` | NvmlDetector: Linux NVML-based VRAM refresh |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add module declarations and pub use for dxgi, sysfs, nvml |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Add windows + libloading optional deps, dxgi/nvml features, bump version 0.1.2 → 0.1.3 |
| CREATE | `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | Integration tests for new detectors |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_dxgi_detector_new` | `DxgiDetector::new()` and `Default::default()` construct successfully | `#[cfg(windows)]` | None | Construction succeeds, no panic | `cargo test -p anvilml-hardware --features mock-hardware -- dxgi` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_dxgi_detect_no_devices` | On Windows without GPUs, `detect()` returns `Ok(vec![])` | `#[cfg(windows)]`, no GPU | None | Empty device list | `cargo test -p anvilml-hardware --features mock-hardware -- dxgi` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_sysfs_detector_new` | `SysfsPciDetector::new()` and `Default::default()` construct successfully | `#[cfg(unix)]` | None | Construction succeeds | `cargo test -p anvilml-hardware --features mock-hardware -- sysfs` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_sysfs_detect_no_pci` | On systems without `/sys/bus/pci/devices/`, `detect()` returns `Ok(vec![])` | `#[cfg(unix)]`, no PCI sysfs | None | Empty device list | `cargo test -p anvilml-hardware --features mock-hardware -- sysfs` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_nvml_detector_new` | `NvmlDetector::new()` and `Default::default()` construct successfully | `#[cfg(unix)]` | None | Construction succeeds | `cargo test -p anvilml-hardware --features mock-hardware -- nvml` exits 0 |
| `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs` | `test_nvml_refresh_vram_no_library` | On systems without `libnvidia-ml.so`, `refresh_vram()` returns `Ok((0, 0))` | `#[cfg(unix)]`, no NVML library | None | `(0, 0)` tuple | `cargo test -p anvilml-hardware --features mock-hardware -- nvml` exits 0 |

## CI Impact

No CI changes required. The new modules are behind `#[cfg(windows)]` and `#[cfg(unix)]` guards, so the CI jobs (`rust-linux`, `rust-windows`) pick them up automatically through the existing workspace test command. The `windows` crate dependency is optional and gated by the `dxgi` feature, so it won't affect non-Windows CI builds. The new test file follows the existing convention of `crates/{name}/tests/` and is automatically discovered by `cargo test --workspace`.

## Platform Considerations

- `dxgi.rs` is guarded by `#[cfg(windows)]` — only compiled on Windows targets. Uses COM initialization (`CoInitializeEx`), DXGI factory creation, and adapter enumeration. The `windows` crate provides safe Rust bindings for these COM APIs.
- `sysfs.rs` is guarded by `#[cfg(unix)]` — only compiled on Unix-like systems. Reads `/sys/bus/pci/devices/*/vendor` and `device` files using `std::fs::read_to_string`. Does not require any special permissions beyond read access to sysfs (available to all users).
- `nvml.rs` is guarded by `#[cfg(unix)]` — only compiled on Unix-like systems. Uses `libloading` to dynamically load `libnvidia-ml.so.1` at runtime. If the library is absent, returns `(0, 0)` gracefully. No compile-time dependency on NVIDIA drivers.
- The Windows cross-check (`--target x86_64-pc-windows-gnu`) exercises the `#[cfg(windows)]` code paths to ensure no platform-incompatible code slips through.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `windows` crate 0.57 may not expose `IDXGIFactory1::EnumAdapters1` or `DXGI_ADAPTER_DESC1` under the `Win32_Graphics_Dxgi` feature flag. The ACT agent must verify these type names exist in the resolved version via `rust-docs` MCP before writing code. | Medium | High | The ACT agent queries `rust-docs` MCP at session start. If types are missing, the plan is adjusted to use the correct type names or the version is bumped. |
| `libloading` may not be available or may have API changes between 0.7 and 0.8. The NVML symbol resolution code depends on `libloading::Library::new()` and `Library::get::<T>()`. | Low | Medium | The ACT agent verifies `libloading` API shape via MCP. If the API changed, the code is adjusted accordingly. The NVML path is a fallback — if it fails, the system still works with sysfs data. |
| The `windows` crate pulls in transitive dependencies that may not compile on non-Windows targets even when behind `#[cfg(windows)]`. Cargo resolves dependencies for all targets, not just the current one. | Medium | High | Gate the `windows` dependency behind an optional feature (`dxgi`) in `Cargo.toml`. The feature is only enabled on Windows targets. This prevents transitive dependency resolution failures on Linux/Mac. |
| `/sys/bus/pci/devices/` may contain entries that are not GPUs (e.g., network adapters, storage controllers). The vendor ID filter helps but doesn't eliminate all false positives. | Low | Low | The sysfs detector only reads vendor/device IDs and maps known GPU vendor IDs (0x10de, 0x1002). Unknown vendors are mapped to `DeviceType::Cpu` and flagged with `CapabilitiesSource::Fallback`. The device_db task (P4-A4) will refine this with a PCI-ID capability table. |
| NVML's `nvmlDeviceGetMemoryInfo` requires a valid `nvmlDevice_t` handle, which is opaque and obtained from `nvmlDeviceGetHandleByIndex`. Using a dummy pointer will cause the call to fail, returning an error that we catch and convert to `(0, 0)`. | High | Low | This is by design — NVML is a VRAM refresh supplement, not a primary enumerator. If NVML fails, we fall back to the sysfs-detected VRAM values (which are 0 at this stage). The actual VRAM refresh happens via the Python worker's PyTorch report. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (Linux mock-hardware path)
- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `head -1 crates/anvilml-hardware/Cargo.toml` shows version `0.1.3`
