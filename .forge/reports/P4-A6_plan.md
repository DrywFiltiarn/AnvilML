# Plan Report: P4-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A6                                       |
| Phase       | 004 — Hardware Detection: Detectors         |
| Description | anvilml-hardware: SysfsPciDetector Linux fallback (cfg-gated) |
| Depends on  | P4-A4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T06:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `SysfsPciDetector` struct in `crates/anvilml-hardware/src/sysfs.rs`, implementing the `DeviceDetector` trait to enumerate Linux PCI display controllers from `/sys/bus/pci/devices/`. This is the Linux fallback detector that Phase 5's `detect_all_devices()` will call when Vulkan enumeration returns empty. The detector reads vendor/device/class PCI config space files, filters for display controllers (class prefix 0x03), maps vendor IDs to `DeviceType` via the shared `vendor_id_to_device_type()` function, and constructs `GpuDevice` structs with `enumeration_source: EnumerationSource::Sysfs`. It never panics — missing paths or permission errors return `Ok(vec![])`.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/sysfs.rs` with `SysfsPciDetector` struct and `DeviceDetector` impl.
- Update `crates/anvilml-hardware/src/lib.rs` to add `#[cfg(target_os = "linux")] pub mod sysfs;` and `#[cfg(target_os = "linux")] pub use sysfs::SysfsPciDetector;`.
- Implement `detect()` that reads `/sys/bus/pci/devices/*/{vendor,device,class}` files, filters by class prefix `0x03`, maps vendor IDs to `DeviceType`, and constructs `GpuDevice` structs.
- Implement `refresh_vram()` returning `Ok((0, 0))` — sysfs PCI config space does not expose VRAM info.
- Create `crates/anvilml-hardware/tests/sysfs_tests.rs` with ≥ 3 tests.
- No new external dependencies — uses only `std::fs`, `std::path`, and existing `anvilml-core` types.

### Out of Scope
None. This task's `defers_to (from JSON): []` is empty — no scope is deferred.

## Existing Codebase Assessment

The `anvilml-hardware` crate already exists as a buildable stub with `mock-hardware` feature declared. Three concrete detectors are implemented: `CpuDetector` (pure value construction, no I/O), `VulkanDetector` (headless Vulkan enumeration via `ash`), and `DxgiDetector` (Windows DXGI via `windows` crate). The `DeviceDetector` trait is defined in `detect.rs` with two methods: `detect()` and `refresh_vram()`.

The shared `vendor_id_to_device_type()` function is already exported from `vulkan.rs` and re-exported from `lib.rs` — this is the vendor ID → `DeviceType` mapping (`0x10de → Cuda`, `0x1002 → Rocm`) that all three real/fallback detectors must use consistently.

The `GpuDevice` struct in `anvilml-core` has all necessary fields including `enumeration_source: EnumerationSource::Sysfs` (the variant already exists in the enum). The existing test files (`vulkan_tests.rs`, `mock_tests.rs`, `cpu_tests.rs`, `dxgi_tests.rs`) follow a consistent pattern: doc-comment headers, `anvilml_core::types::*` import, trait import, per-test doc comments, and assertions on field values.

No gap exists between the design doc and current source — the `EnumerationSource::Sysfs` variant is already defined in `hardware.rs`, and the `lib.rs` structure (with cfg-gated module declarations for dxgi) provides the exact pattern to follow for the sysfs module.

## Resolved Dependencies

None. This task introduces no new external crates. It uses only `std::fs::read_to_string`, `std::fs::read_dir`, and `std::path::PathBuf` from the Rust standard library, plus existing `anvilml-core` types.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| std | std::fs / std::path | (Rust stdlib) | n/a | n/a |

## Approach

### Step 1: Create `crates/anvilml-hardware/src/sysfs.rs`

Create a new file with the following structure:

**Module-level doc comment** describing `SysfsPciDetector` as the Linux fallback detector per §6.4 step 5.

**Struct definition:**
```rust
pub struct SysfsPciDetector;
```

**Imports:**
```rust
use crate::detect::DeviceDetector;
use crate::vendor_id_to_device_type;
use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps,
};
use std::fs;
use std::path::Path;
```

**`detect()` implementation:**
1. Open `/sys/bus/pci/devices/` directory. If the path does not exist or is not a directory (IO error), return `Ok(vec![])` — never `Err`, never panic. Log at DEBUG level.
2. Iterate over entries in the directory. For each entry that is a directory (device name like `0000:00:1f.0`):
   a. Construct paths to `{vendor,device,class}` files within the device directory.
   b. Read `class` file first — this is the filter. Parse as hex. If the class file is missing or unreadable (permission error, IO error), skip that device — log at DEBUG level.
   c. Check if class starts with `0x03` (display controller per PCI class code spec). If not, skip the device.
   d. Read `vendor` file, parse as hex to u16. If missing/unreadable, skip.
   e. Read `device` file, parse as hex to u16. If missing/unreadable, skip.
   f. Map vendor ID via `vendor_id_to_device_type()`. If `None`, skip (unknown vendor).
   g. Construct `GpuDevice` with: `index` = counter, `name` = device name from directory entry (or "PCI Device"), `device_type` = mapped type, `vram_total_mib: 0`, `vram_free_mib: 0`, `driver_version: "n/a"` (sysfs doesn't expose driver version), `pci_vendor_id`, `pci_device_id`, `arch: None`, `caps: InferenceCaps::default()`, `enumeration_source: EnumerationSource::Sysfs`, `capabilities_source: CapabilitySource::Fallback`.
   h. Push to devices vector.
3. Return `Ok(devices)`.

**`refresh_vram()` implementation:**
Return `Ok((0, 0))` — sysfs PCI config space does not expose VRAM information. This is consistent with `DxgiDetector`'s approach.

**Key design decisions and rationale:**
- The vendor ID mapping uses the shared `vendor_id_to_device_type()` from `vulkan.rs` — this ensures identical mapping across all three real/fallback detectors (Vulkan, DXGI, sysfs), preventing inconsistent results in Phase 5's priority chain.
- Class prefix filtering uses `0x03` (display controller) — per PCI class code specification, class code `0x03xxxx` indicates a display controller. We check the high byte only (class prefix) since the low bytes are subclass/programming interface which vary by vendor.
- VRAM returns `(0, 0)` — unlike Vulkan which can query memory heaps, sysfs PCI config space has no VRAM query API. The `(0, 0)` sentinel signals "unknown" to the caller.
- Missing/unreadable files are silently skipped (returning `Ok(vec![])` for the whole detector if none succeed) — per the "detection never panics" rule (§6.2).

### Step 2: Update `crates/anvilml-hardware/src/lib.rs`

Add two cfg-gated lines after the existing dxgi block:
```rust
#[cfg(target_os = "linux")]
pub mod sysfs;
#[cfg(target_os = "linux")]
pub use sysfs::SysfsPciDetector;
```

The gate is at the `mod` statement level (not wrapping file contents), matching the established pattern for `dxgi` and `mock-hardware` modules.

### Step 3: Create `crates/anvilml-hardware/tests/sysfs_tests.rs`

Create the test file with `#[cfg(target_os = "linux")]` at the file level (matching `dxgi_tests.rs` pattern). Import:
```rust
#[cfg(target_os = "linux")]
use anvilml_core::types::*;
#[cfg(target_os = "linux")]
use anvilml_hardware::detect::DeviceDetector;
#[cfg(target_os = "linux")]
use anvilml_hardware::sysfs::SysfsPciDetector;
```

**Test 1: `test_sysfs_detect_missing_path_returns_empty`**
- Create a `SysfsPciDetector`.
- Since `/sys/bus/pci/devices/` may or may not exist in the test environment, assert `result.is_ok()` and that the result is a vector. On systems without `/sys`, returns `Ok(vec![])`. The key invariant: no panic, no `Err`.

**Test 2: `test_sysfs_detect_synthetic_display_device`**
- Create a temporary directory tree mimicking `/sys/bus/pci/devices/0000:01:00.0/` with:
  - `vendor` file containing `"0x10de"` (NVIDIA)
  - `device` file containing `"0x2204"` (RTX 3080)
  - `class` file containing `"0x030000"` (display controller, VGA compatible)
- Temporarily override the sysfs path by using a test helper function. Since `SysfsPciDetector` reads directly from `/sys/bus/pci/devices/`, we need to test via a temp-dir approach. The approach: create a synthetic sysfs tree in a temp dir, then use `std::env` to set a test-specific path... Actually, the simplest approach per the task spec is: create a temp dir with the synthetic tree, then use `std::env::set_var` to point a test constant... 

Wait — looking at the task spec more carefully: "a temp-dir-mocked sysfs tree with one synthetic display-class device parses correctly." The detector reads directly from `/sys/bus/pci/devices/`. To mock this without modifying the source, the test can:
1. Create a temp directory with the synthetic tree
2. Symlink or temporarily replace `/sys/bus/pci/devices/` — but that requires root.

The practical approach: the test creates a temp directory with a synthetic sysfs tree, then calls a **test helper** that accepts a base path. But the current `detect()` signature doesn't accept a path parameter.

The correct approach for this test is to create a temp directory with the synthetic tree, then directly call `SysfsPciDetector::detect()` which reads from `/sys/bus/pci/devices/`. On the test machine, if `/sys/bus/pci/devices/` exists and has real devices, this test would need to filter. 

Actually, re-reading the task spec: "a temp-dir-mocked sysfs tree with one synthetic display-class device parses correctly." The most pragmatic approach: create a temp dir with the synthetic tree, and write a **private test helper** in the test file that accepts a `base_path: &Path` parameter, mirrors the detection logic, and is tested with the temp dir. This is the standard pattern for filesystem-dependent code in this codebase.

Better approach — the simplest that satisfies the spec: create a temp directory with a synthetic sysfs tree, then write a `fn detect_from_path(base: &Path) -> Result<Vec<GpuDevice>, AnvilError>` private function inside `sysfs.rs` (or in the test file via a `#[cfg(test)]` module) that accepts a path parameter. The public `detect()` on `SysfsPciDetector` calls `detect_from_path("/sys/bus/pci/devices")`. This way the test can call `detect_from_path(temp_dir)` directly.

**Test 3: `test_sysfs_filter_non_display_class`**
- Create a temp directory with a synthetic sysfs tree where the `class` file contains `"0x020000"` (network controller, NOT display).
- Call `detect_from_path(temp_dir)` and assert the result is empty (device filtered out).

**Test 4: `test_sysfs_vendor_id_mapping`** (bonus test to ensure AMD mapping works)
- Create a temp directory with `vendor = "0x1002"` (AMD), `class = "0x030000"`.
- Assert the device's `device_type` is `DeviceType::Rocm`.

### Step 4: Update `docs/TESTS.md`

Add test catalogue entries for the new sysfs tests, following the existing format for hardware tests.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `struct SysfsPciDetector` | `anvilml_hardware::SysfsPciDetector` | Zero-sized struct, no fields. Implements `DeviceDetector`. |
| `fn detect(&self)` | `impl DeviceDetector for SysfsPciDetector` | Enumerate Linux PCI display controllers from `/sys/bus/pci/devices/`. |
| `fn refresh_vram(&self, index: u32)` | `impl DeviceDetector for SysfsPciDetector` | Returns `Ok((0, 0))` — sysfs has no VRAM query API. |

No new `pub` functions or types beyond the struct itself. The `vendor_id_to_device_type` function is already exported from `vulkan.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/sysfs.rs` | `SysfsPciDetector` struct and `DeviceDetector` impl |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add cfg-gated `mod sysfs` and `pub use sysfs::SysfsPciDetector` |
| CREATE | `crates/anvilml-hardware/tests/sysfs_tests.rs` | ≥ 3 integration tests for `SysfsPciDetector` |
| MODIFY | `docs/TESTS.md` | Add catalogue entries for new sysfs tests |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/sysfs_tests.rs` | `test_sysfs_detect_missing_path_returns_empty` | `detect()` returns `Ok(vec![])` when `/sys/bus/pci/devices/` is absent or unreadable | None (runs on any Linux) | Real filesystem | `Ok(vec![])` — no panic, no `Err` | `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_missing_path_returns_empty` exits 0 |
| `crates/anvilml-hardware/tests/sysfs_tests.rs` | `test_sysfs_detect_synthetic_display_device` | A temp-dir-mocked sysfs tree with one synthetic display-class device (vendor=0x1002, class=0x030000) parses correctly into a `GpuDevice` with `DeviceType::Rocm` | `tempfile` crate available as dev-dependency (or use `std::env::temp_dir()`) | Synthetic sysfs tree in temp dir | `Ok(vec![device])` where `device.enumeration_source == Sysfs`, `device.device_type == Rocm` | `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_synthetic_display_device` exits 0 |
| `crates/anvilml-hardware/tests/sysfs_tests.rs` | `test_sysfs_filter_non_display_class` | A device with class `0x020000` (network controller) is filtered out | None | Synthetic sysfs tree with non-display class | `Ok(vec![])` — device excluded by class filter | `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_filter_non_display_class` exits 0 |
| `crates/anvilml-hardware/tests/sysfs_tests.rs` | `test_sysfs_detect_nvidia_vendor` | A synthetic sysfs tree with vendor=0x10de (NVIDIA) and class=0x030000 maps to `DeviceType::Cuda` | None | Synthetic sysfs tree with NVIDIA vendor | `Ok(vec![device])` where `device.device_type == Cuda` | `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_nvidia_vendor` exits 0 |

Note: `std::env::temp_dir()` is used for the temp directory — no new dev-dependency needed. The `tempfile` crate is not declared in this crate's Cargo.toml; the tests use `std::fs::create_dir_all` + `std::fs::write` on a `temp_dir()` path.

## CI Impact

No CI changes required. The test file is gated by `#[cfg(target_os = "linux")]` at the file level, so it compiles to an empty test binary on the `windows-latest` CI runner (expected and correct). The `rust-linux` CI job will collect and run these tests. The `mock-hardware` feature flag is used for all CI builds, and this module is gated by `target_os`, not by feature — so it's included in both mock-hardware and real-hardware CI builds on Linux.

## Platform Considerations

This task is **Linux-only**. The module is gated by `#[cfg(target_os = "linux")]` at the `mod` statement in `lib.rs`. On Windows, the module is not compiled and `SysfsPciDetector` is not available.

No `#[cfg(windows)]` guards are needed — the entire file is excluded from non-Linux builds. The test file also uses `#[cfg(target_os = "linux")]` at the file level, matching the `dxgi_tests.rs` pattern.

No path-separator or line-ending handling is needed — `/sys/bus/pci/devices/` uses Unix-style forward slashes and is Linux-specific.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `/sys/bus/pci/devices/` may not be readable in the test environment (permission denied or not mounted) | Medium | Medium | The `detect()` function guards all IO errors — missing path returns `Ok(vec![])`, permission errors log at DEBUG and skip the device. Test 1 explicitly verifies this path. |
| Sysfs class code format may vary across kernel versions (e.g., 3-byte vs 4-byte hex) | Low | Medium | Parse as string, strip "0x" prefix if present, then check if the first two hex digits (high byte) equal `03`. Handle both `0x030000` and `030000` formats. |
| `std::fs::read_dir` may include non-directory entries (symlinks, files) in `/sys/bus/pci/devices/` | Low | Low | Check `entry.path().is_dir()` before processing each entry. Skip non-directories silently. |
| Vendor ID parsing may fail for malformed class files (not valid hex) | Low | Low | Use `u32::from_str_radix` with error handling — if parsing fails, skip the device. Log at DEBUG level. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-hardware` exits 0 (compilation succeeds)
- [ ] `cargo test -p anvilml-hardware --test sysfs_tests` exits 0 (all sysfs tests pass on Linux)
- [ ] `cargo clippy -p anvilml-hardware -- -D warnings` exits 0 (no clippy warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
- [ ] `grep -c "SysfsPciDetector" crates/anvilml-hardware/src/sysfs.rs` returns ≥ 1 (struct exists)
- [ ] `grep -c "EnumerationSource::Sysfs" crates/anvilml-hardware/src/sysfs.rs` returns ≥ 1 (correct enumeration source used)
- [ ] `grep "mod sysfs" crates/anvilml-hardware/src/lib.rs` returns 1 line (module declared in lib.rs)
