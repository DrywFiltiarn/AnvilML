# Implementation Report: P3-A2

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P3-A2                                       |
| Phase          | 003 — Hardware Detection                    |
| Description    | anvilml-hardware: mock detector driven by env vars |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T10:13:02Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the deterministic mock detector (`MockDetector`) used in all CI runs, gated behind the `mock-hardware` feature flag. The `MockDetector` reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) and returns a single deterministic `GpuDevice` with the specified values. When the `mock-hardware` feature is active, `detect_all_devices()` uses `MockDetector` exclusively, ensuring CI is fully hermetic without any real GPU hardware. Added four cfg-gated fixture tests in `mock.rs` covering default CPU detection, CUDA with custom VRAM, ROCm with custom architecture, and `refresh_vram`. Added `serial_test` as a dev-dependency to serialize env-var tests and avoid cross-test pollution.

## Files Changed

| Action   | Path                              | Description                                       |
|----------|-----------------------------------|---------------------------------------------------|
| CREATE   | crates/anvilml-hardware/src/mock.rs | MockDetector struct implementing DeviceDetector trait with env-var-driven config and 4 cfg-gated tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs  | Added `#[cfg(feature = "mock-hardware")] pub mod mock;`, updated `detect_all_devices()` to use MockDetector when feature is active, made lib-level tests feature-aware |
| MODIFY   | crates/anvilml-hardware/Cargo.toml  | Added `serial_test = "3"` as dev-dependency       |

## Test Results

### Default build (no features)

```
running 8 tests
test cpu::tests::cpu_detect_returns_single_device ... ok
test cpu::tests::cpu_detector_is_send_sync ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram_returns_zeros ... ok
test tests::detect_all_devices_host_fields_empty ... ok
test tests::detect_all_devices_inference_caps_cpu ... ok
test tests::detect_all_devices_returns_cpu_device ... ok
test tests::device_detector_trait_is_object_safe ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### With `mock-hardware` feature

```
running 12 tests
test cpu::tests::cpu_detect_returns_single_device ... ok
test cpu::tests::cpu_detector_is_send_sync ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram_returns_zeros ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_defaults_to_cpu ... ok
test mock::tests::mock_detect_rocm ... ok
test mock::tests::mock_refresh_vram ... ok
test tests::detect_all_devices_host_fields_empty ... ok
test tests::detect_all_devices_inference_caps_cpu ... ok
test tests::detect_all_devices_returns_cpu_device ... ok
test tests::device_detector_trait_is_object_safe ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Full workspace build (no features)

```
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s   [anvilml-core]
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s    [anvilml-hardware]
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s   [anvilml-ipc]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s    [anvilml-registry]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s    [anvilml-scheduler]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s    [anvilml-server]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s    [anvilml-worker]
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P3-A2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  crates/anvilml-hardware/Cargo.toml
M  crates/anvilml-hardware/src/lib.rs
A  crates/anvilml-hardware/src/mock.rs
```

## Acceptance Criteria — Verification

| Criterion                 | Status | Evidence                        |
|---------------------------|--------|---------------------------------|
| MockDetector implements DeviceDetector trait | PASS | `MockDetector` compiles and implements all required methods (`detect`, `refresh_vram`) |
| ANVILML_MOCK_DEVICE_TYPE defaults to "cpu" → DeviceType::Cpu | PASS | Test `mock_detect_defaults_to_cpu` passes with no env vars set |
| ANVILML_MOCK_DEVICE_TYPE=cuda → DeviceType::Cuda with correct VRAM | PASS | Test `mock_detect_cuda` sets env var to "cuda", verifies 16384 MiB VRAM |
| ANVILML_MOCK_DEVICE_TYPE=rocm + custom VRAM → DeviceType::Rocm | PASS | Test `mock_detect_rocm` sets env vars, verifies 32768 MiB VRAM and gfx1030 arch in name |
| detect_all_devices() uses MockDetector when mock-hardware feature is active | PASS | Test `detect_all_devices_returns_cpu_device` asserts "Mock CPU" name with feature, "CPU" without |
| serial_test added as dev-dependency | PASS | `Cargo.toml` lists `serial_test = "3"` under `[dev-dependencies]` |
| All existing tests still pass (no regressions) | PASS | Full workspace test suite: 82 tests passed, 0 failed |
