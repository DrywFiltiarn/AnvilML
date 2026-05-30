# Plan Report: P3-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A2                                       |
| Phase       | 003 — Hardware Detection                    |
| Description | anvilml-hardware: mock detector driven by env vars |
| Depends on  | P3-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T09:53:50Z                        |
| Attempt     | 1                                           |

## Objective

Implement the deterministic mock detector (`MockDetector`) used in all CI runs, gated behind the `mock-hardware` feature flag. The mock detector reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) and returns a single deterministic `GpuDevice` with the specified values. When the `mock-hardware` feature is active, `detect_all_devices()` must use `MockDetector` exclusively, ensuring CI is fully hermetic without any real GPU hardware.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/mock.rs` with `MockDetector` struct implementing the `DeviceDetector` trait.
- `MockDetector::detect()` reads `ANVILML_MOCK_DEVICE_TYPE` (default `cpu`), `ANVILML_MOCK_VRAM_MIB` (default `8192`), and `ANVILML_MOCK_GFX_ARCH` (default `gfx1100`) from the environment.
- When `mock-hardware` feature is active, `detect_all_devices()` in `lib.rs` uses `MockDetector` instead of `CpuDetector`.
- Add three cfg-gated fixture tests in `mock.rs`: (1) default env → `DeviceType::Cpu`, (2) `ANVILML_MOCK_DEVICE_TYPE=cuda` → `DeviceType::Cuda` with correct VRAM, (3) `ANVILML_MOCK_DEVICE_TYPE=rocm` + custom VRAM → `DeviceType::Rocm`.
- Add `serial_test` as a dev-dependency to serialize env-var tests and avoid cross-test pollution.

### Out of Scope
- CUDA detector (P3-A3) — separate task.
- ROCm detector (P3-A4) — separate task.
- `HardwareOverrideConfig` wiring and host info (P3-B1) — separate task.
- Feature forwarding to other crates (`worker`, `scheduler`, `server`) — out of scope for this phase.
- Integration tests covering override scenarios — covered in P3-B1.

## Approach

1. **Create `crates/anvilml-hardware/src/mock.rs`:**
   - Define `pub struct MockDetector;` (unit struct, no fields).
   - Implement `DeviceDetector for MockDetector`:
     - `detect()`: read env vars with defaults, construct a single `GpuDevice`.
       - `ANVILML_MOCK_DEVICE_TYPE`: parse as `cpu`, `cuda`, or `rocm`; default `"cpu"`. Map to `DeviceType::Cpu`, `DeviceType::Cuda`, `DeviceType::Rocm`.
       - `ANVILML_MOCK_VRAM_MIB`: parse as `u32`; default `8192`.
       - `ANVILML_MOCK_GFX_ARCH`: read as `String`; default `"gfx1100"`.
       - Device name: derive from device type — `"Mock CPU"`, `"Mock CUDA GPU"`, or `"Mock ROCm GPU"`.
       - `driver_version`: set to `"mock"` for all types.
       - `vram_free_mib`: equal to `vram_total_mib` (fully free in mock).
       - `index`: always `0`.
     - `refresh_vram()`: return `(vram_free_mib, vram_total_mib)` from the mock config.
   - Write three `#[cfg(test)]` tests gated with `#[cfg(feature = "mock-hardware")]` and `#[serial_test::serial]`:
     - `mock_default_returns_cpu`: no env vars set → verifies DeviceType::Cpu, vram_total_mib=8192.
     - `mock_cuda_device_type`: `ANVILML_MOCK_DEVICE_TYPE=cuda` → verifies DeviceType::Cuda.
     - `mock_rocm_custom_vram`: `ANVILML_MOCK_DEVICE_TYPE=rocm` + `ANVILML_MOCK_VRAM_MIB=16384` → verifies DeviceType::Rocm and vram_total_mib=16384.
   - Each test sets env vars via `std::env::set_var`, runs the detector, asserts results, then calls `std::env::remove_var` to clean up.

2. **Modify `crates/anvilml-hardware/src/lib.rs`:**
   - Add conditional module declaration: `#[cfg(feature = "mock-hardware")] pub mod mock;`
   - Modify `detect_all_devices()` to use conditional compilation:
     - When `mock-hardware` feature is active: instantiate `MockDetector`, call `detect()`, return `HardwareInfo` with mock devices.
     - When `mock-hardware` feature is not active: keep existing behavior (call `CpuDetector`).
   - The conditional `detect_all_devices()` must produce a `HardwareInfo` with the same shape in both branches.

3. **Modify `crates/anvilml-hardware/Cargo.toml`:**
   - Add `serial_test = "2"` under `[dev-dependencies]` for test serialization.

## Files Affected

| Action   | Path                              | Description                                              |
|----------|-----------------------------------|----------------------------------------------------------|
| CREATE   | crates/anvilml-hardware/src/mock.rs | MockDetector impl, DeviceDetector trait, 3 fixture tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs  | Conditional mock module import; conditional detect_all_devices() |
| MODIFY   | crates/anvilml-hardware/Cargo.toml  | Add serial_test dev-dependency                           |

## Tests

| Test ID / Name            | File                                | Validates                                              |
|---------------------------|-------------------------------------|--------------------------------------------------------|
| `mock_default_returns_cpu` | crates/anvilml-hardware/src/mock.rs  | Default env vars → DeviceType::Cpu, vram_total_mib=8192 |
| `mock_cuda_device_type`    | crates/anvilml-hardware/src/mock.rs  | ANVILML_MOCK_DEVICE_TYPE=cuda → DeviceType::Cuda        |
| `mock_rocm_custom_vram`   | crates/anvilml-hardware/src/mock.rs  | ANVILML_MOCK_DEVICE_TYPE=rocm + custom VRAM → DeviceType::Rocm, correct vram_total_mib |

## CI Impact

No CI changes required. The `mock-hardware` feature is already referenced in CI workflows (see ARCHITECTURE.md §9). This task only adds code behind an existing feature flag.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Env var pollution between tests causing false failures | Medium | High | Use `serial_test::serial` attribute to serialize all mock tests; clean up env vars after each test |
| `detect_all_devices()` conditional compilation causes feature-unstable code when `mock-hardware` is inactive | Low | Medium | Use `#[cfg(feature = "mock-hardware")]` guards consistently; ensure both branches produce identical `HardwareInfo` shape |
| `serial_test` crate API mismatch with pinned version | Low | Low | Pin to a well-known stable version (2.x); verify API before implementation |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware -- mock` exits 0 with ≥3 passing fixture tests
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all --check` exits 0 (proper formatting)
- [ ] `detect_all_devices()` returns mock devices when `mock-hardware` feature is active
- [ ] `detect_all_devices()` returns CPU devices when `mock-hardware` feature is inactive (no regression of P3-A1)
- [ ] All three env vars (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) are read with correct defaults
- [ ] Mock detector implements `DeviceDetector` trait (object-safe, usable as `Box<dyn DeviceDetector>`)
