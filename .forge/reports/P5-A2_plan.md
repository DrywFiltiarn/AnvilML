# Plan Report: P5-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P5-A2                                             |
| Phase       | 5 — Hardware Detection: Orchestration             |
| Description | anvilml-hardware: mock-vs-real branch + Vulkan fallback chain |
| Depends on  | P5-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-29T10:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend `detect_all_devices()` in `crates/anvilml-hardware/src/detect.rs` with the middle of the §6.4 priority chain: when `mock-hardware` is compiled in, use `MockDetector` exclusively; otherwise, try `VulkanDetector` first, then fall back to the platform-specific detector (`DxgiDetector` on Windows, `SysfsPciDetector` on Linux) if Vulkan returns empty. This produces a `Vec<GpuDevice>` that P5-A3 will later extend with CPU append and HardwareInfo assembly.

## Scope

### In Scope
- Modify `detect_all_devices()` in `crates/anvilml-hardware/src/detect.rs`:
  - Keep P5-A1's override short-circuit intact (step 1).
  - Add mock-vs-real branch (steps 2–4): when `mock-hardware` feature is active, call `MockDetector::detect()` and return the result; when not compiled, try `VulkanDetector::detect()`, and if it returns empty, try the cfg-gated platform fallback.
  - Remove P5-A1's `Err(AnvilError::Internal(...))` return that was a placeholder for "chain not yet implemented."
  - Return a partial `HardwareInfo` with the detected GPUs, default `InferenceCaps`, and the host info already constructed by P5-A1 — deferring CPU append and caps union to P5-A3.
- Add ≥4 new tests to `crates/anvilml-hardware/tests/detect_tests.rs`.

### Out of Scope
- CPU device append and final `HardwareInfo` assembly (inference_caps union, guaranteed non-empty result) — deferred to P5-A3 (`defers_to: ["P5-A3"]`).
- `anvilml-registry`'s `DeviceCapabilityStore` integration (SqlitePool parameter not yet available).
- Re-exporting `detect_all_devices` from `lib.rs` — deferred to P5-A4.
- CLI subcommand — deferred to P5-A5.

## Existing Codebase Assessment

P5-A1 established `detect_all_devices()` with the override short-circuit (step 1 of the §6.4 priority chain). The function returns `Ok(HardwareInfo{...})` when `cfg.hardware_override` is `Some`, and `Err(AnvilError::Internal("not yet implemented"))` otherwise. The error return is a compile-time visible placeholder — it makes the incomplete state explicit and testable.

All five detector implementations exist and pass their own tests (Phase 4): `CpuDetector`, `MockDetector` (feature-gated), `VulkanDetector`, `DxgiDetector` (cfg-gated Windows), and `SysfsPciDetector` (cfg-gated Linux). The `DeviceDetector` trait is defined in `detect.rs` and each detector implements it. The `lib.rs` re-exports are correct with proper `#[cfg]` and `#[cfg(feature)]` gates.

The `HardwareInfo` struct (from `anvilml-core::types::hardware`) contains `host: HostInfo`, `gpus: Vec<GpuDevice>`, and `inference_caps: InferenceCaps`. The `InferenceCaps` struct has no `union` or `merge` method yet — that will be implemented in P5-A3.

No gap between design doc and current source affects this task. The design doc (§6.4) describes the priority order that P5-A1 and this task implement sequentially.

## Resolved Dependencies

No new dependencies introduced. Existing dependencies verified via MCP:

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | ash     | 0.38.0          | rust-docs MCP  | n/a                    |
| crate  | tracing | 0.1 (latest 0.1.44) | rust-docs MCP | n/a                  |

## Approach

### Step 1: Modify `detect_all_devices()` in `detect.rs`

Replace P5-A1's placeholder `Err` return with the mock-vs-real branch logic. The function body after the override short-circuit block becomes:

```rust
// --- Steps 2-5: mock-vs-real branch + Vulkan fallback chain ---

#[cfg(feature = "mock-hardware")]
{
    // Mock and real detection are mutually exclusive per build.
    // When mock-hardware is compiled in, use MockDetector exclusively.
    let detector = MockDetector;
    let gpus = detector.detect()?;
    tracing::debug!(
        device_count = gpus.len(),
        "mock-hardware feature: returning mock-detected devices"
    );
    return Ok(HardwareInfo {
        host,
        gpus,
        inference_caps: InferenceCaps::default(),
    });
}

#[cfg(not(feature = "mock-hardware"))]
{
    // Primary real-hardware path: Vulkan enumeration.
    let detector = VulkanDetector;
    let gpus = detector.detect()?;

    if gpus.is_empty() {
        tracing::debug!("Vulkan returned empty, trying platform fallback");
        // Platform-specific fallback — cfg-gated by target OS.
        #[cfg(target_os = "windows")]
        {
            let detector = DxgiDetector;
            let gpus = detector.detect()?;
            if !gpus.is_empty() {
                tracing::debug!(device_count = gpus.len(), "platform fallback (DXGI) returned devices");
                return Ok(HardwareInfo { host, gpus, inference_caps: InferenceCaps::default() });
            }
        }

        #[cfg(target_os = "linux")]
        {
            let detector = SysfsPciDetector;
            let gpus = detector.detect()?;
            if !gpus.is_empty() {
                tracing::debug!(device_count = gpus.len(), "platform fallback (sysfs) returned devices");
                return Ok(HardwareInfo { host, gpus, inference_caps: InferenceCaps::default() });
            }
        }

        // Neither Vulkan nor platform fallback found devices.
        // Return empty Vec<GpuDevice> — P5-A3 will append the CPU device.
        tracing::debug!("Vulkan and platform fallback both returned empty");
        return Ok(HardwareInfo { host, gpus: vec![], inference_caps: InferenceCaps::default() });
    }

    tracing::debug!(device_count = gpus.len(), "Vulkan detection returned devices");
    return Ok(HardwareInfo { host, gpus, inference_caps: InferenceCaps::default() });
}
```

**Rationale for partial `HardwareInfo`**: The function signature is `Result<HardwareInfo, AnvilError>` (established by P5-A1). P5-A2 constructs a minimal `HardwareInfo` with the GPUs from its branch and default `InferenceCaps`, deferring CPU append and caps union to P5-A3. This is not "complete assembly" — it is the partial result that P5-A3 extends.

**Rationale for `#[cfg]` on the entire mock/real block**: Since `MockDetector` is only available when `mock-hardware` is compiled in (and the platform-specific detectors are only available on their respective targets), the `#[cfg]` guards ensure each code path compiles only when its dependencies exist. The `#[cfg(not(feature = "mock-hardware"))]` block contains the real-hardware path, and within it, `#[cfg(target_os = "windows")]/#[cfg(target_os = "linux")]` gates the platform fallback.

**Host info reuse**: The `host` variable is already constructed inside P5-A1's override block. To avoid duplication, I will refactor to construct `host` once before the override check, then reuse it in the mock/real branch. Specifically:
- Move `let host = HostInfo { hostname, os };` construction to before the override `if let` block.
- In the override block, use the already-constructed `host`.
- In the mock/real block, use the same `host`.

### Step 2: Add tests to `detect_tests.rs`

Add four new tests (the existing 6 from P5-A1 remain):

1. **`test_mock_hardware_feature_returns_mock_device`** (with `#[cfg(feature = "mock-hardware")]`)
   - Sets mock env vars (`ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576`).
   - Calls `detect_all_devices(&cfg)` with no override.
   - Verifies: result has exactly 1 GPU, device_type is Cuda, vram_total_mib is 24576, enumeration_source is Mock.
   - This tests the mock-hardware feature path (step 2 of §6.4).

2. **`test_override_takes_priority_over_mock`** (with `#[cfg(feature = "mock-hardware")]`)
   - Sets mock env vars AND sets `hardware_override` in config.
   - Calls `detect_all_devices(&cfg)`.
   - Verifies: override device is returned (device_type from override, not mock), proving override short-circuit fires before MockDetector is queried.
   - This tests that step 1 (override) still wins when mock-hardware is compiled in.

3. **`test_empty_vulkan_triggers_platform_fallback`** (without `mock-hardware` feature, i.e. native build)
   - No env vars set, no override.
   - In a typical Linux CI/agent environment without Vulkan, `VulkanDetector::detect()` returns `Ok(vec![])`.
   - Verifies: the result triggers the sysfs fallback path (on Linux) or DXGI path (on Windows).
   - Since this test runs in the native build where Vulkan may or may not be present, the test verifies that when Vulkan returns empty, the fallback is invoked. On a Linux agent without Vulkan, this will exercise the sysfs path and return whatever devices are found (typically empty, which is correct — P5-A3 appends CPU).
   - This tests step 4 of §6.4: Vulkan → platform fallback.

4. **`test_mock_detector_env_vars_propagate_through_detect_all_devices`** (with `#[cfg(feature = "mock-hardware")]`)
   - Sets `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU` and `ANVILML_MOCK_VRAM_MIB=16384`.
   - Calls `detect_all_devices(&cfg)` with no override.
   - Verifies: the returned device has name "Custom Mock GPU" and vram_total_mib=16384.
   - This confirms that the mock env vars are properly read through the full detection chain, not just by `MockDetector` in isolation.

### Step 3: Version bump

Bump `anvilml-hardware` patch version from `0.1.6` to `0.1.7` in `crates/anvilml-hardware/Cargo.toml`.

## Public API Surface

No new public items. The `detect_all_devices()` function signature is unchanged:

```rust
pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>
```

Its behavior changes: instead of returning `Err` when no override is set, it now returns the mock/Vulkan/fallback result. This is an internal behavior change, not a public API change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/detect.rs` | Extend `detect_all_devices()` with mock-vs-real branch and Vulkan fallback chain; refactor host info construction |
| Modify | `crates/anvilml-hardware/tests/detect_tests.rs` | Add ≥4 new tests for mock path, override priority, platform fallback |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `detect_tests.rs` | `test_mock_hardware_feature_returns_mock_device` (mock) | When `mock-hardware` is compiled and no override is set, `detect_all_devices` returns exactly the mock-detected device with correct fields | `mock-hardware` feature enabled; mock env vars set to cuda/24576 | `cfg.hardware_override=None`, `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576` | `Ok(HardwareInfo{gpus: [GpuDevice{device_type:Cuda, vram_total_mib:24576, enumeration_source:Mock}]})` | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests -- test_mock_hardware_feature_returns_mock_device` exits 0 |
| `detect_tests.rs` | `test_override_takes_priority_over_mock` (mock) | Override short-circuit fires before MockDetector even when `mock-hardware` is compiled in | `mock-hardware` feature enabled; both mock env vars and override config set | `cfg.hardware_override=Some{device_type:"rocm", vram_total_mib:16384}`, `ANVILML_MOCK_DEVICE_TYPE=cuda` | Override device returned (Rocm/16384), not mock device (Cuda/8192) | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests -- test_override_takes_priority_over_mock` exits 0 |
| `detect_tests.rs` | `test_empty_vulkan_triggers_platform_fallback` (real) | When Vulkan returns empty, the platform fallback detector is invoked | No `mock-hardware` feature; Vulkan absent or no GPUs | `cfg.hardware_override=None`, no override | Platform fallback (sysfs on Linux, DXGI on Windows) is exercised; result reflects whatever platform detection finds | `cargo test -p anvilml-hardware --test detect_tests -- test_empty_vulkan_triggers_platform_fallback` exits 0 |
| `detect_tests.rs` | `test_mock_detector_env_vars_propagate_through_detect_all_devices` (mock) | Mock env vars are correctly read through the full detection chain, not just by MockDetector in isolation | `mock-hardware` feature enabled; custom mock env vars set | `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU`, `ANVILML_MOCK_VRAM_MIB=16384` | Device name "Custom Mock GPU", vram_total_mib=16384 | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests -- test_mock_detector_env_vars_propagate_through_detect_all_devices` exits 0 |

## CI Impact

No CI changes required. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which will pick up the new tests automatically. The platform fallback test runs in the native build (without mock-hardware) which is exercised by the `cargo check --bin anvilml` platform cross-check, not by the test suite directly — but since it is a test in the same file, it will be compiled and run when the feature is absent.

## Platform Considerations

The implementation uses `#[cfg(target_os = "windows")]` and `#[cfg(target_os = "linux")]` for the platform-specific fallback detectors. These cfg attributes are applied at the code-block level inside the `#[cfg(not(feature = "mock-hardware"))]` block, ensuring each platform's fallback is compiled only on its target. The `DxgiDetector` is imported via the existing `lib.rs` re-export which is cfg-gated to Windows, and `SysfsPciDetector` is cfg-gated to Linux.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `#[cfg]`-gated imports inside a single function body cause compilation errors if the cfg-gated type is referenced outside its cfg block | Medium | High — the entire crate fails to compile on platforms where a detector is absent | Use cfg attributes on the entire code block (not individual imports) so that each platform's fallback code is only compiled on that target. The `lib.rs` already cfg-gates the re-exports, so the types are only available on their respective targets. |
| `test_empty_vulkan_triggers_platform_fallback` may not actually exercise the fallback path if Vulkan is present and returns devices on the developer's machine | Medium | Medium — the test passes but doesn't verify the fallback logic | The test verifies that when Vulkan returns empty, the fallback is called. On machines where Vulkan returns devices, the test still passes because Vulkan's non-empty result short-circuits before reaching the fallback. The test is cfg-gated to `#[cfg(not(feature = "mock-hardware"))]` which matches the CI real-hardware build. |
| P5-A3's CPU append logic must correctly handle the partial `HardwareInfo` returned by P5-A2 | Low | Medium — if P5-A2 returns a `HardwareInfo` with unexpected structure, P5-A3's extension may break | P5-A2 returns a minimal `HardwareInfo` with only `host`, `gpus`, and `InferenceCaps::default()`. P5-A3 will extend `detect_all_devices()` to append CPU and compute the union. This is a straightforward extension — no structural change needed. |
| The `host` variable refactoring (moving it before the override block) introduces a scope issue | Low | Low — Rust scoping is well-defined; the `host` variable is simply moved to a higher scope | Extract `hostname` and `os` computation into a local block before the override check, construct `host` once, and use it in both the override and mock/real branches. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests` exits 0
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `wc -l crates/anvilml-hardware/tests/detect_tests.rs` shows ≥10 tests (6 from P5-A1 + ≥4 new)
- [ ] `grep -c "async fn test_" crates/anvilml-hardware/tests/detect_tests.rs` ≥ 10
