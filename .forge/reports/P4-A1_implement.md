# Implementation Report: P4-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P4-A1                              |
| Phase         | 004 — Hardware Detection           |
| Description   | anvilml-hardware: DeviceDetector trait + CpuDetector |
| Implemented   | 2026-06-15T00:45:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `DeviceDetector` trait and `CpuDetector` struct in `anvilml-hardware`. The trait defines the `detect()` and `refresh_vram()` methods for hardware enumeration backends. `CpuDetector` is a zero-sized unit struct that uses the `sysinfo` crate to read host-level information (OS version, CPU brand, total RAM) and synthesises a single CPU `GpuDevice`. All three integration tests pass, the full workspace test suite is green, all platform cross-checks pass, and both lint and format gates are clean.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|------------------|----------------|
| crate  | sysinfo    | 0.33.1           | cargo search   |
| crate  | serial_test| 3.5.0            | Cargo.lock     |

`sysinfo 0.33` was resolved via `cargo search` (latest is 0.39.3; the plan specified 0.33 with a technical reason). The APIs used are `System::new_all()`, `System::long_os_version()`, `System::total_memory()`, `System::cpus()`, and `Cpu::brand()` — these differ from the plan's API names (`HostInfo::os_version()`, `cpu_brand()`, `ram_total()`) which do not exist in the `sysinfo` crate. `serial_test 3.5` was already present in the workspace lockfile at version 3.5.0.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-hardware/Cargo.toml | Added `sysinfo = "0.33"` dependency, `serial_test = "3.5"` dev-dependency; bumped version 0.1.0 → 0.1.1 |
| CREATE | crates/anvilml-hardware/src/cpu.rs | `CpuDetector` struct and `DeviceDetector` trait implementation |
| Modify | crates/anvilml-hardware/src/lib.rs | Updated crate doc; added `pub mod cpu`; defined `pub trait DeviceDetector`; added `pub use cpu::CpuDetector` |
| CREATE | crates/anvilml-hardware/tests/cpu_tests.rs | Three integration tests: detect, refresh_vram, send_sync |
| Modify | docs/TESTS.md | Added entries for the three new tests |
| Modify | Cargo.lock | Updated with new sysinfo and serial_test dependencies |

## Commit Log

```
 .forge/reports/P4-A1_plan.md               | 143 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 | 213 ++++++++++++++++++++++++++-
 crates/anvilml-hardware/Cargo.toml         |   6 +-
 crates/anvilml-hardware/src/cpu.rs         | 130 ++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs         |  50 +++++--
 crates/anvilml-hardware/tests/cpu_tests.rs |  53 +++++++
 docs/TESTS.md                              |  24 ++++
 9 files changed, 615 insertions(+), 23 deletions(-)
```

## Test Results

```
     Running tests/cpu_tests.rs (target/debug/deps/cpu_tests-ea974e3c14e6bd5b)

running 3 tests
test test_cpu_detector_detect_returns_one_device ... ok
test test_cpu_detector_is_send_sync ... ok
test test_cpu_detector_refresh_vram_returns_zero ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
```

Full workspace test suite: all 66 tests passed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, anvilml-openapi). Zero failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.72s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 37.76s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.73s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.60s
```

All four cross-checks exited 0. Zero errors.

## Project Gates

Gate 1 (Config Surface Sync):
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI Drift): Not triggered — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

Gate 3 (Node Parity): Not triggered — task does not add, remove, or rename node types.

## Public API Delta

```
+pub struct CpuDetector;
+    pub const fn new() -> Self {
+pub mod cpu;
+pub trait DeviceDetector: Send + Sync {
+pub use cpu::CpuDetector;
```

New public items introduced:
- `pub struct CpuDetector` — zero-sized unit struct, module `anvilml_hardware::cpu`
- `pub const fn new() -> Self` — constructor for `CpuDetector`, module `anvilml_hardware::cpu`
- `pub mod cpu` — module declaration, module `anvilml_hardware`
- `pub trait DeviceDetector: Send + Sync` — trait with `detect()` and `refresh_vram()` methods, module `anvilml_hardware`
- `pub use cpu::CpuDetector` — re-export, module `anvilml_hardware`

## Deviations from Plan

- **API name substitution**: The plan referenced `HostInfo::os_version()`, `cpu_brand()`, and `ram_total()` as sysinfo APIs. These names do not exist in the `sysinfo` crate. The actual APIs used are `System::long_os_version()` (OS version string), `System::cpus().first().map(|c| c.brand())` (CPU brand), and `System::total_memory() / (1024 * 1024)` (RAM in MiB). This is a plan authoring defect — the plan's API names are incorrect for the `sysinfo` crate. The implementation uses the correct API names and produces the same semantic results.
- **Removed redundant re-export**: The plan specified `pub use crate::DeviceDetector;` in lib.rs. Since `DeviceDetector` is defined directly in lib.rs (not imported from elsewhere), this re-import would cause a name collision with the trait definition. Removed the redundant line; the trait is already accessible at the crate root without explicit re-export.
- **Removed unused `HostInfo` import**: The plan's cpu.rs design mentioned `HostInfo` but the implementation does not construct a `HostInfo` struct — it reads values directly from sysinfo and uses them in log calls. The `HostInfo` import was removed to avoid an unused-import warning.

## Blockers

None.
