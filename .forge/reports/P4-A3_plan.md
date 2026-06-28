# Plan Report: P4-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A3                                       |
| Phase       | 004 — Hardware Detection: Detectors         |
| Description | anvilml-hardware: MockDetector env-var driven stub |
| Depends on  | P4-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T23:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-hardware/src/mock.rs` containing `MockDetector`, a `DeviceDetector` implementation gated behind the `mock-hardware` feature flag. It reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_DEVICE_NAME`) with defaults `"cpu"`, `8192`, and `"Mock GPU"` respectively, and returns one synthetic `GpuDevice` whose fields are derived from those values. This detector is the one every CI job exercises when built with `--features mock-hardware`.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/mock.rs`: `MockDetector` struct (zero-sized) implementing `DeviceDetector`, gated `#[cfg(feature = "mock-hardware")]`.
- `detect()` returns `Ok(vec![GpuDevice { ... }])` with one device whose fields are populated from env vars: `device_type` from `ANVILML_MOCK_DEVICE_TYPE` (cuda→Cuda, rocm→Rocm, else Cpu), `vram_total_mib` and `vram_free_mib` from `ANVILML_MOCK_VRAM_MIB`, `name` from `ANVILML_MOCK_DEVICE_NAME`, `enumeration_source = EnumerationSource::Mock`, `capabilities_source = CapabilitySource::Fallback`.
- `refresh_vram()` returns `Ok((vram_mib, vram_mib))` — both total and free equal the VRAM env value (no real hardware query in mock mode).
- Declare `mod mock;` in `lib.rs` with the same `#[cfg(feature = "mock-hardware")]` gate.
- Create `crates/anvilml-hardware/tests/mock_tests.rs` with ≥4 tests.
- Bump `anvilml-hardware` crate patch version (0.1.1 → 0.1.2).

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No deferrals.

## Existing Codebase Assessment

The `anvilml-hardware` crate exists as a buildable stub (introduced in Phase 1's P1-B2). It currently contains two source files: `detect.rs` (the `DeviceDetector` trait with `detect()` and `refresh_vram()` methods) and `cpu.rs` (`CpuDetector` implementation, always returning one synthesized CPU device). The `lib.rs` re-exports these two modules.

The established pattern is clear: zero-sized detector structs implementing `DeviceDetector`, returning `Result<Vec<GpuDevice>, AnvilError>` from `detect()` and `Result<(u32, u32), AnvilError>` from `refresh_vram()`. Error types come from `anvilml-core::AnvilError`. The `GpuDevice` struct (defined in `anvilml-core/src/types/hardware.rs`) has all required fields: `index`, `name`, `device_type`, `vram_total_mib`, `vram_free_mib`, `driver_version`, `pci_vendor_id`, `pci_device_id`, `arch`, `caps`, `enumeration_source`, `capabilities_source`.

The test style uses integration test files in `crates/anvilml-hardware/tests/`, importing via `use anvilml_core::types::*` and the crate's public module paths. Tests are simple assertion-based with `#[test]` attributes, no test frameworks beyond `assert_eq!`/`assert!`.

The design doc (§6.7) and task context agree on the three env var names, defaults, and the return semantics. No gap detected.

## Resolved Dependencies

None. This task introduces no new external crates. It only uses types already in `anvilml-core` (`GpuDevice`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`, `AnvilError`) and standard library (`std::env`).

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Bump crate version.** Read `crates/anvilml-hardware/Cargo.toml`, change `version = "0.1.1"` to `version = "0.1.2"`. This follows §12 of ENVIRONMENT.md — every task modifying source files bumps the patch version.

2. **Create `crates/anvilml-hardware/src/mock.rs`.** Write the file with:
   - A module-level doc comment describing `MockDetector` and its env-var contract per §6.7.
   - `pub struct MockDetector;` — zero-sized, no fields.
   - `use crate::detect::DeviceDetector;` import.
   - `use anvilml_core::{AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};` imports.
   - `impl DeviceDetector for MockDetector`:
     - `detect(&self) -> Result<Vec<GpuDevice>, AnvilError>`: read the three env vars via `std::env::var`, apply defaults, parse `device_type` (match `"cuda"` → `Cuda`, `"rocm"` → `Rocm`, else → `Cpu`), construct a single `GpuDevice` with `index=0`, `name` from env, `vram_total_mib=vram_free_mib=vr` from env, `driver_version="mock"`, `pci_vendor_id=0`, `pci_device_id=0`, `arch=None`, `caps=InferenceCaps::default()`, `enumeration_source=Mock`, `capabilities_source=Fallback`. Return `Ok(vec![device])`.
     - `refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError>`: read `ANVILML_MOCK_VRAM_MIB` with default 8192, return `Ok((vr, vr))`.
   - Inline comment on the `else` branch of the device_type match explaining that any unrecognized value falls back to CPU (matching the design doc's "cuda, rocm, or cpu" constraint).
   - Inline comment on `_index` underscore prefix explaining it is unused because mock mode has no per-device VRAM query.

3. **Update `crates/anvilml-hardware/src/lib.rs`.** Add `#[cfg(feature = "mock-hardware")]` gated `pub mod mock;` and `pub use mock::MockDetector;` after the existing `cpu`/`detect` entries. Keep `lib.rs` under 80 lines (it will be ~10 lines).

4. **Create `crates/anvilml-hardware/tests/mock_tests.rs`.** Write ≥4 integration tests:
   - `test_mock_detector_defaults`: No env vars set; verify all default values (device_type=Cpu, vram=8192, name="Mock GPU", enumeration_source=Mock, capabilities_source=Fallback).
   - `test_mock_cuda_device_type`: Set `ANVILML_MOCK_DEVICE_TYPE=cuda`; verify device_type=Cuda.
   - `test_mock_rocm_device_type`: Set `ANVILML_MOCK_DEVICE_TYPE=rocm`; verify device_type=Rocm.
   - `test_mock_vram_override`: Set `ANVILML_MOCK_VRAM_MIB=16384`; verify vram_total_mib=16384 and vram_free_mib=16384.
   - `test_mock_device_name_override`: Set `ANVILML_MOCK_DEVICE_NAME=Test GPU`; verify name="Test GPU".
   - `test_mock_refresh_vram`: Call `refresh_vram(0)` with default VRAM; verify `Ok((8192, 8192))`.
   - Every env-var test uses the `#[serial]` attribute (per §11.3 of ENVIRONMENT.md / §9.6 of FORGE_AGENT_RULES.md) and captures/restores the prior env value unconditionally in a `drop` guard or `match prior { ... }` at function end.

5. **Verify compilation.** Run `cargo check -p anvilml-hardware --features mock-hardware` to confirm the new module compiles with the feature gate.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub struct MockDetector` | `anvilml_hardware::mock::MockDetector` | Zero-sized struct implementing `DeviceDetector` |
| `pub fn detect` | `MockDetector::detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` | Returns one synthetic device from env vars |
| `pub fn refresh_vram` | `MockDetector::refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` | Returns (vram_mib, vram_mib) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/mock.rs` | MockDetector implementation, feature-gated |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `mod mock;` and `pub use mock::MockDetector;` with feature gate |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |
| CREATE | `crates/anvilml-hardware/tests/mock_tests.rs` | ≥4 integration tests with env-var isolation |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_detector_defaults` | All three env vars absent → defaults applied (Cpu, 8192, "Mock GPU") | No env vars set | None (env unset) | `Ok([GpuDevice { device_type: Cpu, vram_total_mib: 8192, ... }])` | `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_detector_defaults` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_cuda_device_type` | `ANVILML_MOCK_DEVICE_TYPE=cuda` → device_type is Cuda | Prior value captured | `ANVILML_MOCK_DEVICE_TYPE=cuda` | `device_type == DeviceType::Cuda` | same test binary, `-- test_mock_cuda_device_type` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_rocm_device_type` | `ANVILML_MOCK_DEVICE_TYPE=rocm` → device_type is Rocm | Prior value captured | `ANVILML_MOCK_DEVICE_TYPE=rocm` | `device_type == DeviceType::Rocm` | same test binary, `-- test_mock_rocm_device_type` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_vram_override` | `ANVILML_MOCK_VRAM_MIB=16384` → vram_total_mib and vram_free_mib both 16384 | Prior value captured | `ANVILML_MOCK_VRAM_MIB=16384` | `vram_total_mib == 16384 && vram_free_mib == 16384` | same test binary, `-- test_mock_vram_override` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_device_name_override` | `ANVILML_MOCK_DEVICE_NAME=Test GPU` → name is "Test GPU" | Prior value captured | `ANVILML_MOCK_DEVICE_NAME=Test GPU` | `name == "Test GPU"` | same test binary, `-- test_mock_device_name_override` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_mock_refresh_vram` | `refresh_vram(0)` returns `(8192, 8192)` with defaults | No env vars set | None | `Ok((8192, 8192))` | same test binary, `-- test_mock_refresh_vram` exits 0 |

## CI Impact

No CI changes required. The `mock-hardware` feature is already declared in the workspace and forwarded by `anvilml-worker`, `anvilml-scheduler`, `anvilml-server`, and `backend` (per ARCHITECTURE.md §5). All CI jobs already build with `--features mock-hardware`, so the new `mock.rs` module and its tests will be compiled and exercised automatically by the existing `cargo test --workspace --features mock-hardware` command. The new test file `mock_tests.rs` is picked up by the default Rust test crate discovery (any `tests/*.rs` file in a crate's `tests/` directory).

## Platform Considerations

None identified. The `mock.rs` module uses only `std::env::var` which is cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `std::env::var` returns `Err(NotPresent)` for unset env vars — the `unwrap_or_else` default must handle this correctly for all three vars. | Low | Medium | Use `std::env::var(name).unwrap_or_else(|_| default_string)`. Test the defaults path explicitly (no env vars set) as the first test case. |
| The `device_type` match on env string value uses exact case-sensitive comparison — `"CUDA"` or `"CUDA "` would fall through to `else → Cpu`. This matches the design doc's "cuda, rocm, or cpu" constraint (lowercase only). | Low | Low | Document in an inline comment that the env var must be lowercase per the design spec. The design doc §6.7 table explicitly lists lowercase values. |
| Test env-var tests may interfere with each other due to process-global `std::env`. | High | High | Every env-var test uses `#[serial]` (from `serial_test` crate) and captures/restores the prior value unconditionally at function end, per ENVIRONMENT.md §11.3 / FORGE_AGENT_RULES.md §9.6. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests` exits 0 with ≥6 tests passing
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
