# Plan Report: P5-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A1                                       |
| Phase       | 5 — Hardware Detection: Orchestration       |
| Description | anvilml-hardware: hardware_override config short-circuit |
| Depends on  | P4-A6                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T07:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` in `crates/anvilml-hardware/src/detect.rs` with the hardware_override short-circuit as step 1 of ANVILML_DESIGN.md §6.4's six-step priority chain. When `cfg.hardware_override` is `Some`, synthesize exactly one `GpuDevice` and return `Ok(HardwareInfo{...})` immediately, skipping all other detectors. This establishes the function signature and entry point for subsequent tasks (P5-A2, P5-A3) to extend with the mock/Vulkan/fallback/CPU chain.

## Scope

### In Scope
- Implement `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` in `crates/anvilml-hardware/src/detect.rs`
- Override short-circuit: if `cfg.hardware_override.is_some()`, synthesize one `GpuDevice` from its `device_type`/`vram_total_mib` fields with `enumeration_source=EnumerationSource::Override`, `capabilities_source=CapabilitySource::Fallback`, and return `Ok(HardwareInfo{host, gpus, inference_caps})` immediately
- Parse the override's `device_type` string (`"cuda"`, `"rocm"`, `"cpu"`) into `DeviceType` enum; fall back to `DeviceType::Cpu` for any unrecognized value (with a WARN log)
- When `cfg.hardware_override` is `None`, return `Err(AnvilError::Internal("detect_all_devices chain not yet implemented"))` — the full detection chain is deferred to P5-A2
- Create `crates/anvilml-hardware/tests/detect_tests.rs` with ≥2 tests
- Bump `anvilml-hardware` crate patch version from `0.1.5` to `0.1.6`

### Out of Scope
- Mock detector path — deferred to P5-A2 (`defers_to: P5-A2`)
- Vulkan detector path — deferred to P5-A2 (`defers_to: P5-A2`)
- Platform fallback (DXGI on Windows, sysfs on Linux) — deferred to P5-A2 (`defers_to: P5-A2`)
- CPU fallback append and `HardwareInfo` assembly with `inference_caps` union — deferred to P5-A2 (`defers_to: P5-A2`)
- `SqlitePool` parameter for PCI-ID capability hint lookup — deferred until `anvilml-registry`'s `DeviceCapabilityStore` exists in a later phase
- `lib.rs` re-export of `detect_all_devices` — deferred to P5-A4 (`defers_to: P5-A2` transitively)
- `hw-probe` CLI subcommand — deferred to P5-A5 (`defers_to: P5-A2` transitively)

## Existing Codebase Assessment

The `anvilml-hardware` crate already has five working `DeviceDetector` implementations from Phase 4: `CpuDetector` (always returns one CPU device), `MockDetector` (env-var driven, `mock-hardware` feature-gated), `VulkanDetector` (headless Vulkan enumeration), `DxgiDetector` (Windows-only), and `SysfsPciDetector` (Linux-only). The shared `DeviceDetector` trait is declared in `detect.rs` (lines 14–27) with `detect()` and `refresh_vram()` methods.

All domain types needed for this task already exist in `anvilml-core`: `ServerConfig` (with `hardware_override: Option<HardwareOverrideConfig>`), `HardwareInfo`, `GpuDevice`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`, and `HostInfo`. The types have the correct derives (`Clone`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`, `ToSchema`) and field names match the design spec exactly.

The established patterns to follow: (1) `///` doc comments on every `pub` item describing what it does, arguments, and return/error types; (2) `#[serial]` annotation on tests that mutate process-global state (env vars); (3) capture-and-restore pattern for env vars with unconditional final-step restore outside any conditional block; (4) test files in `crates/{name}/tests/` as separate test crates using the crate's public API; (5) `tracing::warn!()` for non-obvious fallback decisions.

No gap between design doc and current source affects this task's approach: the `ServerConfig::hardware_override` field exists, all domain types are present, and the `DeviceDetector` trait provides the pattern for how detectors produce `GpuDevice` values.

## Resolved Dependencies

This task introduces no new external crates. All types consumed (`ServerConfig`, `HardwareInfo`, `GpuDevice`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`, `HostInfo`, `AnvilError`) are re-exported from the workspace's `anvilml-core` path dependency, which already exists in `Cargo.toml`. The `serial_test` dev-dependency is already declared in `anvilml-hardware/Cargo.toml`.

| Type   | Name     | Version verified | MCP source | Feature flags confirmed |
|--------|----------|-----------------|------------|------------------------|
| crate  | serial_test | 3.5.0        | Cargo.toml (workspace lock) | n/a |

## Approach

1. **Add `detect_all_devices` function to `detect.rs`** (after the `DeviceDetector` trait, ~line 29). The function signature is:

   ```rust
   pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>
   ```

   Inside the function body:
   - Check `cfg.hardware_override`. If `Some(override_cfg)`:
     - Parse `override_cfg.device_type` string into `DeviceType` — match `"cuda" => DeviceType::Cuda`, `"rocm" => DeviceType::Rocm`, `"cpu" => DeviceType::Cpu`; for any other value, log `tracing::warn!(device_type = %override_cfg.device_type, "unrecognized hardware_override device_type, defaulting to Cpu")` and use `DeviceType::Cpu`.
     - Build a single `GpuDevice` with: `index = 0`, `name` derived from the parsed `DeviceType` (e.g., `"CUDA"` for Cuda, `"ROCm"` for Rocm, `"CPU"` for Cpu), `device_type` = parsed type, `vram_total_mib` = `override_cfg.vram_total_mib`, `vram_free_mib` = `override_cfg.vram_total_mib` (free = total for override), `driver_version = "override"`, `pci_vendor_id = 0`, `pci_device_id = 0`, `arch = None`, `caps = InferenceCaps::default()`, `enumeration_source = EnumerationSource::Override`, `capabilities_source = CapabilitySource::Fallback`.
     - Build `HardwareInfo` with: `host = HostInfo { hostname, os }` where `hostname` is from `std::env::var("HOSTNAME").or_else(|| std::env::var("COMPUTERNAME").or_else(|| Ok("unknown".into())))` and `os` is from `std::env::consts::OS`, `gpus = vec![device]`, `inference_caps = InferenceCaps::default()` (single device, so union = that device's caps).
     - Return `Ok(hardware_info)`. This short-circuits the entire chain.
   - If `cfg.hardware_override` is `None`:
     - Return `Err(AnvilError::Internal("detect_all_devices chain not yet implemented — mock/Vulkan/fallback/CPU chain deferred to P5-A2"))`.

   Rationale for returning `Err` when override is absent: this is the first step of a multi-step chain. The full chain is not yet implemented; P5-A2 will extend this function to add the remaining steps. Returning `Err` with a clear message makes the incomplete state explicit and testable.

2. **Add `///` doc comment** to the new function describing its purpose, the override short-circuit behavior, and noting that the full chain is deferred.

3. **Add inline comment** at the override-synthesized device explaining that it satisfies the design's requirement that override always wins unconditionally before any detector runs.

4. **Create `crates/anvilml-hardware/tests/detect_tests.rs`** with two tests:
   - `test_override_present_returns_device` — constructs a `ServerConfig` with `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "cuda".into(), vram_total_mib: 24576 })`, calls `detect_all_devices(&cfg).await`, verifies the returned `HardwareInfo.gpus` has exactly one device with `device_type == Cuda`, `vram_total_mib == 24576`, `enumeration_source == Override`, `capabilities_source == Fallback`.
   - `test_override_absent_returns_err` — constructs a `ServerConfig` with `hardware_override` set to `None` (the default), calls `detect_all_devices(&cfg).await`, verifies the result is `Err` and the error message contains `"not yet implemented"`.

5. **Bump `anvilml-hardware` crate version** from `0.1.5` to `0.1.6` in `crates/anvilml-hardware/Cargo.toml`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `detect_all_devices` | `anvilml_hardware::detect` | `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` |

No new `pub struct`, `pub enum`, or `pub trait` items. No new `pub use` re-exports.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-hardware/src/detect.rs` | Add `pub async fn detect_all_devices()` with override short-circuit logic |
| CREATE | `crates/anvilml-hardware/tests/detect_tests.rs` | Integration tests: override-present and override-absent |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version `0.1.5` → `0.1.6` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/detect_tests.rs` | `test_override_present_returns_device` | When `hardware_override` is `Some`, `detect_all_devices` returns `Ok(HardwareInfo)` with exactly one synthesized `GpuDevice` matching the override config fields | `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })` | `cfg` with cuda override | `Ok` with `gpus.len() == 1`, `gpus[0].device_type == Cuda`, `gpus[0].vram_total_mib == 24576`, `gpus[0].enumeration_source == Override`, `gpus[0].capabilities_source == Fallback` | `cargo test -p anvilml-hardware --test detect_tests test_override_present_returns_device` exits 0 |
| `crates/anvilml-hardware/tests/detect_tests.rs` | `test_override_absent_returns_err` | When `hardware_override` is `None`, `detect_all_devices` returns `Err(Internal(...))` with a message indicating the chain is not yet implemented | `ServerConfig` with default (no override) | default `cfg` | `Err` with message containing `"not yet implemented"` | `cargo test -p anvilml-hardware --test detect_tests test_override_absent_returns_err` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-hardware/tests/` which is picked up by `cargo test --workspace --features mock-hardware` (the standard CI Rust test command). No new file types, gates, or test modules that would alter CI behavior.

## Platform Considerations

None identified. The `detect_all_devices` function and its override short-circuit are platform-neutral — they read from `ServerConfig` and synthesize a `GpuDevice` without any platform-specific APIs. The `hostname` resolution uses `HOSTNAME` (Unix) and `COMPUTERNAME` (Windows) env vars, both of which are available on all platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ServerConfig::hardware_override.device_type` is a `String` — parsing it to `DeviceType` requires a match statement; an unrecognized string silently falls back to `Cpu` without the caller knowing. | Low | Medium | Log a `tracing::warn!()` at parse time naming the unrecognized value. The WARN log satisfies §11.1's requirement that non-obvious fallbacks are instrumented. |
| The `Err(Internal(...))` return when override is absent creates a breaking API contract: callers expecting `Ok` will get `Err` until P5-A2 lands. | Medium | Low | The error message explicitly names P5-A2 as the deferral target. P5-A2 is a direct prereq (P5-A1 → P5-A2) so the chain will be completed in order. This is the intended interim state. |
| `detect_all_devices` is `pub` but not yet re-exported from `lib.rs` (deferred to P5-A4). External crates cannot import it until P5-A4. | Low | Low | This is by design — P5-A4 explicitly defers the re-export. The function is `pub` within the crate for test access and for P5-A4's re-export. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --test detect_tests` exits 0 (≥2 tests)
- [ ] `cargo clippy -p anvilml-hardware -- -D warnings` exits 0
- [ ] `grep '^version' crates/anvilml-hardware/Cargo.toml` contains `version = "0.1.6"`
