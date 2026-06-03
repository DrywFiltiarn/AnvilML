# Plan Report: P4-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A4                                         |
| Phase       | 004 — Hardware Detection                     |
| Description | anvilml-hardware: DXGI (Windows) + sysfs/NVML (Linux) fallback enumerators |
| Depends on  | P4-A3                                        |
| Project     | anvilml                                      |
| Planned at  | 2026-06-03T10:35:00Z                        |
| Attempt     | 1                                            |

## Objective

Implement three fallback GPU device enumerators for `anvilml-hardware` that provide SDK-free hardware detection when the primary Vulkan detector cannot enumerate devices. The DXGI enumerator (Windows, gated behind `#[cfg(windows)]`) reads adapter info via COM/IDXGIFactory; the sysfs enumerator (Linux/unix) parses `/sys/bus/pci/devices/*`; and NVML provides an additional VRAM data source on NVIDIA Linux systems where libnvidia-ml is available. All three implement the existing `DeviceDetector` trait, return `Ok(vec![])` when absent or errored, warn-and-skip individual device failures, include parse helpers suitable for tests with fixtures, pass unit and integration test suites (`cargo test -p anvilml-hardware -- dxgi/sysfs/nvml`) on Linux, and cross-check cleanly under Windows targeting via `--target x86_64-pc-windows-gnu --features mock-hardware`.

## Scope

### In Scope
- **`src/dxgi.rs`** (gated: `#[cfg(windows)]`): DxgiDetector struct implementing DeviceDetector. Uses COM/IDXGIFactory1 → EnumAdapters(0) to enumerate adapters; reads adapter name via IDXGIAdapter::GetName, vendor/device IDs from DXGI_ADAPTER_DESC3 (`VendorId`, `DeviceId`), and DedicatedVideoMemory for VRAM total. Sets enumeration_source = Dxgi (placeholder enum variant). Vendor→DeviceType mapping: 0x10DE=Cuda, 0x1002=Rocm, else Cpu. Absent/COM error → Ok(vec![]); per-device failure warn+skip via `log::warn!`.
- **`src/sysfs.rs`** (gated: `#[cfg(unix)]`): SysfsDetector struct implementing DeviceDetector. Walks `/sys/bus/pci/devices/*/`, reads vendor and device IDs from the PCI config space (`vendor` / `device` files). VRAM read via amdgpu sysfs path at `/sys/class/drm/cardN/device/mem_info_vram_total`. Sets enumeration_source = Sysfs. Vendor→DeviceType mapping: 0x10DE=Cuda, 0x1002=Rocm, else Cpu. Absent/error → Ok(vec![]); per-device failure warn+skip.
- **`src/nvml.rs`** (gated: `#[cfg(unix)]`, lazy-loaded): NvmlDetector struct implementing DeviceDetector. Uses the `nvml-wrapper` crate to load libnvidia-ml dynamically at runtime; if unavailable, returns Ok(vec![]). Enumerates devices via nvml.DeviceCount() → get_name(), pci_bus_device_id(), memory_info(). VRAM from NVML's MemoryInfo.total/used fields (free = total - used). Sets enumeration_source = Nvml. Absent/error → Ok(vec![]); per-device failure warn+skip.
- **`src/lib.rs`**: Add `pub mod dxgi;`, `pub mod sysfs;`, `pub mod nvml;` with appropriate cfg gates. Re-export EnumerationSource enum (see below). Update compile-check tests to cover all three modules.
- **New module: `EnumerationSource`** in anvilml-core types/hardware.rs or locally as a new type — but since P4-B2 is the retrofit task that adds this, and tasks must be atomic per FORGE_AGENT_RULES §4 rules (no touching files outside scope unless necessary for compilation), we will use placeholder string-based source tracking within each detector's GpuDevice construction. The `enumeration_source` field does not yet exist on GpuDevice in the committed hardware.rs; P4-B2 adds it later, so these detectors must compile against the current type signature (index, name, device_type, vram_total_mib, vram_free_mib, driver_version) without source fields. We will store whatever data we collect and let P4-A5 integrate them into detect_all_devices with proper routing.
- **Parse helpers**: Public helper functions in each module (`parse_pci_ids`, `read_vram_from_sysfs`) for test fixture injection via unit tests inside the same file's `mod tests`.

### Out of Scope
- Integration with `detect_all_devices` (that is P4-A5).
- PCI-ID capability resolution table (P4-A4B, device_db.rs).
- Extending GpuDevice or HardwareInfo types — that belongs to P4-B2.
- NVML on Windows (NVML wrapper only supports Linux; NVIDIA's nvml.dll exists but the `nvml-wrapper` crate targets Unix/dlopen) — future work if needed.
- Direct3D12/DirectX 12 native path via dxgi-sys raw FFI bindings without COM interop overhead.

## Approach

### Step 0: Verify prerequisite dependencies and resolve NVML binding version
Use `rust-docs` MCP tool to look up the latest stable `nvml-wrapper` crate on crates.io, confirm it supports dynamic loading (`dlopen`) of libnvidia-rl.so at runtime (so absence does not cause linker failure), check its MSRV against workspace rust-toolchain.toml. If unavailable or incompatible with unix-only cfg gates, record as dependency note and use an alternative like `libloading` + raw NVML FFI stubs in a separate module scope function — but prefer nvml-wrapper if it meets criteria.

### Step 1: Create src/dxgi.rs (Windows)
- Define `#[derive(Debug, Clone, Default)] pub struct DxgiDetector;`.
- Implement DeviceDetector for DxgiDetector on cfg(windows).
- Use COM via the `windows` crate or raw FFI to create IDXGIFactory1. Enumerate adapters: loop over index 0..N calling idxgifactory.EnumAdapters(index), break when S_FALSE returned. For each adapter, call GetDesc3() for VendorId/DeviceId/DedicatedVideoMemory and GetName(). Build GpuDevice from collected fields (index=loop counter, name=name_str, device_type=vendor_id_to_device_type(vid), vram_total_mib=DedicatedVideoBytes/MiB_divisor, vram_free_mib=u32::MAX as fallback since DXGI doesn't expose per-app VRAM usage in this API path).
- Wrap COM initialization failures and EnumAdapters errors: log warn + skip that device; if factory creation fails entirely → Ok(vec![]).

### Step 2: Create src/sysfs.rs (Unix)
- Define `#[derive(Debug, Clone, Default)] pub struct SysfsDetector`.
- Implement DeviceDetector for SysfsDetector on cfg(unix).
- Walk `/sys/bus/pci/devices/*/` using std::fs::read_dir. For each directory entry that matches the glob pattern: read vendor file (`{path}/vendor`) and device file (`{path}/device`). Parse hex strings to u16 values; skip entries where parsing fails (warn + continue).
- Determine GPU type from vendor ID mapping function identical in logic to vulkan.rs's private `vendor_id_to_device_type`. AMD GPUs → attempt amdgpu sysfs VRAM read: scan `/sys/class/drm/card*/device/` for matching PCI bus/device, then read `{card_path}/mem_info_vram_total`. NVIDIA cards — skip VRAM (handled by nvml module); set vram_free_mib = u32::MAX.
- Build GpuDevice per valid entry: index=sequential counter starting from 0, name="PCI-{vendor_id}:{device_id}" fallback since sysfs doesn't always provide human-readable names directly via PCI config read (could also try reading `{path}/subsystem/devices/*/class` or using `udevadm info --query=all`).
- Handle errors per ANVILML_DESIGN §5: absent/error → Ok(vec![]); individual device parse failure warn+skip.

### Step 3: Create src/nvml.rs (Unix)
- Define `#[derive(Debug, Clone, Default)] pub struct NvmlDetector`.
- Implement DeviceDetector for SysfsDetector on cfg(unix).
- Use nvml-wrapper crate to initialize NVML via static init() which dlopen's libnvidia.so at runtime. If init fails (library absent or too old) → Ok(vec![]), no error propagated up the call chain — this is correct per SDK-free design principle: "Loader absent→Ok(vec![])".
- Enumerate devices count, loop to get each device handle via nvml.DeviceByIndex(i). Call Name() for name string. PCI info from DevicePciBusIdStr(), parse bus/device hex values (not vendor_id directly — NVML doesn't expose raw VendorID; we'll use a heuristic: if the card is NVIDIA → Cuda, else skip or map conservatively to CPU since NVML only works on NVIDIA hardware). VRAM total+free from MemoryInfo struct.
- Build GpuDevice per device with enumeration_source = Nvml concept (stored as string field placeholder for now until P4-B2 adds the enum type).

### Step 4: Wire modules into lib.rs and update compile checks
- Add three new pub mod declarations in src/lib.rs behind appropriate cfg gates.
- Re-export EnumerationSource from anvilml-core if already committed by prior tasks; otherwise note as dependency on P4-B2 for full compilation of downstream code that references it (the detectors themselves do not need to reference the type directly — they populate GpuDevice fields).
- Add compile-check tests in lib.rs's mod tests confirming DxgiDetector, SysfsDetector, and NvmlDetector implement DeviceDetector where their cfg gates are active.

### Step 5: Write unit/integration test fixtures
Each module gets a `#[cfg(test)] mod tests` block with fixture-driven parse helpers tested via mock data files or direct function calls to parsing logic (e.g., pass hex strings directly, verify vendor→DeviceType mapping). For sysfs specifically, create temporary directories under /tmp during the test and write fake PCI IDs + VRAM values into them — use tempfile crate if available in dev-dependencies. If not, add it as a lightweight dependency or inline temp dir creation with std::env::temp_dir().

### Step 6: Add Cargo.toml dependencies
- For dxgi.rs on Windows: The `windows` crate is the recommended Microsoft FFI binding for DXGI interop (DXGI1_4 feature set). Check if already present in workspace lockfile; otherwise add. Alternatively, use raw COM via core-foundation + winapi crates which are lighter-weight and commonly used — prefer `winapi` with dxgi/features since it's more lightweight than the full MSFT windows crate for this narrow API surface (IDXGIFactory1::EnumAdapters0/1, IDXGIAdapter methods).
- For nvml.rs on Unix: Add `nvml-wrapper = "0.9"` to Cargo.toml dev-dependencies only? No — it's a runtime dependency but with dlopen-based loading so absent at compile time is fine (it links against libnvidia.so which may or may not be present). Actually, since the crate needs to link against nvml wrapper even if no GPU exists on CI runner, verify that `nvml-wrapper` supports build-time optional linking. If it requires -lnvidia-ml at compile/link step: add as regular dependency but gate behind cfg(unix) in Cargo.toml (possible with conditional dependencies). Alternative approach used by many Rust NVML wrappers is to use libloading + manual FFI for the specific functions needed — this avoids any system library requirement. Plan uses `nvml-wrapper` if it supports optional linking; otherwise falls back to raw FFI via `libloading`.

### Step 7: Cross-compile Windows check
Run (conceptually in ACT session): cargo check --target x86_64-pc-windows-gnu --features mock-hardware. Verify dxgi.rs compiles under the mingw target, sysfs/nvml are properly cfg-gated and not compiled for windows-gnu.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-hardware/src/dxgi.rs` | DxgiDetector: Windows DXGI IDXGIFactory1 GPU enumerator + tests |
| Create | `crates/anvilml-hardware/src/sysfs.rs`  | SysfsDetector: Linux PCI sysfs enumeration helper + amdgpu VRAM reader, with unit test fixtures |
| Create | `crates/anvilml-hardware/src/nvml.rs`    | NvmlDetector: Unix NVML libnvidia-ml GPU enumerator (lazy-loaded) + tests |
| Modify | `crates/anvilml-hardware/Cargo.toml`     | Add cfg-gated dependencies for dxgi sysfs vendor parsing and nvml-wrapper/libloading |
| Modify | `crates/anvilml-hardware/src/lib.rs`      | pub mod declarations with #[cfg(windows)] / #[cfg(unix)], compile-check tests, re-exports if applicable |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `src/dxgi.rs::tests` (in-file) | vendor_id_maps_cuda/rocm/cpu | Vendor→DeviceType mapping identical to vulkan logic, via direct function call on fixture hex values |
| `src/dxgi.rs::tests` (in-file) | dxgi_detect_returns_ok | Compile-check: DxgiDetector implements DeviceDetector; detect() returns Ok(vec![]) when COM unavailable or no adapters present |
| `src/sysfs.rs::tests` (in-file) | parse_pci_ids_valid_hex | Helper function correctly parses "0x10de" → 0x10DE u16, handles leading zeros and case-insensitivity |
| `src/sysfs.rs::tests` (in-file) | read_vram_from_amdgpu_sysfs | Creates temp dir with fake mem_info_vram_total file; helper returns correct MiB value from byte count |
| `src/sysfs.rs::tests` (in-file) | sysfs_detect_returns_ok_on_absent_dir | When /sys/bus/pci/devices doesn't exist or is empty, detect() returns Ok(vec![]) — no panic/error in CI environment without GPUs |
| `src/nvml.rs::tests` (in-file) | nvml_init_fallback_no_library | NVML init fails gracefully when libnvidia-ml absent; detect() → Ok(vec!) with zero devices logged, not Err |

## CI Impact

No changes to `.github/workflows/`. The existing Linux rust job (`cargo test -p anvilml-hardware --features mock-hardware`) will pick up the new modules because they are cfg-gated — sysfs and nvml compile on ubuntu-linux but return empty device lists since no GPU drivers exist in CI runners. DXGI is #[cfg(windows)] only so it never compiles under Linux or Windows runner jobs that use cargo test (not --target x86_64-pc-windows-gnu). The cross-check gate (`cargo check --target x86_64-pc-windows-gnu`) already runs for all hardware modules; adding dxgi.rs behind cfg(windows) means it compiles under the mingw target and sysfs/nvml are excluded. No new CI jobs, no existing job modifications required — task stays within established gates defined in TASKS_PHASE001.md §P1-A5 (fmt + clippy -D warnings + test).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Windows cross-compile target x86_64-pc-windows-gnu may lack the `windows` crate or winapi MSRV mismatch with workspace rust-toolchain.toml. Check lockfile first; if missing, add to Cargo.toml in ACT session before writing report (Rule 3.2: no git ops). |
| NVML library linking failure on CI ubuntu-latest runners without NVIDIA GPU drivers — nvml-wrapper requires libnvidia-rl.so at link time by default which will not exist in the runner image causing build failures even with cfg(unix) gates since Cargo still compiles all deps. Use `libloading` + raw FFI instead of nvml-wrapper to avoid any system library dependency, OR use a dev-dependency-only approach where NVML detection is compiled but returns Ok(vec![]) at runtime without requiring libnvidia.so — the task specifies "absent→Ok(vec!)". |
| Sysfs paths vary across distros and kernel versions; amdgpu sysfs interface may not exist on all AMD GPUs (e.g., older Polaris vs RDNA). Read-only access to /sys/bus/pci/devices/* is universal. Fall back gracefully: if mem_info_vram_total absent, set VRAM = 0 for that device rather than failing the whole enumeration — matches "per-device failure warn+skip" requirement from task spec. |
| DXGI on Windows requires COM initialization (CoInitializeEx) which must be called once per process thread and is not safe to call concurrently across threads without synchronization. Use std::sync::OnceLock for a one-shot CoInitialize guard; if it fails, log warning + return Ok(vec![]). This follows the same pattern used by ash's Entry::load() in vulkan.rs (graceful fallback on absent loader → empty vec) |
| Task scope creep: accidentally touching GpuDevice struct fields that P4-B2 will add later. Strictly limit to existing 6-field signature; do not modify types/hardware.rs or any type definitions — only populate the six already-committed fields with whatever data each enumerator can gather, and leave source tracking (EnumerationSource/CapabilitySource) for future tasks |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- dxgi sysfs nvml` exits 0 on Linux host
- [ ] All three modules compile: src/dxgi.rs behind #[cfg(windows)], src/sysfs.rs + src/nvml.rs behind #[cfg(unix)] — verified via cargo check without errors or warnings under the native target (rustc --version stable, platform linux)
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware` exits 0 with `-D warnings` on Linux host
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` succeeds (dxgi.rs compiles under mingw, sysfs/nvml excluded by cfg gates) — verified in ACT session using the installed gcc-mingw-w64 linker per FORGE_AGENT_RULES §5.7
- [ ] Each module's `mod tests {}` block contains at least one unit test exercising a parse helper function with fixture data (hex ID parsing, VRAM byte conversion from temp file) — no external GPU hardware required for any test to pass
