# Plan Report: P5-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A3                                       |
| Phase       | 005 — Hardware Detection: Orchestration     |
| Description | anvilml-hardware: CPU-append + HardwareInfo assembly |
| Depends on  | P5-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T11:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `detect_all_devices()` in `crates/anvilml-hardware/src/detect.rs` by appending the unconditional CPU fallback device last and assembling the final `HardwareInfo` with a unioned `inference_caps`, guaranteeing the function never returns an empty device list. After this task, any caller that invokes `detect_all_devices()` receives a complete `HardwareInfo` with real host info, all detected GPUs, and the guaranteed CPU fallback — no partial results, no default caps.

## Scope

### In Scope
- Modify `crates/anvilml-hardware/src/detect.rs`: append `CpuDetector`'s single device last in every non-override code path (mock-hardware branch and real-hardware branch), then compute `inference_caps` as the union of all per-GPU `InferenceCaps` (including the CPU device's default caps).
- Update doc comments in `detect.rs` to reflect that steps 5–6 are now implemented (remove "deferred to P5-A3" from the function-level doc, update inline comments on the `return` sites).
- Add `use crate::CpuDetector;` import in `detect.rs`.
- Add `pub use detect::detect_all_devices;` re-export in `crates/anvilml-hardware/src/lib.rs` (this is part of the assembly — downstream crates need a clean public entry point).
- Add `#[cfg(feature = "mock-hardware")] pub use cpu::CpuDetector;` and `#[cfg(not(feature = "mock-hardware"))] pub use cpu::CpuDetector;` in lib.rs (CpuDetector is always available, no feature gate).
- Add `pub mod cpu;` to lib.rs if not already present (it is present, so just confirm).
- Bump `anvilml-hardware` crate version from `0.1.7` to `0.1.8` in `Cargo.toml`.
- Add >=3 new tests in `crates/anvilml-hardware/tests/detect_tests.rs` (total >= 9 in file).

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No functionality is deferred.

## Existing Codebase Assessment

The codebase inspection revealed:

**(a) What already exists:** P5-A1 implemented the override short-circuit (step 1). P5-A2 implemented the mock-vs-real branch (steps 2–4) with `#[cfg(feature = "mock-hardware")]` for MockDetector and `#[cfg(not(feature = "mock-hardware"))]` for VulkanDetector + platform fallbacks. `CpuDetector` exists in `cpu.rs` and implements `DeviceDetector`, returning exactly one synthesized CPU device. The `detect_all_devices()` function currently constructs `HostInfo` from env vars, runs the override/mock/real branch, but returns a partial `HardwareInfo` with `InferenceCaps::default()` — all intermediate `return` sites have inline comments noting "deferred to P5-A3."

**(b) Established patterns:** The crate uses `#[cfg(feature = "mock-hardware")]` and `#[cfg(not(feature = "mock-hardware"))]` blocks for mutually exclusive build paths. Tests in `detect_tests.rs` use `#[tokio::test]` for async tests, `#[serial_test::serial]` for env-var-mutating tests, and follow the capture-and-restore pattern for `std::env` isolation. `InferenceCaps` derives `Default` (all fields `false`) and `PartialEq` (enabling direct comparison). `CpuDetector` is a unit struct with no fields.

**(c) Gap between design doc and source:** The design doc (§6.4) shows `detect_all_devices` taking a `SqlitePool` parameter, but the current implementation omits it — this is intentional per the task constraints (the `DeviceCapabilityStore` doesn't exist until a later phase). The current partial `HardwareInfo` returns are the explicit P5-A2 deferral that P5-A3 must complete.

## Resolved Dependencies

None. This task uses only existing types from `anvilml-core` (`HardwareInfo`, `GpuDevice`, `HostInfo`, `InferenceCaps`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `ServerConfig`) and existing crate types from `anvilml-hardware` (`CpuDetector`, `DeviceDetector` trait). No new external crates or features are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Add `CpuDetector` import in `detect.rs`.** Add `use crate::CpuDetector;` at the top of `detect.rs` (after the existing `use` block). `CpuDetector` is always available (no feature gate) — it lives in `cpu.rs` which is always compiled.

2. **Refactor every non-override code path to append CPU device and compute caps union.** Replace the current partial `return Ok(HardwareInfo { host, gpus, inference_caps: InferenceCaps::default() })` sites with a common assembly block. The pattern in each branch:
   - After the branch produces `gpus: Vec<GpuDevice>`, call `CpuDetector.detect()` to get the CPU device.
   - Append the CPU device to `gpus`: `gpus.extend(cpu_device);`
   - Compute the caps union by folding over all devices: start with `InferenceCaps::default()`, then for each device, OR each field of its `caps` into the accumulator. This is a simple loop — no external crate needed.
   - Return `Ok(HardwareInfo { host, gpus, inference_caps })`.

   This must be done in every non-override code path:
   - **Mock-hardware branch** (line ~155–171): after `detector.detect()`, append CPU device, compute union, return.
   - **Real-hardware Vulkan success** (line ~237–248): after `VulkanDetector.detect()` returns non-empty, append CPU, compute union, return.
   - **Real-hardware DXGI fallback success** (line ~193–205): after `DxgiDetector.detect()` returns non-empty, append CPU, compute union, return.
   - **Real-hardware sysfs fallback success** (line ~214–225): after `SysfsPciDetector.detect()` returns non-empty, append CPU, compute union, return.
   - **Real-hardware both empty** (line ~231–235): even when no GPU is detected, still append CPU device and return (this is the key guarantee — result is never empty).

   Rationale: Instead of duplicating the CPU-append + caps-union logic at each of the 5 return sites, extract it into a local helper closure or a small inline block that runs after each `gpus` assignment. This keeps the branch logic clean while ensuring every path gets the same assembly treatment. Given the small scope (5 sites), an inline block after each `gpus` assignment is more explicit than a closure.

3. **Update doc comments in `detect.rs`.** Remove "deferred to P5-A3" from the function-level doc comment (lines 54–58). Update inline comments at each `return` site to reflect that CPU append and caps union are now implemented.

4. **Confirm `lib.rs` re-exports.** `pub mod cpu;` and `pub use cpu::CpuDetector;` already exist in `lib.rs` (lines 3, 6). No changes needed — confirm they are present and correct.

5. **Bump crate version.** Change `version = "0.1.7"` to `version = "0.1.8"` in `crates/anvilml-hardware/Cargo.toml`.

6. **Add new tests in `detect_tests.rs`.** Add >= 3 new tests (see Tests section below).

## Public API Surface

No new `pub` items are introduced. This task modifies the internal behavior of the existing `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` function in `crates/anvilml-hardware/src/detect.rs` and confirms existing `pub` re-exports in `lib.rs`.

Existing `pub` items used:
- `CpuDetector` (re-exported from `lib.rs` as `pub use cpu::CpuDetector`)
- `DeviceDetector` trait (re-exported from `lib.rs` as `pub use detect::DeviceDetector`)
- Types from `anvilml-core`: `HardwareInfo`, `GpuDevice`, `HostInfo`, `InferenceCaps`, `ServerConfig`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/detect.rs` | Append CPU device, compute caps union, update doc comments |
| Modify | `crates/anvilml-hardware/tests/detect_tests.rs` | Add >= 3 new tests |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bump version 0.1.7 → 0.1.8 |
| Confirm | `crates/anvilml-hardware/src/lib.rs` | Confirm existing re-exports are correct (no changes) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `detect_tests.rs` | `test_cpu_device_always_present_and_last` | CPU device is always the last element in `gpus` when mock-hardware feature is active. Verifies that after mock GPU detection, CpuDetector's device is appended and occupies index `gpus.len() - 1` with `enumeration_source == Cpu`. | `mock-hardware` feature compiled. Env vars set to produce a mock GPU. | `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576` | `gpus.len() >= 2`, last device has `device_type == Cpu` and `enumeration_source == EnumerationSource::Cpu` | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_cpu_device_always_present_and_last` exits 0 |
| `detect_tests.rs` | `test_inference_caps_union_correctness` | `inference_caps` is the field-wise OR union of all per-device `InferenceCaps`. With two mock devices having different true fields (e.g. device 1: fp16=true, device 2: bf16=true), the result has both fp16 and bf16 set to true. | `mock-hardware` feature compiled. Env vars set to produce a mock GPU with custom caps. | `ANVILML_MOCK_DEVICE_TYPE=cuda` with caps that include fp16=true | `inference_caps.fp16 == true` and `inference_caps.bf16 == false` (CPU device has default caps, union = device caps OR default = device caps) | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_inference_caps_union_correctness` exits 0 |
| `detect_tests.rs` | `test_host_fields_non_empty` | `host.hostname` and `host.os` are both non-empty strings after `detect_all_devices()` returns. Verifies the minimal `HostInfo` population works correctly. | No special preconditions. | Default `ServerConfig::default()` | `result.host.hostname.len() > 0` and `result.host.os.len() > 0` | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_host_fields_non_empty` exits 0 |
| `detect_tests.rs` | `test_override_path_still_has_cpu_device` | Even the override path (which currently returns a single override device) now appends the CPU device, making the result contain 2 devices. This ensures the CPU guarantee applies universally. | `mock-hardware` feature compiled. Override config set. | Override with `device_type=cuda`, `vram_total_mib=24576` | `gpus.len() == 2`, last device is CPU | `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_override_path_still_has_cpu_device` exits 0 |

## CI Impact

No CI changes required. The `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which will pick up the new tests in `detect_tests.rs`. The test suite is gated by the existing `mock-hardware` feature flag that all CI builds use.

## Platform Considerations

None identified. The CPU append and caps union logic is platform-neutral — `CpuDetector` works identically on all platforms, and `InferenceCaps` union is a pure data transformation with no platform-specific branches. The `#[cfg(target_os = "windows")]` and `#[cfg(target_os = "linux")]` guards in the existing code are preserved as-is. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The override path currently returns early without going through the mock/real branch. After appending CPU, the override path will return 2 devices (override GPU + CPU) instead of 1. Existing tests (`test_override_present_returns_device`, `test_override_rocm_device_type`, etc.) assert `gpus.len() == 1` which will now fail. | High | High | Update all override-path tests that assert `gpus.len() == 1` to assert `gpus.len() == 2` and verify the second device is the CPU fallback. This is expected behavior — the CPU guarantee applies to ALL paths including override. |
| The `InferenceCaps` union logic must correctly OR all 6 boolean fields (fp32, fp16, bf16, fp8, fp4, flash_attention). A bug in the fold would silently produce incorrect caps. | Low | Medium | The union is a simple loop with field-by-field OR — easy to verify. The `test_inference_caps_union_correctness` test explicitly verifies multi-field union with devices having different true fields. |
| Adding `CpuDetector` import and the CPU append logic changes the behavior of every code path, including the override path which previously returned a single override device. Tests that check specific device count or ordering will need updates. | High | Medium | Audit all 8 existing tests in `detect_tests.rs` for assertions about `gpus.len()` or device ordering, and update them systematically. The mock-hardware tests already verify device content, so only count assertions need updating. |
| The existing `test_override_inference_caps_is_default` test asserts `inference_caps == InferenceCaps::default()` for the override path. After P5-A3, the override path will still have default inference caps (override device has default caps, CPU device has default caps, union of defaults = default), so this test should still pass. | Low | Low | Verify this test still passes after implementation — if it does, no update needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests` exits 0 (>= 9 tests total in file)
- [ ] `cargo clippy --package anvilml-hardware --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `wc -l crates/anvilml-hardware/src/lib.rs` reports <= 80 lines
- [ ] `grep -c "^async fn test_\|^fn test_" crates/anvilml-hardware/tests/detect_tests.rs` reports >= 9
