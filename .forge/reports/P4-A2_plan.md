# Plan Report: P4-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2                                         |
| Phase       | 004 — Hardware Detection                     |
| Description | anvilml-hardware: mock detector (feature mock-hardware, env-driven) |
| Depends on  | P4-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T22:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Create a mock GPU detector in `crates/anvilml-hardware/src/mock.rs`, gated behind the `mock-hardware` feature flag. `MockDetector` reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) with built-in defaults (cpu, 8192 MiB, gfx1100) and returns a single deterministic `GpuDevice`. This enables CI and local testing without physical GPU hardware. Three fixture tests validate each device type path.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/mock.rs` implementing `MockDetector`
- Implement `DeviceDetector` trait for `MockDetector`
- Read `ANVILML_MOCK_DEVICE_TYPE` (cpu/cuda/rocm, default cpu)
- Read `ANVILML_MOCK_VRAM_MIB` (default 8192)
- Read `ANVILML_MOCK_GFX_ARCH` (default gfx1100)
- Return one deterministic `GpuDevice` per detection call
- Add `serial_test` as dev-dependency in `anvilml-hardware/Cargo.toml`
- Modify `crates/anvilml-hardware/src/lib.rs` to conditionally include the mock module and re-export types under the feature flag
- Three fixture unit tests covering cpu, cuda, and rocm device paths

### Out of Scope
- CUDA detector (nvidia-smi parsing) — P4-A3
- ROCm detector (rocm-smi / HIP probe) — P4-A4
- `detect_all_devices` orchestration logic — P4-A5
- Server integration (`GET /v1/system`) — P4-A6
- CI workflow changes
- Feature forwarding in dependent crates (worker, scheduler, server)
- Any changes to `anvilml-core` types or modules

## Approach

1. **Add `serial_test` dev-dependency to `crates/anvilml-hardware/Cargo.toml`.**
   - Add `serial_test = "3.5"` under `[dev-dependencies]` (latest stable is 3.5.0).
   - This crate provides the `#[serial]` attribute for serializing tests that share global state (environment variables).

2. **Create `crates/anvilml-hardware/src/mock.rs`.**
   - Define `pub struct MockDetector;` (unit struct, no state needed — reads env vars on each call).
   - Implement `DeviceDetector` for `MockDetector`, gated with `#[cfg(feature = "mock-hardware")]`:
     - `detect()`: read env vars via `std::env::var`, map `ANVILML_MOCK_DEVICE_TYPE` string to `DeviceType` enum variant (cpu→Cpu, cuda→Cuda, rocm→Rocm; default Cpu). Parse `ANVILML_MOCK_VRAM_MIB` as u32 (default 8192). Use `ANVILML_MOCK_GFX_ARCH` string for the device name (default gfx1100). Construct and return `vec![GpuDevice { index: 0, name: arch, device_type: mapped, vram_total_mib, vram_free_mib: vram_total_mib, driver_version: "mock" }]`.
     - `refresh_vram(idx, ...)`: return `(vram_total_mib, vram_total_mib)` — mock always reports fully free VRAM regardless of index.
   - Add a helper function or impl block for env-var parsing with defaults (e.g. `fn parse_device_type(s: &str) -> DeviceType`).

3. **Modify `crates/anvilml-hardware/src/lib.rs`.**
   - Add `#[cfg(feature = "mock-hardware")] pub mod mock;` to declare the mock module conditionally.
   - The existing `pub mod cpu;`, trait, and re-exports remain unchanged.
   - No additional re-exports needed beyond what's already in lib.rs (the mock detector is accessed via `hardware::mock::MockDetector`).

4. **Add three fixture tests inside `src/mock.rs` under `#[cfg(test)] mod tests`.**
   - Each test must be annotated with `#[serial]` from `serial_test` to prevent env-var pollution across tests.
   - Test 1 `mock_device_type_cpu`: set `ANVILML_MOCK_DEVICE_TYPE=cpu`, assert returned device has `device_type == DeviceType::Cpu` and `name == "gfx1100"` (default arch).
   - Test 2 `mock_device_type_cuda`: set `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576`, assert `device_type == DeviceType::Cuda`, `vram_total_mib == 24576`, `name == "gfx1100"`.
   - Test 3 `mock_device_type_rocm`: set `ANVILML_MOCK_DEVICE_TYPE=rocm` and `ANVILML_MOCK_GFX_ARCH=gfx1102`, assert `device_type == DeviceType::Rocm`, `name == "gfx1102"`, `vram_total_mib == 8192` (default).

5. **Verify locally.**
   - Run `cargo test -p anvilml-hardware --features mock-hardware -- mock` and confirm exit code 0 with all 3 tests passing.
   - Also run `cargo clippy -p anvilml-hardware --features mock-hardware -D warnings` to check for lint issues.

## Files Affected

| Action   | Path                                          | Description                                                  |
|----------|-----------------------------------------------|--------------------------------------------------------------|
| CREATE   | crates/anvilml-hardware/src/mock.rs           | MockDetector struct + DeviceDetector impl + 3 fixture tests  |
| MODIFY   | crates/anvilml-hardware/Cargo.toml            | Add `serial_test = "3.5"` to `[dev-dependencies]`            |
| MODIFY   | crates/anvilml-hardware/src/lib.rs            | Add `#[cfg(feature = "mock-hardware")] pub mod mock;`        |

## Tests

| Test ID / Name          | File                                        | Validates                                    |
|-------------------------|---------------------------------------------|----------------------------------------------|
| `mock_device_type_cpu`  | `crates/anvilml-hardware/src/mock.rs`       | MockDetector returns Cpu device with default VRAM and arch when ANVILML_MOCK_DEVICE_TYPE=cpu |
| `mock_device_type_cuda` | `crates/anvilml-hardware/src/mock.rs`       | MockDetector returns Cuda device with custom VRAM (24576 MiB) and default arch |
| `mock_device_type_rocm` | `crates/anvilml-hardware/src/mock.rs`       | MockDetector returns Rocm device with default VRAM and custom arch (gfx1102) |

## CI Impact

No CI changes required. The existing CI matrix already runs `cargo test -p anvilml-hardware --features mock-hardware` as part of the workspace test gate (ARCHITECTURE.md §9, TASKS_PHASE004.md). Adding `serial_test` as a dev-dependency does not alter any CI steps or workflow definitions. The `-- mock` filter used in the task acceptance criterion is a standard cargo test filter and works with the existing CI setup.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `serial_test` version API mismatch (e.g. `#[serial]` attribute location) | Low | Medium | Pin to a well-tested minor version (`3.5`). Verify the crate docs before implementation; if the derive macro path differs, adjust the test annotations accordingly. |
| Env-var tests pollute each other despite `#[serial]` | Low | Medium | The `serial_test` crate serializes all tests marked `#[serial]` within a single test file, so env-var state is isolated per test. If issues arise, scope env-var sets to individual test functions rather than module-level. |
| Invalid `ANVILML_MOCK_DEVICE_TYPE` value causes panic | Medium | Low | The task spec says the valid values are cpu/cuda/rocm; for robustness, treat any unrecognized string as default (Cpu) via `.unwrap_or(DeviceType::Cpu)` or a match with a `_ => Cpu` catch-all. This prevents panics in CI if an env var is accidentally set to an unexpected value. |
| `ANVILML_MOCK_VRAM_MIB` parse fails on non-numeric input | Low | Medium | Use `.ok().and_then(|s| s.parse().ok())` with a default of 8192 MiB, so invalid values silently fall back to the default rather than propagating an error through `detect()`. |

## Acceptance Criteria

- [ ] `crates/anvilml-hardware/src/mock.rs` defines `pub struct MockDetector` implementing `DeviceDetector`
- [ ] `MockDetector::detect()` reads `ANVILML_MOCK_DEVICE_TYPE` (cpu/cuda/rocm, default cpu), `ANVILML_MOCK_VRAM_MIB` (default 8192), `ANVILML_MOCK_GFX_ARCH` (default gfx1100)
- [ ] `MockDetector::detect()` returns exactly one `GpuDevice` with correct field values for the configured device type
- [ ] `MockDetector::refresh_vram()` returns `(vram_total_mib, vram_total_mib)` for any index
- [ ] `crates/anvilml-hardware/Cargo.toml` includes `serial_test = "3.5"` in `[dev-dependencies]`
- [ ] `crates/anvilml-hardware/src/lib.rs` conditionally declares `pub mod mock` behind `feature = "mock-hardware"`
- [ ] Three fixture tests exist and are annotated with `#[serial]` from `serial_test`
- [ ] `cargo test -p anvilml-hardware --features mock-hardware -- mock` exits 0 with all 3 tests passing
