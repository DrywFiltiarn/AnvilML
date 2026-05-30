# Plan Report: P3-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A4                                       |
| Phase       | 003 — Hardware Detection                    |
| Description | anvilml-hardware: ROCm detector via rocm-smi |
| Depends on  | P3-A1 (DeviceDetector trait, CpuDetector), P3-A3 (CUDA detector) |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T12:22:25Z                        |
| Attempt     | 1                                           |

## Objective

Implement the ROCm GPU detector in `crates/anvilml-hardware/src/rocm.rs`. The `RocmDetector` struct invokes `rocm-smi --showmeminfo vram --json` to enumerate AMD GPUs, parsing the JSON output into a `Vec<GpuDevice>` with `DeviceType::Rocm`. If `rocm-smi` is absent or fails, the detector returns an empty device list (not an error). Inference capabilities are derived from the graphics architecture: `fp16=true` always; `bf16=true` when `gfx_arch` from `HardwareOverrideConfig.hsa_override_gfx_version` starts with `gfx11` or higher (RDNA3+), defaulting to `bf16=false` for safety if no override is provided. Tests use a pure-parse helper function operating on fixture strings — no AMD GPU hardware is required.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/rocm.rs` with:
  - `RocmDetector` struct implementing `DeviceDetector`
  - `spawn_rocm_smi()` helper that runs `rocm-smi --showmeminfo vram --json` and returns stdout as `String`
  - `parse_rocm_smi_output(raw: &str) -> Vec<GpuDevice>` pure-parse function (extracted so tests can call it without spawning a process)
  - `RocmDetector::detect()` that calls `spawn_rocm_smi()`, delegates to `parse_rocm_smi_output()`, and returns `Ok(vec![])` on command-not-found or non-zero exit
  - `RocmDetector::refresh_vram(device_index)` that re-invokes `rocm-smi --showmeminfo vram --json`, parses the per-card VRAM values in bytes, converts to MiB (`/ 1024 / 1024`), and returns `(used_mib, total_mib)`
  - Inference caps computation: `fp16=true`; `bf16` and `flash_attention` gated on gfx_arch from override config (gfx11+ = RDNA3+)
- Modify `crates/anvilml-hardware/src/lib.rs` to:
  - Add `pub mod rocm;` module declaration (unconditionally, so it is always compiled)
  - Integrate `RocmDetector` into the non-mock detection path (called between CUDA and CPU fallback)
- Write unit tests in `rocm.rs` using the pure-parse helper with at least one fixture string representing a single ROCm GPU

### Out of Scope
- HardwareOverrideConfig wiring and HostInfo population (P3-B1)
- gfx_arch extraction from `rocminfo` or `lspci` (deferred; only override config is used in MVP)
- Any changes to CI workflow files
- Any changes to `anvilml-core` types
- Integration with the server or worker crates
- Windows-specific ROCm path (ROCm on Windows is not supported in the MVP)

## Approach

1. **Create `rocm.rs`** — Implement the detector module with the following structure:
   - Define `pub struct RocmDetector;`
   - Implement `DeviceDetector` trait: `detect()` and `refresh_vram()`
   - `spawn_rocm_smi()` uses `std::process::Command::new("rocm-smi")` with args `["--showmeminfo", "vram", "--json"]`. If the command is not found or exits non-zero, return `Err(AnvilError::ConfigLoad(...))` which callers convert to `Ok(vec![])`.
   - `parse_rocm_smi_output(raw: &str)` deserializes the JSON into a defensive structure. The `rocm-smi --json` output varies across ROCm versions, so we parse into `serde_json::Value` first, then navigate keys defensively (using `.get()` and `.as_object()`, defaulting to 0 for missing fields). Per-card VRAM is in bytes; convert to MiB by dividing by `1024 * 1024`. Extract per-device index, name/product, vram_total_bytes, vram_used_bytes.
   - `detect()` calls `spawn_rocm_smi()`, handles errors gracefully (returning `Ok(vec![])`), and passes stdout to `parse_rocm_smi_output()`.
   - `refresh_vram(device_index)` re-invokes the same JSON command, parses the result for the matching device index, and returns `(used_mib, total_mib)`.
   - `compute_inference_caps(gfx_arch: Option<&str>)` — `fp16=true` always; `bf16` and `flash_attention` are true if `gfx_arch` starts with `"gfx11"` or higher (e.g., "gfx1100", "gfx1101"); otherwise false. If no override is provided, default to `bf16=false`.

2. **Modify `lib.rs`** — Add `pub mod rocm;` alongside existing module declarations. The module is unconditionally compiled (not feature-gated), per the known constraints. Update `detect_all_devices()` in the non-mock path to call `RocmDetector` between CUDA and CPU fallback.

3. **Write tests** — Inside `rocm.rs`, add a `#[cfg(test)] mod tests` block with:
   - `test_parse_single_rocm_gpu()` — fixture: single GPU JSON output; validates all fields
   - `test_parse_empty_input()` — fixture: empty string or `{}`; validates `Vec::is_empty()`
   - `test_inference_caps_rdna3()` — gfx_arch "gfx1100" → bf16=true, flash_attention=true
   - `test_inference_caps_not_rdna3()` — gfx_arch "gfx1030" → bf16=false, flash_attention=false
   - `test_inference_caps_no_override()` — gfx_arch=None → bf16=false, flash_attention=false
   - `test_rocm_detector_is_send_sync()` — compile-time trait check

## Files Affected

| Action   | Path                                    | Description                                                    |
|----------|-----------------------------------------|----------------------------------------------------------------|
| CREATE   | crates/anvilml-hardware/src/rocm.rs     | RocmDetector struct, rocm-smi invocation, JSON parser, tests  |
| MODIFY   | crates/anvilml-hardware/src/lib.rs      | Add `pub mod rocm;` declaration; integrate into detect_all_devices non-mock path |

## Tests

| Test ID / Name                  | File                     | Validates                                    |
|---------------------------------|--------------------------|----------------------------------------------|
| `test_parse_single_rocm_gpu`    | `rocm.rs`                | Single-GPU JSON fixture → correct GpuDevice fields |
| `test_parse_empty_input`        | `rocm.rs`                | Empty / `{}` input → empty Vec                |
| `test_inference_caps_rdna3`     | `rocm.rs`                | gfx_arch "gfx1100" → bf16=true, flash_attention=true |
| `test_inference_caps_not_rdna3` | `rocm.rs`                | gfx_arch "gfx1030" → bf16=false, flash_attention=false |
| `test_inference_caps_no_override`| `rocm.rs`               | gfx_arch=None → bf16=false, flash_attention=false |
| `test_rocm_detector_is_send_sync`| `rocm.rs`               | RocmDetector implements Send + Sync            |

## CI Impact

No CI changes required. The ROCm detector is compiled into every build (not feature-gated), and tests use fixture strings — they do not require `rocm-smi` to be present in the CI environment. The existing CI matrix already runs `cargo test -p anvilml-hardware --features mock-hardware`; the new tests run under `-- rocm` filter without any feature flag, following the same pattern as CUDA tests.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation                                              |
|---------------------------|-----------|--------|---------------------------------------------------------|
| `rocm-smi --json` output structure varies across ROCm versions | Medium | High | Parse into `serde_json::Value` first, then navigate defensively with `.get()` and `.as_object()`, defaulting to 0 for missing fields — never abort on partial data |
| gfx_arch parsing edge case (e.g., "gfx1100" vs "gfx11") | Low | Low | Use `str::starts_with("gfx11")` or check the prefix numerically; this covers all gfx11x variants including gfx1100, gfx1101, gfx1102 |
| `serde_json` not declared as a dependency in `anvilml-hardware` | Low | Medium | Check Cargo.toml; if missing, add it as a regular dependency (not dev-only) since the parser needs it at runtime. Alternatively, parse JSON manually with string operations to avoid the dep |
| Per-device VRAM keys differ between ROCm minor versions | Medium | Medium | The parser iterates over all top-level device entries and matches by index; if a key is missing for a particular device, default VRAM to 0 with graceful degradation (not error) |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- rocm` exits 0 with all tests passing
- [ ] `rocm.rs` contains a `parse_rocm_smi_output(raw: &str) -> Vec<GpuDevice>` function callable without spawning a process
- [ ] `RocmDetector::detect()` returns `Ok(vec![])` when `rocm-smi` is not on PATH
- [ ] `RocmDetector::detect()` returns `Ok(vec![])` when `rocm-smi` exits non-zero
- [ ] Fixture test with single ROCm GPU validates index, name, vram_total_mib, vram_free_mib, device_type=Rocm
- [ ] Inference caps: fp16=true always; bf16 and flash_attention true when gfx_arch starts with "gfx11", false otherwise
- [ ] `pub mod rocm;` declared in `lib.rs`
- [ ] `RocmDetector` integrated into `detect_all_devices()` non-mock path between CUDA and CPU fallback
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` passes
- [ ] `cargo fmt --all --check` passes
