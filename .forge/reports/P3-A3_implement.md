# Implementation Report: P3-A3

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P3-A3                                       |
| Phase          | 003 — Hardware Detection                    |
| Description    | anvilml-hardware: CUDA detector via nvidia-smi |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T14:15:00Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the CUDA GPU detector in `crates/anvilml-hardware/src/cuda.rs`. The `CudaDetector` struct invokes `nvidia-smi` with a fixed CSV query (`--query-gpu=index,name,vram_total,driver_version --format=csv,noheader,nounits`) to enumerate NVIDIA GPUs. Each line is parsed into a `GpuDevice { device_type: Cuda }`. If `nvidia-smi` is absent or fails, the detector returns an empty device list (not an error). Inference capabilities are derived from the driver version: `fp16=true` always; `bf16=true` and `flash_attention=true` when driver major ≥ 525. The pure-parse helper `parse_nvidia_smi_output()` operates on fixture strings so tests require no GPU hardware.

## Files Changed

| Action   | Path                              | Description |
|----------|-----------------------------------|-------------|
| CREATE   | crates/anvilml-hardware/src/cuda.rs | CUDA detector with CudaDetector struct, spawn_nvidia_smi(), parse_nvidia_smi_output(), compute_inference_caps(), refresh_vram(), and 15 unit tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs  | Added `pub mod cuda;`, updated detect_all_devices() to try CUDA first then fall back to CPU, removed needless return in mock path |

## Test Results

```running unittests src/lib.rs (target/debug/deps/anvilml_hardware-b8a2c695edc7fc1f)

running 27 tests
test cpu::tests::cpu_detector_is_send_sync ... ok
test cpu::tests::cpu_detect_returns_single_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram_returns_zeros ... ok
test cuda::tests::cuda_detector_is_send_sync ... ok
test cuda::tests::inference_caps_bf16_flash_at_threshold_525 ... ok
test cuda::tests::inference_caps_bf16_flash_with_new_driver ... ok
test cuda::tests::inference_caps_no_bf16_flash_at_524 ... ok
test cuda::tests::inference_caps_no_bf16_flash_with_old_driver ... ok
test cuda::tests::inference_caps_unparseable_driver_version ... ok
test cuda::tests::parse_blank_lines_skipped ... ok
test cuda::tests::parse_dual_gpu ... ok
test cuda::tests::parse_empty_input ... ok
test cuda::tests::parse_single_gpu ... ok
test cuda::tests::parse_skips_malformed_lines ... ok
test cuda::tests::parse_vram_mib_graceful_fallback ... ok
test cuda::tests::parse_vram_mib_strips_unit ... ok
test tests::device_detector_trait_is_object_safe ... ok
test cuda::tests::cuda_detect_empty_when_nvidia_smi_absent ... ok
test cuda::tests::cuda_refresh_vram_error_when_nvidia_smi_absent ... ok
test tests::detect_all_devices_host_fields_empty ... ok
test tests::detect_all_devices_inference_caps_cpu ... ok
test tests::detect_all_devices_returns_cpu_device ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_rocm ... ok
test mock::tests::mock_detect_defaults_to_cpu ... ok
test mock::tests::mock_refresh_vram ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P3-A3_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
A  crates/anvilml-hardware/src/cuda.rs
M  crates/anvilml-hardware/src/lib.rs
M  crates/anvilml-hardware/src/mock.rs
```

## Acceptance Criteria — Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| `crates/anvilml-hardware/src/cuda.rs` exists with `CudaDetector` struct | PASS | File created, compiles successfully |
| `CudaDetector` implements `DeviceDetector` trait (`detect()` and `refresh_vram()`) | PASS | Clippy passes, tests pass |
| `spawn_nvidia_smi()` helper runs `nvidia-smi` CLI and returns stdout as `String` | PASS | Implemented and tested |
| `parse_nvidia_smi_output(raw: &str) -> Vec<GpuDevice>` pure-parse function exists | PASS | Called from tests with fixture strings |
| `CudaDetector::detect()` returns `Ok(vec![])` when `nvidia-smi` absent | PASS | Test `cuda_detect_empty_when_nvidia_smi_absent` passes |
| `CudaDetector::refresh_vram()` re-invokes `nvidia-smi` for single device index | PASS | Implemented with `--id={index}` query |
| Inference caps: `fp16=true`, `bf16/flash_attention` gated on driver major ≥ 525 | PASS | Four threshold tests pass (535→true, 470→false, 525→true, 524→false) |
| `pub mod cuda;` added to `lib.rs` | PASS | Module declaration present |
| `detect_all_devices()` integrated CUDA detector before CPU fallback | PASS | Non-mock path tries CUDA first, falls back to CPU |
| Unit tests: single-GPU fixture | PASS | `parse_single_gpu` passes |
| Unit tests: dual-GPU fixture | PASS | `parse_dual_gpu` passes |
| Unit tests: driver-version gating for bf16/flash_attention | PASS | `inference_caps_bf16_flash_with_new_driver`, `inference_caps_no_bf16_flash_with_old_driver`, `inference_caps_bf16_flash_at_threshold_525`, `inference_caps_no_bf16_flash_at_524` pass |
| Unit tests: empty-input handling | PASS | `parse_empty_input` passes |
| `cargo fmt --all` passes | PASS | Formatted successfully |
| `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` passes | PASS | Zero warnings |
| Full workspace test suite passes (0 failures) | PASS | 102 tests passed, 0 failed |
