# Plan Report: P4-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2                                         |
| Phase       | 004 — Hardware Detection                     |
| Description | anvilml-hardware: mock detector (feature mock-hardware, env-driven) |
| Depends on  | P4-A1                                          |
| Project     | anvilml                                        |
| Planned at  | 2026-06-01T16:36:14Z                           |
| Attempt     | 1                                              |

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

## Approach

1. **Add `serial_test` dev-dependency** to `crates/anvilml-hardware/Cargo.toml`. Use version `2.0` (latest stable) which supports `#[serial]` for isolating env-var tests.

2. **Create `src/mock.rs`** with:
   - `MockDetector` struct (derive `Debug, Clone, Default`).
   - `impl DeviceDetector for MockDetector`:
     - `detect()`: read env vars via `std::env::var`, map `ANVILML_MOCK_DEVICE_TYPE` string to `DeviceType` enum variant (cpu→Cpu, cuda→Cuda, rocm→Rocm; default Cpu). Parse `ANVILML_MOCK_VRAM_MIB` as u32 (default 8192). Construct and return `vec![GpuDevice { index: 0, name: <arch string>, device_type: <mapped>, vram_total_mib, vram_free_mib: vram_total_mib, driver_version: "mock" }]`.
     - `refresh_vram()`: return `(vram_total_mib, vram_total_mib)` (mock always reports fully free).
   - Use `#[cfg(feature = "mock-hardware")]` guards on the module and impl.

3. **Modify `src/lib.rs`**:
   - Add `#[cfg(feature = "mock-hardware")] pub mod mock;`
   - Add `#[cfg(feature = "mock-hardware")] pub use anvilml_core::{AnvilError, DeviceType, GpuDevice};` (or keep existing re-export and ensure it works with feature flag).
   - The existing re-exports already cover the types needed.

4. **Write three fixture tests** in `src/mock.rs` under `#[cfg(all(test, feature = "mock-hardware"))]`:
   - Test 1: `ANVILML_MOCK_DEVICE_TYPE=cpu` → device_type is Cpu, vram=8192.
   - Test 2: `ANVILML_MOCK_DEVICE_TYPE=cuda` → device_type is Cuda, vram=8192.
   - Test 3: `ANVILML_MOCK_DEVICE_TYPE=rocm` → device_type is Rocm, vram=8192.
   - Each test annotated with `#[serial]` to prevent env-var interference.

5. **Verify**: Run `cargo test -p anvilml-hardware --features mock-hardware -- mock` and confirm exit code 0 with 3 tests passing.

## Files Affected

| Action   | Path                                           | Description                                              |
|----------|------------------------------------------------|----------------------------------------------------------|
| MODIFY   | crates/anvilml-hardware/Cargo.toml             | Add `serial_test = "2.0"` to `[dev-dependencies]`         |
| CREATE   | crates/anvilml-hardware/src/mock.rs            | MockDetector impl + 3 fixture tests                      |
| MODIFY   | crates/anvilml-hardware/src/lib.rs             | Conditionally include `mock` module and re-export types   |

## Tests

| Test ID / Name        | File                              | Validates                                  |
|-----------------------|-----------------------------------|--------------------------------------------|
| mock_device_cpu       | crates/anvilml-hardware/src/mock.rs | ANVILML_MOCK_DEVICE_TYPE=cpu → Cpu device  |
| mock_device_cuda      | crates/anvilml-hardware/src/mock.rs | ANVILML_MOCK_DEVICE_TYPE=cuda → Cuda device |
| mock_device_rocm      | crates/anvilml-hardware/src/mock.rs | ANVILML_MOCK_DEVICE_TYPE=rocm → Rocm device  |

## CI Impact

No CI changes required. The `mock-hardware` feature flag is already wired into the CI matrix (see ARCHITECTURE.md §9: all CI Rust jobs use `--features mock-hardware`). No new CI jobs or steps are needed.

## Risks and Mitigations

| Risk                              | Likelihood | Impact | Mitigation                                       |
|-----------------------------------|-----------|--------|--------------------------------------------------|
| `serial_test` version incompatibility with Rust 1.95 toolchain | Low | Medium | Pin to a known-compatible version; verify with `cargo check` before writing tests |
| Env-var tests leaking state between parallel runs | High | Medium | Use `#[serial]` attribute on each test; `serial_test` serializes execution per test function |
| `ANVILML_MOCK_DEVICE_TYPE` accepts invalid values causing panic | Low | Low | Parse with `.unwrap_or(DeviceType::Cpu)` fallback to default on unrecognized string |
| Missing `env_logger` init in tests causes warnings | Low | Low | Add `#[cfg(test)] fn test_init() { let _ = env_logger::try_init(); }` or suppress with `env_logger::Builder::from_env(...).is_test(true).init()` |

## Acceptance Criteria

- [ ] `crates/anvilml-hardware/src/mock.rs` exists and contains `MockDetector` implementing `DeviceDetector`
- [ ] `MockDetector::detect()` returns a single `GpuDevice` whose `device_type` matches the value of `ANVILML_MOCK_DEVICE_TYPE` (cpu, cuda, rocm)
- [ ] `MockDetector::detect()` uses default values when env vars are unset: device_type=Cpu, vram=8192 MiB, gfx_arch=gfx1100
- [ ] `serial_test` is listed in `[dev-dependencies]` of `crates/anvilml-hardware/Cargo.toml`
- [ ] `crates/anvilml-hardware/src/lib.rs` conditionally includes the mock module under `#[cfg(feature = "mock-hardware")]`
- [ ] `cargo test -p anvilml-hardware --features mock-hardware -- mock` exits 0 with exactly 3 passing tests
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -D warnings` passes with no warnings
