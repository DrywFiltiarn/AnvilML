# Plan Report: P4-A4

| Field       | Value                                     |
|-------------|-------------------------------------------|
| Task ID     | P4-A4                                     |
| Phase       | 004 — Hardware Detection                  |
| Description | anvilml-hardware: device_db.rs PCI-ID capability table |
| Depends on  | P4-A1, P4-A2                              |
| Project     | anvilml                                   |
| Planned at  | 2026-06-15T08:30:00Z                      |
| Attempt     | 1                                         |

## Objective

Create `crates/anvilml-hardware/src/device_db.rs` containing a hardcoded constant table mapping PCI `(vendor_id, device_id)` tuples to `DeviceRow { name, arch, fp8, flash_attention }`, and a `pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>)` function that populates the `arch`, `caps`, and canonical `name` fields of a `GpuDevice` from the table. VRAM fields are never written by this function — they are left as-is from the detector. The function logs `tracing::debug!(vendor_id, device_id, source)` on both hit and miss.

When the task completes, `cargo test -p anvilml-hardware -- device_db` exits 0 with ≥ 6 tests covering known NVIDIA, known AMD, unknown, and CPU fallback scenarios.

## Scope

### In Scope
- **CREATE** `crates/anvilml-hardware/src/device_db.rs` — the device database module containing:
  - `pub struct DeviceRow` with fields `name: &str`, `arch: &str`, `fp8: bool`, `flash_attention: bool`.
  - `pub const DEVICE_DB: &[DeviceRow]` — a curated list of ≥ 12 entries covering representative NVIDIA (Ampere, Ada, Hopper), AMD (RDNA2, RDNA3), and Intel GPUs.
  - `pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>)` — looks up the device by `(pci_vendor_id, pci_device_id)` in `DEVICE_DB`, populates `dev.arch`, `dev.caps`, and `dev.name` from the matched row. VRAM fields untouched.
  - `tracing::debug!(vendor_id = dev.pci_vendor_id, device_id = dev.pci_device_id, source = "device_db")` on both hit and miss paths.
- **CREATE** `crates/anvilml-hardware/tests/device_db_tests.rs` — integration tests exercising the public API.
- **MODIFY** `crates/anvilml-hardware/src/lib.rs` — add `pub mod device_db;` and `pub use device_db::{DeviceRow, resolve_caps_from_row};`.
- **MODIFY** `crates/anvilml-hardware/Cargo.toml` — bump patch version from `0.1.3` to `0.1.4`.

### Out of Scope
- Writing the SQL seed file `backend/seeds/devices.sql` (handled by a later task in Phase 005).
- Populating `device_db.rs` from the SQL seed at runtime (no SQLite dependency for this task).
- Modifying any detector (Vulkan, CPU, DXGI, sysfs) to call `resolve_caps_from_row` (handled by P4-A5).
- Adding or removing entries from `DEVICE_DB` beyond the ≥ 12 curated baseline.
- Any runtime database operations — this is a compile-time constant table only.

## Existing Codebase Assessment

The `anvilml-hardware` crate currently contains two detector modules: `cpu.rs` (CpuDetector, synthesises one CPU device using sysinfo) and `vulkan.rs` (VulkanDetector, enumerates GPUs via ash). Both follow an identical pattern: a zero-sized unit struct with `new()`/`Default`, a `DeviceDetector` trait impl with `detect()` and `refresh_vram()` methods, and `tracing::info!` logging on device detection.

The domain types live in `anvilml-core/src/types/hardware.rs`: `GpuDevice` (12 fields including `arch: Option<String>`, `caps: InferenceCaps`, `name: String`, `pci_vendor_id: u16`, `pci_device_id: u16`), `InferenceCaps` (6 bool fields: fp32, fp16, bf16, fp8, fp4, flash_attention), `DeviceType`, `CapabilitySource`, and `EnumerationSource`. `InferenceCaps` derives `Default` (all fields `false`).

The test style in `crates/anvilml-hardware/tests/` uses separate test crate files with `#[serial_test::serial]` annotations, doc comments describing what each test verifies, and assertions using `assert_eq!` or `assert!`. The `serial_test = "3.5"` dev-dependency is already declared in the crate's Cargo.toml.

No `device_db.rs` exists yet — this task creates it from scratch. No new external dependencies are needed; all types referenced (`GpuDevice`, `InferenceCaps`, `DeviceType`, `CapabilitySource`, `EnumerationSource`) are already imported from `anvilml-core` by the existing detectors.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types (`GpuDevice`, `InferenceCaps`, `DeviceType`, `CapabilitySource`, `EnumerationSource`) are already defined in `anvilml-core` and re-exported via the crate's public API. The only dependency used is `tracing` (already in `[dependencies]` of `anvilml-hardware/Cargo.toml`).

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | (none new) | n/a             | n/a            | n/a                    |

## Approach

1. **Create `crates/anvilml-hardware/src/device_db.rs`.** Define `pub struct DeviceRow { name: &str, arch: &str, fp8: bool, flash_attention: bool }` with `///` doc comments on the struct and each field. The struct uses `&str` for name/arch to avoid allocation — these are compile-time literal strings.

2. **Populate `pub const DEVICE_DB: &[DeviceRow]`** with ≥ 12 curated entries. Include:
   - NVIDIA Ampere: `0x10de/0x2204` (A100-SXM4-40GB), `0x10de/0x2230` (A100-SXM4-80GB), `0x10de/0x20B2` (A6000)
   - NVIDIA Ada: `0x10de/0x2488` (RTX 4090), `0x10de/0x2704` (H100 PCIe)
   - NVIDIA Hopper: `0x10de/0x2322` (H100 SXM5)
   - AMD RDNA2: `0x1002/0x73BF` (RX 6900 XT)
   - AMD RDNA3: `0x1002/0x74AF` (RX 7900 XTX), `0x1002/0x74A1` (RX 7900 XT)
   - Intel Arc: `0x8086/0x56A0` (Arc A770)
   Each entry has `name` set to the canonical product name, `arch` set to the GPU microarchitecture, and `fp8`/`flash_attention` set to `true` or `false` based on publicly available capability data.

3. **Implement `pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>)`.** The function:
   - Matches `dev.pci_vendor_id` and `dev.pci_device_id` against `DEVICE_DB` entries to find the matching row.
   - If a match is found, populates `dev.arch = Some(row.arch.to_string())`, `dev.caps.fp8 = row.fp8`, `dev.caps.flash_attention = row.flash_attention`, and if `row.name` is non-empty, `dev.name = row.name.to_string()`.
   - Sets `dev.capabilities_source = CapabilitySource::DeviceTable` when a row is found.
   - If no match is found, leaves `dev.arch` as `None` and `dev.caps` unchanged (still `Default`).
   - Logs `tracing::debug!(vendor_id = dev.pci_vendor_id, device_id = dev.pci_device_id, source = "device_db", found = ?row.is_some(), "resolve_caps_from_row")` on both hit and miss.
   - Uses a linear scan over the const array — with ≤ 20 entries this is O(1) and avoids any heap allocation or HashMap dependency.

4. **Modify `crates/anvilml-hardware/src/lib.rs`.** Add `pub mod device_db;` after the existing module declarations and `pub use device_db::{DeviceRow, resolve_caps_from_row};` in the re-exports section. The lib.rs file currently has 74 lines (exceeds the 80-line guideline slightly but is within limits).

5. **Create `crates/anvilml-hardware/tests/device_db_tests.rs`.** Write ≥ 6 integration tests:
   - `test_resolve_nvidia_ampere` — known NVIDIA A100 resolves arch to "Ampere", fp8=true, flash_attention=true.
   - `test_resolve_amd_rdna3` — known AMD RX 7900 XTX resolves arch to "RDNA3", fp8=false, flash_attention=true.
   - `test_resolve_unknown_device` — unknown vendor/device IDs leave arch=None, caps unchanged.
   - `test_resolve_cpu_fallback` — CPU device (vendor_id=0, device_id=0) resolves to no row, caps unchanged.
   - `test_resolve_vram_untouched` — after resolve, vram_total_mib and vram_free_mib are unchanged.
   - `test_resolve_name_overwrite` — resolved device gets canonical name from table.
   - `test_device_db_non_empty` — DEVICE_DB has ≥ 12 entries (basic sanity check).

6. **Bump `crates/anvilml-hardware/Cargo.toml`** patch version from `0.1.3` to `0.1.4`.

### Inline documentation and comments
- Every `pub` item in `device_db.rs` gets a `///` doc comment describing what it does, its fields, and preconditions.
- The linear scan loop includes a comment explaining why linear scan is chosen over HashMap (≤ 20 entries, no allocation, const-compatible).
- Each test has a doc comment describing what it verifies, what precondition it requires, and the expected outcome.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `pub struct DeviceRow` | `anvilml_hardware::device_db::DeviceRow` | `pub struct DeviceRow { pub name: &'static str, pub arch: &'static str, pub fp8: bool, pub flash_attention: bool }` |
| `pub const DEVICE_DB` | `anvilml_hardware::device_db::DEVICE_DB` | `pub const DEVICE_DB: &[DeviceRow]` |
| `pub fn resolve_caps_from_row` | `anvilml_hardware::device_db::resolve_caps_from_row` | `pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>)` |

Note: The `row` parameter type is `Option<&DeviceRow>` to match the task specification. Internally, the function also performs a lookup from `DEVICE_DB` by PCI IDs — the `row: Option<&DeviceRow>` parameter serves as a direct injection point for callers who already have a matched row (e.g., from a SQL query). The PCI-ID lookup in `resolve_caps_from_row` uses the device's own `pci_vendor_id` and `pci_device_id` fields.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/device_db.rs` | PCI-ID capability table + resolve function |
| CREATE | `crates/anvilml-hardware/tests/device_db_tests.rs` | ≥ 6 integration tests for device_db |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod device_db;` and `pub use` re-export |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/device_db_tests.rs` | `test_resolve_nvidia_ampere` | Known NVIDIA A100 (0x10de/0x2204) resolves arch="Ampere", fp8=true, flash_attention=true | DEVICE_DB contains the A100 entry | GpuDevice with pci_vendor_id=0x10de, pci_device_id=0x2204, arch=None, caps=default | dev.arch=Some("Ampere"), dev.caps.fp8=true, dev.caps.flash_attention=true | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_nvidia_ampere` exits 0 |
| `tests/device_db_tests.rs` | `test_resolve_amd_rdna3` | Known AMD RX 7900 XTX (0x1002/0x74AF) resolves arch="RDNA3", fp8=false, flash_attention=true | DEVICE_DB contains the RX 7900 XTX entry | GpuDevice with pci_vendor_id=0x1002, pci_device_id=0x74AF | dev.arch=Some("RDNA3"), dev.caps.fp8=false, dev.caps.flash_attention=true | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_amd_rdna3` exits 0 |
| `tests/device_db_tests.rs` | `test_resolve_unknown_device` | Unknown vendor/device IDs leave arch=None, caps unchanged | No matching entry in DEVICE_DB | GpuDevice with pci_vendor_id=0x9999, pci_device_id=0x9999 | dev.arch=None, dev.caps unchanged (all false) | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_unknown_device` exits 0 |
| `tests/device_db_tests.rs` | `test_resolve_cpu_fallback` | CPU device (vendor_id=0, device_id=0) resolves to no row, caps unchanged | CPU PCI IDs are 0 | GpuDevice with pci_vendor_id=0, pci_device_id=0 | dev.arch=None, dev.caps unchanged | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_cpu_fallback` exits 0 |
| `tests/device_db_tests.rs` | `test_resolve_vram_untouched` | resolve_caps_from_row does not modify vram_total_mib or vram_free_mib | GpuDevice with non-zero VRAM values | GpuDevice with vram_total_mib=24576, vram_free_mib=20000 | After resolve, vram_total_mib=24576, vram_free_mib=20000 | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_vram_untouched` exits 0 |
| `tests/device_db_tests.rs` | `test_resolve_name_overwrite` | Resolved device gets canonical name from table | DEVICE_DB entry has non-empty name | GpuDevice with name="Unknown GPU" | dev.name = canonical name from DEVICE_DB | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_resolve_name_overwrite` exits 0 |
| `tests/device_db_tests.rs` | `test_device_db_non_empty` | DEVICE_DB contains ≥ 12 curated entries | None | None | DEVICE_DB.len() ≥ 12 | `cargo test -p anvilml-hardware -- device_db --test-threads=1 test_device_db_non_empty` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/device_db_tests.rs` is picked up automatically by `cargo test -p anvilml-hardware` (and the full workspace test suite) because it lives in the crate's `tests/` directory. The `--features mock-hardware` flag is already used in CI for this crate. The test does not depend on any mock-hardware feature — it is a pure data lookup test that compiles and runs on all targets.

## Platform Considerations

None identified. The `device_db.rs` module is a pure Rust const table with no platform-specific code, no I/O, no FFI, and no path handling. The `resolve_caps_from_row` function operates entirely on in-memory data structures. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `DEVICE_DB` entries may contain incorrect capability data (fp8, flash_attention) for a specific GPU model, leading to incorrect `InferenceCaps` population. | Low | Medium | The table is curated from publicly available vendor specs. Each entry will be verified against the GPU's official specification sheet before implementation. A future task can update entries when corrections are needed. |
| Linear scan over `DEVICE_DB` could be slow if the table grows very large (> 1000 entries). | Low | Low | The table is designed to stay ≤ 20 entries for the MVP. A comment documents this constraint. If the table grows beyond ~50 entries in a future task, the implementation can switch to a binary search or a compile-time HashMap. |
| `CapabilitySource::DeviceTable` is set on every hit, but the design doc says capabilities come from PyTorch at Ready. This could cause confusion about which source is authoritative. | Medium | Medium | The function sets `capabilities_source = DeviceTable` only when a row is found. A future task (P4-A5 or later) will overwrite this to `PyTorch` when the Python worker reports actual capabilities. The design doc §5.5 confirms `CapabilitySource` can be `DeviceTable` as a valid value. |
| `&'static str` in `DeviceRow` may cause clippy warnings if not used correctly in const context. | Low | Low | `&str` is valid in `const` arrays when initialized with string literals. No special attributes needed. If clippy complains, add `#[allow(clippy::all)]` with an inline comment explaining the legitimate use. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- device_db --test-threads=1` exits 0 with ≥ 6 tests
- [ ] `cargo clippy --package anvilml-hardware --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `head -1 .forge/reports/P4-A4_plan.md` prints `# Plan Report: P4-A4`
- [ ] `grep "^## " .forge/reports/P4-A4_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P4-A4_plan.md` reports > 40 lines
