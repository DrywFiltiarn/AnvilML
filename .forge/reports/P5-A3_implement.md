# Implementation Report: P5-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P5-A3                                       |
| Phase         | 005 — Hardware Detection: Orchestration     |
| Description   | anvilml-hardware: CPU-append + HardwareInfo assembly |
| Implemented   | 2026-06-29T12:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Completed `detect_all_devices()` in `crates/anvilml-hardware/src/detect.rs` by appending the unconditional CPU fallback device last and assembling the final `HardwareInfo` with a field-wise OR union of per-device `InferenceCaps`. All 5 non-override code paths (mock-hardware, Vulkan success, DXGI fallback success, sysfs fallback success, both empty) and the override path now produce complete `HardwareInfo` with at least one device. Added 4 new tests and updated 4 existing tests to reflect the new device count invariant (2 devices for override path). Total test count in `detect_tests.rs` is 14.

## Resolved Dependencies

None. This task uses only existing types from `anvilml-core` (`HardwareInfo`, `GpuDevice`, `HostInfo`, `InferenceCaps`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `ServerConfig`) and existing crate types from `anvilml-hardware` (`CpuDetector`, `DeviceDetector` trait). No new external crates or features are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/detect.rs` | Added `CpuDetector` import, implemented CPU-append + caps-union assembly in all 6 code paths, added `compute_caps_union()` helper, updated doc comments |
| Modify | `crates/anvilml-hardware/tests/detect_tests.rs` | Updated 4 existing tests for new device count (2 instead of 1), added 4 new tests (cpu_device_always_present_and_last, inference_caps_is_caps_union, inference_caps_union_correctness, host_fields_non_empty, override_path_still_has_cpu_device) |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bumped version 0.1.7 → 0.1.8 |
| Modify | `docs/TESTS.md` | Added 8 new/updated test entries for all detect_tests.rs tests |

## Commit Log

```
 .forge/reports/P5-A3_plan.md                  | 127 ++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +-
 Cargo.lock                                    |   2 +-
 crates/anvilml-hardware/Cargo.toml            |   2 +-
 crates/anvilml-hardware/src/detect.rs         | 124 +++++++---
 crates/anvilml-hardware/tests/detect_tests.rs | 336 ++++++++++++++++++++++----
 docs/TESTS.md                                 | 168 +++++++++++++
 8 files changed, 696 insertions(+), 82 deletions(-)
```

## Test Results

```
     Running tests/detect_tests.rs (target/debug/deps/detect_tests-ee5da0633839ba2c)

running 14 tests
test test_host_fields_non_empty ... ok
test test_inference_caps_is_caps_union ... ok
test test_override_absent_returns_hardware_info ... ok
test test_inference_caps_union_correctness ... ok
test test_override_cpu_device_type ... ok
test test_mock_detector_env_vars_propagate_through_detect_all_devices ... ok
test test_cpu_device_always_present_and_last ... ok
test test_mock_hardware_feature_returns_mock_device ... ok
test test_override_inference_caps_is_default ... ok
test test_override_path_still_has_cpu_device ... ok
test test_override_present_returns_device ... ok
test test_override_rocm_device_type ... ok
test test_override_takes_priority_over_mock ... ok
test test_override_unrecognized_device_type_defaults_to_cpu ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 117 tests passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.89s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.58s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.65s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

No new `pub` items introduced. The only change to existing public API is the internal behavior of `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` — it now returns complete `HardwareInfo` with CPU fallback and unioned caps instead of partial results with default caps. The `fn compute_caps_union(devices: &[GpuDevice]) -> InferenceCaps` helper is private (no `pub` keyword).

## Deviations from Plan

- The plan's approach section said "an inline block after each `gpus` assignment is more explicit than a closure" — I implemented this exactly, with each non-override code path having its own inline CPU-append + caps-union block.
- The override path was not listed in the plan's "non-override code paths" table, but the plan's test `test_override_path_still_has_cpu_device` explicitly requires the override path to also append CPU. I implemented CPU-append for the override path as well, which changes its device count from 1 to 2. This required updating all 4 existing override-path tests (`test_override_present_returns_device`, `test_override_unrecognized_device_type_defaults_to_cpu`, `test_override_rocm_device_type`, `test_override_cpu_device_type`) to expect `gpus.len() == 2`.
- The existing test `test_override_inference_caps_is_default` still passes because the union of override device caps (default) and CPU caps (default) equals default. No change needed.
- The existing test `test_partial_hardware_info_has_default_inference_caps` was renamed conceptually to `test_inference_caps_is_caps_union` to reflect that caps are now computed (union) rather than hardcoded default. The assertion remains the same (all-false union when all devices have default caps).
- The existing test `test_override_absent_returns_hardware_info` was updated to assert `gpus.len() >= 1` instead of checking for default inference_caps specifically, since caps are now computed.

## Blockers

None.
