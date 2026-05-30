# Plan Report: P3-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A3                                       |
| Phase       | 003 — Hardware Detection                    |
| Description | anvilml-hardware: CUDA detector via nvidia-smi |
| Depends on  | P3-A1 (DeviceDetector trait, CpuDetector)   |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T10:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the CUDA GPU detector in `crates/anvilml-hardware/src/cuda.rs`. The `CudaDetector` struct invokes `nvidia-smi` with a fixed CSV query to enumerate NVIDIA GPUs, parsing each line into a `GpuDevice` with `DeviceType::Cuda`. If `nvidia-smi` is absent or fails, the detector returns an empty device list (not an error). Inference capabilities are derived from the driver version: `fp16=true` always, `bf16=true` and `flash_attention=true` when driver major ≥ 525. Tests use a pure-parse helper function operating on fixture strings — no GPU hardware is required.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/cuda.rs` with:
  - `CudaDetector` struct implementing `DeviceDetector`
  - `spawn_nvidia_smi()` helper that runs the `nvidia-smi` CLI and returns stdout as `String`
  - `parse_nvidia_smi_output(raw: &str) -> Vec<GpuDevice>` pure-parse function (extracted so tests can call it without spawning a process)
  - `CudaDetector::detect()` that calls `spawn_nvidia_smi()`, delegates to `parse_nvidia_smi_output()`, and returns `Ok(vec![])` on command-not-found or non-zero exit
  - `CudaDetector::refresh_vram()` that re-invokes the same `nvidia-smi` query for a single device index
  - Inference caps computation: `fp16=true`; `bf16` and `flash_attention` gated on driver_version major ≥ 525
- Modify `crates/anvilml-hardware/src/lib.rs` to:
  - Add `pub mod cuda;` module declaration (unconditionally, so it is always compiled)
  - Update `detect_all_devices()` doc comment to reference the CUDA detector
  - Integrate `CudaDetector` into the non-mock detection path (called before CPU fallback)
- Write unit tests in `cuda.rs` using the pure-parse helper with at least two fixture strings: a single-GPU case and a dual-GPU case, covering field extraction, driver-version gating for bf16/flash_attention, and empty-input handling

### Out of Scope
- ROCm detector (P3-A4)
- HardwareOverrideConfig wiring and HostInfo population (P3-B1)
- Any changes to CI workflow files
- Any changes to `anvilml-core` types
- Integration with the server or worker crates
- Windows-specific nvidia-smi path differences (handled by graceful-degradation: command-not-found → empty list)

## Approach

1. **Create `cuda.rs`** — Implement the detector module with the following structure:
   - Define `pub struct CudaDetector;`
   - Implement `DeviceDetector` trait: `detect()` and `refresh_vram()`
   - `detect()` calls `spawn_nvidia_smi()`, which uses `std::process::Command::new("nvidia-smi")` with the exact flags from the task spec. If the command is not found or exits non-zero, return `Ok(vec![])`. On success, pass stdout to `parse_nvidia_smi_output()`.
   - `parse_nvidia_smi_output(raw: &str)` splits on newlines, trims whitespace, skips empty lines, then for each line splits on `,` and maps fields: index (u32), name (trimmed String), memory.total (u32), memory.free (u32), driver_version (String). Builds `Vec<GpuDevice>` with `device_type = DeviceType::Cuda`.
   - Compute `InferenceCaps` per device: `fp16=true`; parse `driver_version` to extract major number; if major ≥ 525, set `bf16=true` and `flash_attention=true`, else both false.
   - `refresh_vram(device_index)` runs the same nvidia-smi command, parses the single matching device line, returns `(total_mib - free_mib, total_mib)`. If no match, returns `(0, 0)`.

2. **Modify `lib.rs`** — Add `pub mod cuda;` at the top (before `pub use anvilml_core::*;` or alongside existing module declarations). The module is unconditionally compiled (not feature-gated) so it is always linted and tested, per the known constraints.

3. **Write tests** — Inside `cuda.rs`, add a `#[cfg(test)] mod tests` block with:
   - `test_parse_single_gpu()` — fixture: one GPU line; validates all fields
   - `test_parse_dual_gpu()` — fixture: two GPU lines; validates both devices have correct indices
   - `test_parse_empty_input()` — fixture: empty string; validates `Vec::is_empty()`
   - `test_inference_caps_high_driver()` — fixture with driver "550.123.01"; validates bf16=true, flash_attention=true
   - `test_inference_caps_low_driver()` — fixture with driver "470.00.00"; validates bf16=false, flash_attention=false
   - `test_cudadetector_is_send_sync()` — compile-time trait check

## Files Affected

| Action   | Path                                    | Description                                                   |
|----------|-----------------------------------------|---------------------------------------------------------------|
| CREATE   | crates/anvilml-hardware/src/cuda.rs     | CudaDetector struct, nvidia-smi invocation, CSV parser, tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs      | Add `pub mod cuda;` declaration; update detect_all_devices doc |

## Tests

| Test ID / Name                  | File                     | Validates                                    |
|---------------------------------|--------------------------|----------------------------------------------|
| `test_parse_single_gpu`         | `cuda.rs`                | Single-GPU CSV line → correct GpuDevice fields |
| `test_parse_dual_gpu`           | `cuda.rs`                | Two-GPU CSV lines → correct indices and data   |
| `test_parse_empty_input`        | `cuda.rs`                | Empty string input → empty Vec                 |
| `test_inference_caps_high_driver`| `cuda.rs`               | Driver ≥ 525 → bf16=true, flash_attention=true |
| `test_inference_caps_low_driver` | `cuda.rs`               | Driver < 525 → bf16=false, flash_attention=false |
| `test_cudadetector_is_send_sync`| `cuda.rs`                | CudaDetector implements Send + Sync            |

## CI Impact

No CI changes required. The CUDA detector is compiled into every build (not feature-gated), and tests use fixture strings — they do not require `nvidia-smi` to be present in the CI environment. The existing CI matrix already runs `cargo test -p anvilml-hardware --features mock-hardware`; the new tests run under `-- cuda` filter without any feature flag.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation                                              |
|---------------------------|-----------|--------|---------------------------------------------------------|
| `nvidia-smi` CSV format varies across driver versions | Low | Medium | Parser is defensive: trims whitespace, handles optional quotes; tests cover clean fixture strings only |
| Driver version parsing edge case (e.g., "550.123.01" vs "55") | Low | Low | Parse only the first segment before `.` as major version; use `str::split_once('.')` then `parse::<u32>()` with unwrap_or(0) |
| Whitespace in nvidia-smi output lines (e.g., leading spaces from CSV alignment) | Medium | Low | Trim each line and each field before parsing; this is standard CSV cleaning |
| `std::process::Command` error handling — IoError vs NonZeroExitCode distinction | Low | Low | Match on both `Err(e)` (not found) and `Ok(result) if !result.status.success()` → return `Ok(vec![])` |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- cuda` exits 0 with all tests passing
- [ ] `cuda.rs` contains a `parse_nvidia_smi_output(raw: &str) -> Vec<GpuDevice>` function callable without spawning a process
- [ ] `CudaDetector::detect()` returns `Ok(vec![])` when `nvidia-smi` is not on PATH
- [ ] `CudaDetector::detect()` returns `Ok(vec![])` when `nvidia-smi` exits non-zero
- [ ] Fixture test with single GPU validates index, name, vram_total_mib, vram_free_mib, driver_version, device_type=Cuda
- [ ] Fixture test with dual GPUs validates both devices have correct sequential indices
- [ ] Inference caps: fp16=true always; bf16 and flash_attention true when driver major ≥ 525, false otherwise
- [ ] `pub mod cuda;` declared in `lib.rs`
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` passes
- [ ] `cargo fmt --all --check` passes
