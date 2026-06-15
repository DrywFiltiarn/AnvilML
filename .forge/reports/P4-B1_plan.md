# Plan Report: P4-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P4-B1                                             |
| Phase       | 004 — Hardware Detection                          |
| Description | anvilml-hardware: MockDetector + comprehensive mock test suite |
| Depends on  | P4-A1, P4-A2, P4-A5 (prerequisites in phase 4)    |
| Project     | anvilml                                           |
| Planned at  | 2026-06-15T12:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Confirm that the `MockDetector` implementation in `crates/anvilml-hardware/src/mock.rs` and its test suite in `crates/anvilml-hardware/tests/mock_tests.rs` satisfy the task requirements: the mock detector reads `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, and `ANVILML_MOCK_DEVICE_NAME` environment variables with correct defaults, all tests use `#[serial]` from the `serial_test` crate with unconditional env-var capture-and-restore per `ENVIRONMENT.md §11.3`, and `cargo test -p anvilml-hardware --features mock-hardware` exits 0.

## Scope

### In Scope
- Verification of existing `crates/anvilml-hardware/src/mock.rs`: the `MockDetector` struct, its `DeviceDetector` trait implementation (`detect()` and `refresh_vram()`), cfg-gating behind `#[cfg(feature = "mock-hardware")]`, and environment variable defaults (device_type=cpu, vram=8192 MiB, name="Mock GPU").
- Verification of existing `crates/anvilml-hardware/tests/mock_tests.rs`: 9 tests using `#[serial_test::serial]`, each using `EnvGuard` for unconditional env-var capture-and-restore.
- Verification that `cargo test -p anvilml-hardware --features mock-hardware` exits 0.
- If any discrepancy is found (missing tests, missing cfg-gate, incorrect env-var handling), correct it in the same files.

### Out of Scope
- Modifying `detect.rs` (the orchestration function `detect_all_devices`) — already completed in P4-A5.
- Modifying `cpu.rs`, `vulkan.rs`, `device_db.rs`, or any other hardware detector module.
- Adding new crates, dependencies, or CI configuration changes.
- Modifying `anvilml-core` types.

## Existing Codebase Assessment

The `anvilml-hardware` crate already has a complete `MockDetector` implementation and test suite from prior commits (P4-A2 created the mock detector, P4-A5 wired it into `detect_all_devices`). Specifically:

**(a) What exists:**
- `mock.rs` (138 lines) contains `MockDetector` (a zero-sized unit struct) behind `#[cfg(feature = "mock-hardware")]`, implementing `DeviceDetector` with `detect()` and `refresh_vram()`. The `detect()` method reads `ANVILML_MOCK_DEVICE_TYPE` (default `"cpu"`), `ANVILML_MOCK_VRAM_MIB` (default `8192`), and `ANVILML_MOCK_DEVICE_NAME` (default `"Mock GPU"`), returning a single `GpuDevice` with `enumeration_source = Mock`. Invalid device types produce an empty vec (graceful fallback).
- `mock_tests.rs` (318 lines) contains 9 tests: 4 unit tests for `MockDetector::detect()` (cuda, rocm, cpu, invalid), and 5 integration tests for `detect_all_devices` (mock-cuda pipeline, hardware override priority, cpu fallback, inference caps union, Ok-return guarantee). All tests are `#[serial]` and use `EnvGuard` for unconditional env-var restoration.
- `detect.rs` (295 lines) contains `detect_all_devices()` which calls `MockDetector::detect()` when the `mock-hardware` feature is active.
- `serial_test = "3.5"` is already declared in `crates/anvilml-hardware/Cargo.toml` under `[dev-dependencies]`.

**(b) Established patterns:**
- Tests use `EnvGuard` struct (RAII drop guard) for env-var capture-and-restore — this is the pattern mandated by `ENVIRONMENT.md §11.3`.
- Tests import `DeviceDetector` from `anvilml_hardware` (crate re-export) and domain types from `anvilml_core`.
- `#[serial_test::serial]` is applied at the test function level, before `#[cfg(feature = "mock-hardware")]` and `#[test]`.
- `MockDetector` follows the same pattern as `CpuDetector`: zero-sized struct, `const fn new()`, `Default` impl.

**(c) Gaps/discrepancies:**
- None found. The implementation matches the design spec (`ANVILML_DESIGN.md §6.5`) and the task requirements exactly.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | serial_test | 3.5             | Lockfile   | n/a (dev-dependency)   |

Note: `serial_test = "3.5"` is already declared in `crates/anvilml-hardware/Cargo.toml` `[dev-dependencies]`. No new dependencies are introduced by this task. The version was confirmed from the project's lockfile per FORGE_AGENT_RULES §6.4 (MCP unavailable for Rust — falling back to lockfile).

## Approach

1. **Read and verify `mock.rs`.** Confirm:
   - `MockDetector` struct is a zero-sized unit struct behind `#[cfg(feature = "mock-hardware")]`.
   - `impl DeviceDetector for MockDetector` is behind `#[cfg(feature = "mock-hardware")]`.
   - `detect()` reads `ANVILML_MOCK_DEVICE_TYPE` (default `"cpu"`), `ANVILML_MOCK_VRAM_MIB` (default `8192`), `ANVILML_MOCK_DEVICE_NAME` (default `"Mock GPU"`).
   - Invalid device type returns `Ok(vec![])` (graceful fallback, not error).
   - Returns a single `GpuDevice` with `enumeration_source = EnumerationSource::Mock`, `capabilities_source = CapabilitySource::Fallback`, PCI IDs = 0.
   - `refresh_vram()` returns `(0, 0)` for all indices.
   - INFO-level log call on device detection per mandatory logging convention.
   - **Rationale:** These are the exact requirements from `ANVILML_DESIGN.md §6.5` and the task description. No changes needed — just verification.

2. **Read and verify `mock_tests.rs`.** Confirm:
   - 9 test functions exist (≥ 8 required).
   - All tests are annotated `#[serial_test::serial]`.
   - All tests use `EnvGuard` for env-var capture-and-restore (unconditional, outside conditional blocks).
   - Test coverage includes: cuda variant, rocm variant, cpu variant, invalid type fallback, full pipeline with mock-cuda, hardware override priority, cpu fallback, inference caps union, Ok-return guarantee.
   - **Rationale:** The `EnvGuard` RAII pattern ensures env-var restoration even on panic or early return — this is the exact pattern mandated by `ENVIRONMENT.md §11.3`.

3. **Verify acceptance criterion.** Run `cargo test -p anvilml-hardware --features mock-hardware` and confirm exit 0. If any test fails, diagnose and fix before completing the task.

4. **If any discrepancy found**, modify `mock.rs` or `mock_tests.rs` to bring them into conformance. No other files are in scope.

## Public API Surface

No new public items are introduced by this task. The existing public API is:

| Item | Type | Module Path | Notes |
|------|------|-------------|-------|
| `MockDetector` | struct | `anvilml_hardware::mock::MockDetector` | Zero-sized unit struct, cfg-gated |
| `MockDetector::new` | fn | `anvilml_hardware::mock::MockDetector::new` | `pub const fn new() -> Self` |
| `MockDetector::detect` | fn (trait impl) | `DeviceDetector::detect` for `MockDetector` | `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` |
| `MockDetector::refresh_vram` | fn (trait impl) | `DeviceDetector::refresh_vram` for `MockDetector` | `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` |

These are already present from prior commits. The plan does not introduce new `pub` items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| READ | `crates/anvilml-hardware/src/mock.rs` | Verify MockDetector implementation (already exists) |
| READ | `crates/anvilml-hardware/tests/mock_tests.rs` | Verify test suite (already exists) |
| MODIFY (conditional) | `crates/anvilml-hardware/src/mock.rs` | Only if verification finds discrepancies |
| MODIFY (conditional) | `crates/anvilml-hardware/tests/mock_tests.rs` | Only if verification finds discrepancies |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/mock_tests.rs` | `test_mock_detect_cuda` | MockDetector returns one CUDA device with correct VRAM and enumeration source | `ANVILML_MOCK_DEVICE_TYPE=cuda`, VRAM=16384, name="Mock CUDA" | Env vars set via EnvGuard | 1 device, device_type=Cuda, vram=16384, source=Mock | `cargo test -p anvilml-hardware --features mock-hardware -- test_mock_detect_cuda` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_rocm` | MockDetector returns one ROCm device | `ANVILML_MOCK_DEVICE_TYPE=rocm`, VRAM=8192, name="Mock ROCm" | Env vars set via EnvGuard | 1 device, device_type=Rocm, vram=8192 | `cargo test -p anvilml-hardware --features mock-hardware -- test_mock_detect_rocm` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_cpu` | MockDetector returns one CPU-type device | `ANVILML_MOCK_DEVICE_TYPE=cpu`, VRAM=0, name="Mock CPU" | Env vars set via EnvGuard | 1 device, device_type=Cpu, vram=0 | `cargo test -p anvilml-hardware --features mock-hardware -- test_mock_detect_cpu` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_invalid_type` | Invalid device type returns empty vec (graceful fallback) | `ANVILML_MOCK_DEVICE_TYPE=invalid` | Env var set via EnvGuard | Empty device list, no error | `cargo test -p anvilml-hardware --features mock-hardware -- test_mock_detect_invalid_type` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_mock_cuda` | Full pipeline: detect_all_devices returns mock CUDA device + CPU fallback | `ANVILML_MOCK_DEVICE_TYPE=cuda`, VRAM=16384 | ServerConfig::default(), in-memory SQLite pool | ≥1 GPU (mock CUDA) + 1 CPU, host info populated | `cargo test -p anvilml-hardware --features mock-hardware -- test_detect_all_devices_mock_cuda` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_hardware_override` | Hardware override takes priority over mock detector | `ANVILML_MOCK_DEVICE_TYPE=cuda` (ignored), override=rocm/32768 | ServerConfig with hardware_override | Override device (rocm, 32768 MiB, source=Override) + CPU fallback | `cargo test -p anvilml-hardware --features mock-hardware -- test_detect_all_devices_hardware_override` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_cpu_fallback` | CPU fallback always produces at least one device even when mock returns empty | `ANVILML_MOCK_DEVICE_TYPE=invalid` (returns empty) | ServerConfig::default(), in-memory SQLite pool | ≥1 CPU device | `cargo test -p anvilml-hardware --features mock-hardware -- test_detect_all_devices_cpu_fallback` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_inference_caps_union` | inference_caps is union of all GPU caps (correct with zero-cap mock devices) | `ANVILML_MOCK_DEVICE_TYPE=cuda` | ServerConfig::default(), in-memory SQLite pool | Valid InferenceCaps struct (all fields are valid bools) | `cargo test -p anvilml-hardware --features mock-hardware -- test_detect_all_devices_inference_caps_union` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_returns_ok` | detect_all_devices always returns Ok under mock-hardware | No env vars needed | ServerConfig::default(), in-memory SQLite pool | Result is Ok | `cargo test -p anvilml-hardware --features mock-hardware -- test_detect_all_devices_returns_ok` exits 0 |

## CI Impact

No CI changes required. The `serial_test` dev-dependency is already declared in `crates/anvilml-hardware/Cargo.toml`. The tests are picked up by the existing CI jobs (`rust-linux` and `rust-windows`) which run `cargo test --workspace --features mock-hardware`. No new CI jobs, gates, or configuration changes are needed.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. `MockDetector` reads environment variables and constructs a `GpuDevice` — all of which are platform-neutral. The `#[cfg(feature = "mock-hardware")]` gating is the same on all platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed in the mock code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `MockDetector` struct declaration is not behind `#[cfg(feature = "mock-hardware")]` — the struct itself must be cfg-gated so it doesn't exist when the feature is disabled. The `impl DeviceDetector` and `impl Default` blocks are also cfg-gated. | Low | High | Verify that `pub struct MockDetector;` at line 16 of mock.rs is preceded by `#[cfg(feature = "mock-hardware")]`. If missing, add it. |
| Tests in `mock_tests.rs` are missing `#[serial_test::serial]` annotation — concurrent test threads would race on `std::env::set_var`, causing flaky failures. | Low | High | Verify all 9 test functions have `#[serial_test::serial]` before `#[cfg(feature = "mock-hardware")]` and `#[test]`. If any are missing, add them. |
| `EnvGuard` does not restore env vars on panic — if the Drop impl is incorrect, subsequent tests would observe mutated env state. | Low | Medium | Verify `EnvGuard::drop()` unconditionally restores the prior value or removes the var. The current implementation uses `match &self.prior` which always runs. |
| `cargo test -p anvilml-hardware --features mock-hardware` fails due to pre-existing compilation errors in other modules (e.g., missing `HardwareOverrideConfig` import). | Low | High | If compilation fails, diagnose the error. If it's in mock.rs or mock_tests.rs, fix it. If it's in another file, document under Deviations from Plan and fix minimally. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `grep -c "fn test_" crates/anvilml-hardware/tests/mock_tests.rs` returns ≥ 8 (currently returns 9)
- [ ] `grep -c "#\[serial_test::serial\]" crates/anvilml-hardware/tests/mock_tests.rs` returns ≥ 8 (all tests use serial)
- [ ] `grep -c "EnvGuard" crates/anvilml-hardware/tests/mock_tests.rs` returns ≥ 8 (all tests use capture-and-restore)
- [ ] `grep "#\[cfg(feature = \"mock-hardware\")\]" crates/anvilml-hardware/src/mock.rs` returns ≥ 1 (cfg-gating present)
- [ ] `grep "ANVILML_MOCK_DEVICE_TYPE" crates/anvilml-hardware/src/mock.rs` returns 1 (device type env var read)
- [ ] `grep "ANVILML_MOCK_VRAM_MIB" crates/anvilml-hardware/src/mock.rs` returns 1 (VRAM env var read)
- [ ] `grep "ANVILML_MOCK_DEVICE_NAME" crates/anvilml-hardware/src/mock.rs` returns 1 (device name env var read)
