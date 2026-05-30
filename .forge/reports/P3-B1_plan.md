# Plan Report: P3-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-B1                                       |
| Phase       | 003 — Hardware Detection                    |
| Description | anvilml-hardware: HardwareOverrideConfig integration and detect_all_devices tests |
| Depends on  | P3-A1, P3-A2, P3-A3, P3-A4                  |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T13:17:09Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `detect_all_devices()` function in `anvilml-hardware/src/lib.rs` with full override wiring and real `HostInfo` population. The function must accept an optional `HardwareOverrideConfig` reference that, when present, forces the detector pipeline to use only the specified device type (skipping all other detectors). When no override is provided, the existing auto-detection sequence (CUDA → ROCm → CPU fallback) runs unchanged. Additionally, populate the `HostInfo` fields (`os`, `cpu_model`, `ram_total_mib`, `ram_free_mib`) using the `sysinfo` crate instead of leaving them blank or zeroed. Finally, add integration tests covering override scenarios to bring the total test count to ≥8.

## Scope

### In Scope
- Modify `detect_all_devices()` signature in `crates/anvilml-hardware/src/lib.rs` to accept `override: Option<&HardwareOverrideConfig>`.
- Implement override priority logic:
  - When `override` is `Some`: check `override.device_type` and run only the matching detector (CudaDetector, RocmDetector, or CpuDetector).
  - When `override` is `None`: keep existing auto-detection (CUDA → ROCm → CPU fallback).
- Populate `HostInfo` fields via the `sysinfo` crate: `os` (OS name + version), `cpu_model` (first logical CPU name), `ram_total_mib`, `ram_free_mib`.
- Add `sysinfo` dependency to `crates/anvilml-hardware/Cargo.toml`.
- Add integration tests in `lib.rs` covering: no-override auto-detect, force-cpu override, force-cuda override, force-rocm override, and host info field verification.

### Out of Scope
- Changes to `anvilml-core` types or config structs (no field renaming or struct changes).
- Changes to CUDA or ROCm detector implementation logic (P3-A3, P3-A4 already complete).
- Mock detector behavior changes (mock-hardware path remains unchanged — always uses MockDetector).
- Any changes to CI workflow files (.github/workflows/).
- Changes to other crates that depend on `anvilml-hardware`.

## Approach

1. **Add `sysinfo` dependency**: Append `sysinfo = "0.30"` to `[dependencies]` in `crates/anvilml-hardware/Cargo.toml`. Version 0.30 is stable and provides `System::new_all()`, `.total_memory()`, `.available_memory()`, and `.cpus()` APIs.

2. **Update `detect_all_devices()` signature**: Change from `pub fn detect_all_devices() -> HardwareInfo` to `pub fn detect_all_devices(override_config: Option<&HardwareOverrideConfig>) -> HardwareInfo`. The parameter is a borrowed reference to avoid cloning.

3. **Implement override priority logic** (non-mock cfg path):
   - If `override_config.is_some()`:
     - Extract `device_type` from the override.
     - If `Cuda`: instantiate `CudaDetector`, call `detect()`. If empty, return `AnvilError::InvalidGraph("forced CUDA but no CUDA devices found")`.
     - If `Rocm`: instantiate `RocmDetector`, call `detect()`. If empty, return `AnvilError::InvalidGraph("forced ROCm but no ROCm devices found")`.
     - If `Cpu`: use `CpuDetector` directly (always succeeds).
   - If `override_config.is_none()`:
     - Keep existing logic: CUDA → ROCm → CPU fallback.
   - After device detection, populate `HostInfo` using `sysinfo`.

4. **Populate `HostInfo`** (both mock and non-mock paths):
   - Create `sysinfo::System::new_all()`.
   - `os`: format as `{name} {version}` (e.g., "Linux 6.1.0").
   - `cpu_model`: first CPU from `.cpus()` via `.first().map(|c| c.brand())`.
   - `ram_total_mib`: `.total_memory() / 1024 / 1024` (sysinfo returns bytes, `HostInfo.ram_total_mib` is `u64`).
   - `ram_free_mib`: `.available_memory() / 1024 / 1024`.

5. **Mock path adjustment**: When `mock-hardware` feature is active, continue using `MockDetector` exclusively (override does not affect mock behavior — CI must remain deterministic). Populate `HostInfo` via `sysinfo` in the mock path as well.

6. **Add integration tests** in `lib.rs` module tests (cfg-gated by `mock-hardware`):
   - `test_no_override`: call with `None`, verify MockDetector device is returned, host fields populated.
   - `test_force_cpu_override`: call with `Some(HardwareOverrideConfig { device_type: Cpu, .. })`, verify CpuDetector device.
   - `test_force_cuda_override`: call with `Some(HardwareOverrideConfig { device_type: Cuda, .. })`, verify CpuDetector fallback (no CUDA on CI) or error.
   - `test_force_rocm_override`: call with `Some(HardwareOverrideConfig { device_type: Rocm, .. })`, verify CpuDetector fallback (no ROCm on CI).
   - `test_host_ram_total_gt_zero`: verify `ram_total_mib > 0` on the host machine.
   - `test_host_cpu_model_non_empty`: verify `cpu_model` is non-empty.

7. **Run tests**: Execute `cargo test -p anvilml-hardware --features mock-hardware` and verify ≥8 total tests pass with exit code 0.

## Files Affected

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | crates/anvilml-hardware/Cargo.toml | Add `sysinfo = "0.30"` to `[dependencies]`           |
| MODIFY   | crates/anvilml-hardware/src/lib.rs | Update `detect_all_devices()` signature, override logic, host info population, and integration tests |

## Tests

| Test ID / Name                        | File                              | Validates                                       |
|---------------------------------------|-----------------------------------|-------------------------------------------------|
| `detect_all_devices_returns_cpu_device` | crates/anvilml-hardware/src/lib.rs (existing) | No-regression: default detection returns CPU device |
| `detect_all_devices_host_fields_empty`  | crates/anvilml-hardware/src/lib.rs (existing) | No-regression: host fields are populated          |
| `detect_all_devices_inference_caps_cpu` | crates/anvilml-hardware/src/lib.rs (existing) | No-regression: inference caps for CPU             |
| `device_detector_trait_is_object_safe`  | crates/anvilml-hardware/src/lib.rs (existing) | No-regression: trait object safety                |
| `test_no_override_auto_detect`          | crates/anvilml-hardware/src/lib.rs (new)        | Override=None → auto-detect path with mock device |
| `test_force_cpu_override`               | crates/anvilml-hardware/src/lib.rs (new)        | Override=Cpu → CpuDetector returns CPU device     |
| `test_force_cuda_override_no_cuda`      | crates/anvilml-hardware/src/lib.rs (new)        | Override=Cuda with no CUDA hardware → error       |
| `test_force_rocm_override_no_rocm`      | crates/anvilml-hardware/src/lib.rs (new)        | Override=Rocm with no ROCm hardware → error       |
| `test_host_info_populated`              | crates/anvilml-hardware/src/lib.rs (new)        | HostInfo fields: os non-empty, cpu_model non-empty, ram_total_mib > 0 |
| `mock_detect_defaults_to_cpu`           | crates/anvilml-hardware/src/mock.rs (existing)  | No-regression: mock default CPU                   |
| `mock_detect_cuda`                      | crates/anvilml-hardware/src/mock.rs (existing)  | No-regression: mock CUDA detection                |
| `mock_detect_rocm`                      | crates/anvilml-hardware/src/mock.rs (existing)  | No-regression: mock ROCm detection                |
| `mock_refresh_vram`                     | crates/anvilml-hardware/src/mock.rs (existing)  | No-regression: mock VRAM refresh                  |

## CI Impact

No CI changes required. The `mock-hardware` feature flag is already declared in the CI matrix (`TASKS_PHASE003.md` §Phase Acceptance Criteria). Adding the `sysinfo` crate as a dependency does not require any CI workflow modifications — it is a pure Rust library with no platform-specific build steps beyond standard compilation.

## Risks and Mitigations

| Risk                                      | Likelihood | Impact | Mitigation                                              |
|-------------------------------------------|-----------|--------|---------------------------------------------------------|
| `sysinfo` crate API changes between minor versions | Medium     | High   | Pin to a specific version (0.30.x) in Cargo.toml; verify API at plan time |
| `detect_all_devices` signature change breaks downstream crates | Low       | Medium | Only `anvilml-hardware` is modified; other crates are not yet calling this function (P3-B1 is the first integration task) |
| Forced CUDA/ROCm detector returns empty on CI runner without GPU hardware | High      | Low    | Use `AnvilError::InvalidGraph` error variant as specified in TASKS_PHASE003.md; test this path explicitly       |
| `sysinfo::System::new_all()` panics or returns zero memory on some containers | Low       | Low    | Gracefully handle: default to 0 if values are missing; test on CI environment |

## Acceptance Criteria

- [ ] `crates/anvilml-hardware/Cargo.toml` includes `sysinfo = "0.30"` in `[dependencies]`
- [ ] `detect_all_devices()` accepts `Option<&HardwareOverrideConfig>` parameter
- [ ] Override logic: when override.device_type is Cuda, only CudaDetector runs; when Rocm, only RocmDetector runs; when Cpu, only CpuDetector runs
- [ ] No-override path: CUDA → ROCm → CPU fallback sequence preserved
- [ ] `HostInfo.os` is populated with OS name + version string (non-empty)
- [ ] `HostInfo.cpu_model` is populated with first logical CPU brand (non-empty)
- [ ] `HostInfo.ram_total_mib` and `ram_free_mib` are populated via `sysinfo`
- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with ≥8 total tests
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes
- [ ] `cargo fmt --all --check` passes
