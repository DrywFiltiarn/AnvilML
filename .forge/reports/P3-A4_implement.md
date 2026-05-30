# Implementation Report: P3-A4

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P3-A4                                       |
| Phase          | 003 — Hardware Detection                    |
| Description    | anvilml-hardware: ROCm detector via rocm-smi |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T13:00:00Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the AMD ROCm GPU detector in `crates/anvilml-hardware/src/rocm.rs`. The `RocmDetector` struct invokes `rocm-smi --showmeminfo vram --json` to enumerate AMD GPUs, parsing the JSON output into a `Vec<GpuDevice>` with `device_type: Rocm`. If `rocm-smi` is absent or fails, the detector returns an empty device list (not an error). Inference capabilities are derived from the graphics architecture: `fp16=true` always; `bf16` and `flash_attention` are gated on `hsa_override_gfx_version` major version >= 11 (RDNA3+), defaulting to disabled for safety if no override is provided. The pure-parse helper `parse_rocm_smi_output()` enables fixture-based tests without requiring AMD GPU hardware.

## Files Changed

| Action   | Path                              | Description |
|----------|-----------------------------------|-------------|
| CREATE   | crates/anvilml-hardware/src/rocm.rs | ROCm GPU detector with spawn, parse, detect, refresh_vram, and inference caps computation + 14 unit tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs | Added `pub mod rocm;` module declaration; integrated `RocmDetector` between CUDA and CPU in `detect_all_devices()` detection pipeline |
| MODIFY   | crates/anvilml-hardware/Cargo.toml | Added `serde_json = "1"` dependency for JSON parsing |
| MODIFY   | crates/anvilml-ipc/src/framing.rs | Fixed pre-existing clippy warnings (unused import, dead code) to enable workspace compilation |

## Test Results

```
running 52 tests
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 41 tests
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P3-A4_plan.md
M  Cargo.lock
M  crates/anvilml-hardware/Cargo.toml
M  crates/anvilml-hardware/src/lib.rs
A  crates/anvilml-hardware/src/rocm.rs
M  crates/anvilml-ipc/src/framing.rs
```

## Acceptance Criteria — Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| `rocm.rs` exists with `RocmDetector` struct implementing `DeviceDetector` | PASS | File created at `crates/anvilml-hardware/src/rocm.rs` |
| `spawn_rocm_smi()` helper runs `rocm-smi --showmeminfo vram --json` | PASS | Implemented; returns stdout as String, Err on not-found or non-zero exit |
| `parse_rocm_smi_output(raw: &str) -> Vec<GpuDevice>` pure-parse function | PASS | Extracted; parses JSON `gpu[]` array with bytes→MiB conversion |
| `RocmDetector::detect()` returns `Ok(vec![])` on command-not-found or non-zero exit | PASS | Uses `spawn_rocm_smi().unwrap_or_default()` then delegates to parse |
| `RocmDetector::refresh_vram(device_index)` returns `(used_mib, total_mib)` | PASS | Re-invokes rocm-smi, parses per-card VRAM, converts bytes→MiB |
| Inference caps: `fp16=true`; `bf16`/`flash_attention` gated on gfx11+ | PASS | `compute_inference_caps()` checks major version >= 11; defaults to false |
| `pub mod rocm;` added to `lib.rs` (unconditional) | PASS | Module declared after `cuda` in `src/lib.rs` |
| `RocmDetector` integrated between CUDA and CPU in detection pipeline | PASS | `detect_all_devices()` tries CUDA → ROCm → CPU |
| Unit tests with fixture strings (no AMD GPU hardware required) | PASS | 14 tests pass: parse_single_gpu, parse_dual_gpu, parse_empty_input, parse_malformed_json, parse_missing_gpu_key, inference_caps_gfx11_rdna3, inference_caps_gfx11_variant, inference_caps_gfx10_cdna2, inference_caps_gfx12_rdna4, inference_caps_no_override, inference_caps_unparseable_gfx, rocm_detect_empty_when_rocm_smi_absent, rocm_refresh_vram_error_when_rocm_smi_absent, rocm_detector_is_send_sync |
| `cargo fmt --all` passes | PASS | Ran successfully with no output (already formatted) |
| `cargo clippy --workspace --features mock-hardware -- -D warnings` passes | PASS | Zero warnings |
| Full workspace test suite passes with `mock-hardware` feature | PASS | 115 tests, 0 failures |
| Full workspace test suite passes without features | PASS | 93 tests, 0 failures |
