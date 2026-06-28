# Plan Report: P4-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2                                       |
| Phase       | 004 — Hardware Detection: Detectors         |
| Description | anvilml-hardware: CpuDetector always returns one CPU device |
| Depends on  | P4-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T22:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-hardware/src/cpu.rs` implementing `CpuDetector: DeviceDetector`, the unconditional final-fallback detector that always returns exactly one synthesized `GpuDevice` with `device_type: DeviceType::Cpu` and `enumeration_source: EnumerationSource::Cpu`. This guarantees `detect_all_devices()` (Phase 5) always has at least one device to report, fulfilling `ANVILML_DESIGN.md §6.2`'s "result is always `Ok(HardwareInfo)` with at least one device" invariant. Declare `mod cpu;` in `lib.rs` and add a `tests/cpu_tests.rs` integration test file with ≥4 tests.

## Scope

### In Scope
- `crates/anvilml-hardware/src/cpu.rs`: `CpuDetector` struct (zero fields, unit struct) and `impl DeviceDetector for CpuDetector` with:
  - `detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` returning `Ok(vec![GpuDevice { ... }])` with the exact field values specified in the task context.
  - `refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError>` returning `Ok((0, 0))`.
- `crates/anvilml-hardware/src/lib.rs`: add `pub mod cpu;` re-export.
- `crates/anvilml-hardware/tests/cpu_tests.rs`: ≥4 integration tests verifying detection behaviour.
- `crates/anvilml-hardware/Cargo.toml`: bump patch version `0.1.0 → 0.1.1`.
- `docs/TESTS.md`: add test catalogue entries for the new test file.

### Out of Scope
None. This task's `defers_to` is `[]` — no scope is deferred.

## Existing Codebase Assessment

The `anvilml-hardware` crate exists as a minimal scaffold from Phase 1 (P1-B2) and has been extended by Phase 4's first task (P4-A1) to declare the `DeviceDetector` trait in `detect.rs`. The trait has two methods: `detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` and `refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>`.

The domain types in `anvilml-core` are fully implemented: `GpuDevice` (12 fields including `enumeration_source` and `capabilities_source`), `DeviceType` (Cuda/Rocm/Cpu), `EnumerationSource` (7 variants including `Cpu` — the addendum from `docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md` has already been applied to the live source at `crates/anvilml-core/src/types/hardware.rs`), `CapabilitySource` (PyTorch/DeviceTable/Fallback), and `InferenceCaps` (with `Default` derive, all fields `false`).

The established test style in this project uses integration test files in `crates/{name}/tests/` that import only public API via `use anvilml_core::types::*;`. Tests use plain `#[test]` functions (no `serial` annotation needed since no env vars or shared state are mutated). The test catalogue in `docs/TESTS.md` follows a structured per-entry format with File, Context, Tests, Mode, Inputs, Expected output, and Acceptance columns.

No gap exists between the design doc and current source: `EnumerationSource::Cpu` is present in the live `hardware.rs`, `InferenceCaps::default()` produces all-false capabilities, and `GpuDevice`'s `arch` field is `Option<String>` (None for CPU).

## Resolved Dependencies

None. This task introduces no new external crates or packages. It uses only `anvilml-core` types already declared as a dependency in `anvilml-hardware`'s `Cargo.toml`.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | | | | |

## Approach

1. **Create `crates/anvilml-hardware/src/cpu.rs`.** Define a zero-field unit struct `CpuDetector` and implement `DeviceDetector` for it:
   - `detect(&self) -> Result<Vec<GpuDevice>, AnvilError>`: construct a single `GpuDevice` with the exact field values from the task context and return `Ok(vec![device])`. The device fields are:
     - `index: 0` — single device, zero-based index.
     - `name: "CPU".into()` — human-readable name.
     - `device_type: DeviceType::Cpu` — CPU backend.
     - `vram_total_mib: 0` — CPU has no VRAM.
     - `vram_free_mib: 0` — CPU has no VRAM.
     - `driver_version: "n/a".into()` — no driver string.
     - `pci_vendor_id: 0` — no PCI device.
     - `pci_device_id: 0` — no PCI device.
     - `arch: None` — CPU has no GPU architecture string.
     - `caps: InferenceCaps::default()` — all inference capability flags are `false` (the default).
     - `enumeration_source: EnumerationSource::Cpu` — marks this as a synthesized fallback device, distinct from `EnumerationSource::Mock` (env-var-driven, P4-A3) and from the four real-enumeration variants.
     - `capabilities_source: CapabilitySource::Fallback` — pre-spawn hint, not authoritative.
   - `refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError>`: return `Ok((0, 0))`. The `_index` parameter is underscore-prefixed to suppress the unused-variable lint warning.

   Both methods never return `Err` and never panic — they are pure value construction with no I/O, no fallible operations, no conditional branches.

   Add a `///` doc comment on `CpuDetector` describing it as the unconditional final-fallback detector that guarantees `detect_all_devices()` always returns at least one device, per §6.2 of the design.

2. **Update `crates/anvilml-hardware/src/lib.rs`.** Add `pub mod cpu;` after the existing `pub mod detect;` line. The file already has a crate-level `//!` doc comment and the `pub use detect::DeviceDetector;` re-export. Keep it under 80 lines (it will be ~5 lines).

3. **Create `crates/anvilml-hardware/tests/cpu_tests.rs`.** Write ≥4 integration tests:
   - `test_cpu_detector_returns_one_device`: construct `CpuDetector`, call `detect()`, assert the result is `Ok(vec![..])` with exactly one element, and verify the device's `name == "CPU"`.
   - `test_cpu_detector_device_type_is_cpu`: assert `device.device_type == DeviceType::Cpu`.
   - `test_cpu_detector_enumeration_source_is_cpu`: assert `device.enumeration_source == EnumerationSource::Cpu`.
   - `test_cpu_detector_refresh_vram_returns_zero`: construct `CpuDetector`, call `refresh_vram(0)`, assert `Ok((0, 0))`.

   Additional tests to reach a robust count:
   - `test_cpu_detector_all_device_fields`: assert every other field on the returned `GpuDevice` (vram_total_mib, vram_free_mib, driver_version, pci_vendor_id, pci_device_id, arch, caps, capabilities_source) has the expected value.
   - `test_cpu_detect_never_errors`: call `detect()` and assert `result.is_ok()` (proving it never panics or returns Err).

   All tests import from the crate's public API: `use anvilml_hardware::detect::DeviceDetector;` and `use anvilml_core::types::*;`.

4. **Update `crates/anvilml-hardware/Cargo.toml`.** Change `version.workspace = true` to `version = "0.1.1"` (bump patch from 0.1.0 to 0.1.1). The workspace version (`0.1.0`) is read-only per §12 of ENVIRONMENT.md.

5. **Update `docs/TESTS.md`.** Add entries for the new test file following the existing catalogue format, one entry per test function.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `anvilml_hardware::cpu::CpuDetector` | `pub struct CpuDetector;` |
| trait impl | `anvilml_hardware::cpu` | `impl DeviceDetector for CpuDetector` |
| method | `CpuDetector::detect` | `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` |
| method | `CpuDetector::refresh_vram` | `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/cpu.rs` | `CpuDetector` struct + `DeviceDetector` impl |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod cpu;` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version `0.1.0 → 0.1.1` |
| CREATE | `crates/anvilml-hardware/tests/cpu_tests.rs` | ≥4 integration tests for `CpuDetector` |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for cpu_tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detector_returns_one_device` | `detect()` returns `Ok(vec![..])` with exactly one element; device name is `"CPU"` | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_returns_one_device` exits 0 |
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detector_device_type_is_cpu` | Returned device has `device_type == DeviceType::Cpu` | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_device_type_is_cpu` exits 0 |
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detector_enumeration_source_is_cpu` | Returned device has `enumeration_source == EnumerationSource::Cpu` (distinct from `Mock`) | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_enumeration_source_is_cpu` exits 0 |
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detector_refresh_vram_returns_zero` | `refresh_vram(0)` returns `Ok((0, 0))` — CPU has no VRAM | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_refresh_vram_returns_zero` exits 0 |
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detector_all_device_fields` | Every field on the returned `GpuDevice` matches expected values (vram=0, driver="n/a", pci_ids=0, arch=None, caps=default, capabilities_source=Fallback) | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_all_device_fields` exits 0 |
| `crates/anvilml-hardware/tests/cpu_tests.rs` | `test_cpu_detect_never_errors` | `detect()` never returns `Err` or panics — pure value construction | `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detect_never_errors` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-hardware/tests/` which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or CI job configurations are needed.

## Platform Considerations

None identified. The `CpuDetector` implementation is pure value construction with no platform-specific code paths, no `#[cfg(unix)]`/`#[cfg(windows)]` guards, and no I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `EnumerationSource::Cpu` variant may not exist in the live source if the addendum was not applied | Low | High — compilation failure | Verified: `crates/anvilml-core/src/types/hardware.rs` line 97 already contains `Cpu` as the fifth variant. The addendum has been applied. |
| `InferenceCaps::default()` may not produce all-false fields as expected | Low | Medium — test assertion mismatch | Verified: `crates/anvilml-core/src/types/hardware.rs` line 39 derives `Default` on `InferenceCaps` with all `bool` fields, which defaults to `false` for each. |
| Unused `_index` parameter in `refresh_vram` triggers clippy warning | Low | Medium — clippy `-D warnings` fails | Prefix with underscore (`_index`) to suppress the warning, matching the convention used elsewhere in the codebase (e.g., `refresh_vram` in MockDetector). |
| `docs/TESTS.md` update may conflict with concurrent edits | Low | Low — merge conflict resolved at staging | The test entries are appended at the end of the file; git merge handles this cleanly. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-hardware` exits 0
- [ ] `cargo test -p anvilml-hardware --test cpu_tests` exits 0 (≥4 tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
