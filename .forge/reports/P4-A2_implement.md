# Implementation Report: P4-A2

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P4-A2                                             |
| Phase         | 004 — Hardware Detection: Detectors               |
| Description   | anvilml-hardware: CpuDetector always returns one CPU device |
| Implemented   | 2026-06-28T23:05:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `CpuDetector` — the unconditional final-fallback CPU detector that always returns exactly one synthesized `GpuDevice` with `enumeration_source: EnumerationSource::Cpu`. This guarantees `detect_all_devices()` (Phase 5) always returns at least one device, fulfilling §6.2 of the design. Implemented `DeviceDetector` trait methods (`detect` and `refresh_vram`) as pure value construction with no I/O, no fallible operations, and no conditional branches. Added 6 integration tests, updated `lib.rs` with module re-export, bumped crate version to 0.1.1, and updated the test catalogue.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It uses only `anvilml-core` types already declared as a dependency in `anvilml-hardware`'s `Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/cpu.rs` | `CpuDetector` struct + `DeviceDetector` impl (45 lines) |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `pub mod cpu;` and `pub use cpu::CpuDetector;` (4 → 6 lines) |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bumped patch version `0.1.0 → 0.1.1` |
| CREATE | `crates/anvilml-hardware/tests/cpu_tests.rs` | 6 integration tests for `CpuDetector` (94 lines) |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries for cpu_tests |

## Commit Log

```
 .forge/reports/P4-A2_plan.md               | 143 +++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +--
 Cargo.lock                                 |   2 +-
 crates/anvilml-hardware/Cargo.toml         |   2 +-
 crates/anvilml-hardware/src/cpu.rs         |  45 +++++++++
 crates/anvilml-hardware/src/lib.rs         |   2 +
 crates/anvilml-hardware/tests/cpu_tests.rs |  94 +++++++++++++++++++
 docs/TESTS.md                              |  72 +++++++++++++++
 9 files changed, 368 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/cpu_tests.rs (target/debug/deps/cpu_tests-ae6104c9edbca9de)

running 6 tests
test test_cpu_detect_never_errors ... ok
test test_cpu_detector_all_device_fields ... ok
test test_cpu_detector_device_type_is_cpu ... ok
test test_cpu_detector_enumeration_source_is_cpu ... ok
test test_cpu_detector_refresh_vram_returns_zero ... ok
test test_cpu_detector_returns_one_device ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 93 tests passed, 0 failed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-artifacts, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, meaning no formatting drift)
```

## Platform Cross-Check

```
# Check 1 — Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
--- CHECK 1 PASS

# Check 2 — Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.07s
--- CHECK 2 PASS

# Check 3 — Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s
--- CHECK 3 PASS

# Check 4 — Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.64s
--- CHECK 4 PASS
```

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
# From lib.rs diff:
+pub mod cpu;
+pub use cpu::CpuDetector;

# From new cpu.rs file:
pub struct CpuDetector;
```

New public items:
- `pub struct CpuDetector` — module `anvilml_hardware::cpu`
- `pub mod cpu` — module re-export in `anvilml_hardware`
- `impl DeviceDetector for CpuDetector` — trait impl (methods `detect` and `refresh_vram` are `pub` via trait)

## Deviations from Plan

- Added `pub use cpu::CpuDetector;` to `lib.rs` (not in the original plan). The plan only specified `pub mod cpu;`. The re-export was needed because the integration test file imports `CpuDetector` from the crate's public API (`use anvilml_hardware::cpu::CpuDetector;`), and the test could not find the struct without the module path. This is a minor addition to the public API surface — the struct is still only reachable via `anvilml_hardware::cpu::CpuDetector` or `anvilml_hardware::CpuDetector`.
- The plan's test import statement listed `use anvilml_hardware::detect::DeviceDetector;` and `use anvilml_core::types::*;` but did not include `CpuDetector` in the imports. Added `use anvilml_hardware::cpu::CpuDetector;` to the test file to resolve the struct reference.

## Blockers

None.
