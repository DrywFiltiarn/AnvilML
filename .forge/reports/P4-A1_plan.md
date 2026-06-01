# Plan Report: P4-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A1                                         |
| Phase       | 004 — Hardware Detection                     |
| Description | anvilml-hardware: DeviceDetector trait and CPU detector |
| Depends on  | P3-A6                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T15:10:02Z                          |
| Attempt     | 1                                             |

## Objective

Introduce the `anvilml-hardware` crate's foundational abstractions: a `DeviceDetector` trait that defines the interface all hardware backends must implement, and a concrete `CpuDetector` that returns a single synthetic CPU device. This establishes the trait-based detection pattern used by all subsequent detectors (CUDA, ROCm, mock) in Phase 4. The `sysinfo` crate is added as a dependency to support VRAM refresh operations and future host-info population.

## Scope

### In Scope
- Add `sysinfo` dependency to `anvilml-hardware/Cargo.toml`
- Define the `DeviceDetector` trait in `crates/anvilml-hardware/src/lib.rs` with two methods: `detect()` returning `Result<Vec<GpuDevice>, AnvilError>` and `refresh_vram()` returning `Result<(u32, u32), AnvilError>`
- Create `crates/anvilml-hardware/src/cpu.rs` implementing `CpuDetector` that returns one `GpuDevice` with `index: 0`, `name: "CPU"`, `device_type: DeviceType::Cpu`, `vram_total_mib: 0`, `vram_free_mib: 0`, `driver_version: "n/a"`
- Add unit tests in `cpu.rs` (filterable via `-- cpu`) verifying CpuDetector returns exactly one device with correct fields
- Update `lib.rs` to export the trait, CpuDetector, and re-export hardware types from anvilml-core for convenience

### Out of Scope
- CUDA detector implementation (P4-A3)
- ROCm detector implementation (P4-A4)
- Mock detector implementation (P4-A2)
- `detect_all_devices` orchestration function (P4-A5)
- HTTP endpoint wiring (P4-A6)
- Any changes to `anvilml-core` types or modules
- Changes to backend launcher binary or server crate

## Approach

1. **Add `sysinfo` dependency to `anvilml-hardware/Cargo.toml`.**
   - Add `sysinfo = "0.32"` (latest stable) under `[dependencies]`. This is the only new external crate needed for this task.
   - The existing `anvilml-core` dependency remains unchanged.

2. **Rewrite `crates/anvilml-hardware/src/lib.rs`.**
   - Remove the existing stub `pub fn stub() {}`.
   - Add `pub mod cpu;` to declare the CPU detector module.
   - Define the `DeviceDetector` trait:
     ```rust
     pub trait DeviceDetector {
         fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;
         fn refresh_vram(&self, idx: u32) -> Result<(u32, u32), AnvilError>;
     }
     ```
   - Re-export `anvilml_core::{GpuDevice, DeviceType, AnvilError}` for ergonomic downstream use.
   - Add a `#[cfg(test)]` module with a compile-check test ensuring the trait object is Send + Sync.

3. **Create `crates/anvilml-hardware/src/cpu.rs`.**
   - Define `pub struct CpuDetector;` (unit struct, no state needed).
   - Implement `DeviceDetector` for `CpuDetector`:
     - `detect()`: returns `Ok(vec![GpuDevice { index: 0, name: "CPU".into(), device_type: DeviceType::Cpu, vram_total_mib: 0, vram_free_mib: 0, driver_version: "n/a".into() }])`
     - `refresh_vram(idx, ...)`: returns `Ok((0, 0))` — CPU has no VRAM to refresh
   - Add `impl Default for CpuDetector` returning `CpuDetector;`
   - Add unit tests:
     - `cpu_detect_returns_one_device`: calls `detect()` and asserts length == 1
     - `cpu_device_fields_correct`: verifies all fields of the returned GpuDevice
     - `cpu_refresh_vram_returns_zero`: verifies `refresh_vram(0)` returns `(0, 0)`
     - `cpu_detector_default`: verifies `CpuDetector::default()` constructs correctly

4. **Verify locally.**
   - Run `cargo test -p anvilml-hardware -- cpu` and confirm exit code 0 with all tests passing.
   - Also run `cargo clippy -p anvilml-hardware` to check for lint issues.

## Files Affected

| Action   | Path                                          | Description                                                  |
|----------|-----------------------------------------------|--------------------------------------------------------------|
| MODIFY   | crates/anvilml-hardware/Cargo.toml            | Add `sysinfo = "0.32"` dependency                            |
| MODIFY   | crates/anvilml-hardware/src/lib.rs            | Replace stub with DeviceDetector trait, module declarations, re-exports |
| CREATE   | crates/anvilml-hardware/src/cpu.rs            | CpuDetector struct + DeviceDetector impl + unit tests         |

## Tests

| Test ID / Name          | File                                        | Validates                                    |
|-------------------------|---------------------------------------------|----------------------------------------------|
| `cpu_detect_returns_one_device` | `crates/anvilml-hardware/src/cpu.rs` | `CpuDetector::detect()` returns exactly 1 GpuDevice |
| `cpu_device_fields_correct` | `crates/anvilml-hardware/src/cpu.rs`   | All fields of the CPU device match spec (index=0, name="CPU", type=Cpu, vram=0, driver="n/a") |
| `cpu_refresh_vram_returns_zero` | `crates/anvilml-hardware/src/cpu.rs` | `refresh_vram(0)` returns `(0, 0)`            |
| `cpu_detector_default`  | `crates/anvilml-hardware/src/cpu.rs`        | `CpuDetector::default()` constructs correctly |
| `trait_is_send_sync`    | `crates/anvilml-hardware/src/lib.rs`         | `DeviceDetector` trait object is Send + Sync   |

## CI Impact

No CI changes required. The existing CI matrix runs `cargo test -p anvilml-hardware --features mock-hardware` as part of the workspace test gate (ARCHITECTURE.md §9). Adding `sysinfo` as a new dependency does not alter any CI steps or workflow definitions. The `-- cpu` filter used in the task acceptance criterion is a standard cargo test filter and works with the existing CI setup.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `sysinfo` crate version incompatibility with target platform | Low | Medium | Use a well-established version (0.32); the crate is cross-platform and widely used. If compilation fails on the CI runner, pin to a known-good version. |
| `AnvilError` lacks a variant for hardware detection errors | Low | Low | `AnvilError::Io` covers I/O failures from sysinfo; `AnvilError::Json` covers parsing failures. No new variants needed for this task. |
| `refresh_vram` signature uses `u32` for VRAM values but sysinfo reports in bytes | Medium | Medium | The trait spec calls for `(u32, u32)` return (total_mib, free_mib). CPU detector returns `(0, 0)` trivially. Subsequent tasks (P4-A5) will handle byte-to-MiB conversion when populating HostInfo via sysinfo. |
| Tests not filterable by `-- cpu` | Low | High | All tests are defined inside `src/cpu.rs` under `#[cfg(test)] mod tests`, so `cargo test -p anvilml-hardware -- cpu` matches the module name prefix automatically. |

## Acceptance Criteria

- [ ] `anvilml-hardware/Cargo.toml` includes `sysinfo = "0.32"` in `[dependencies]`
- [ ] `crates/anvilml-hardware/src/lib.rs` defines `pub trait DeviceDetector` with `detect() -> Result<Vec<GpuDevice>, AnvilError>` and `refresh_vram(&self, idx: u32) -> Result<(u32, u32), AnvilError>`
- [ ] `crates/anvilml-hardware/src/cpu.rs` defines `pub struct CpuDetector` implementing `DeviceDetector`
- [ ] `CpuDetector::detect()` returns exactly one `GpuDevice` with `index: 0`, `name: "CPU"`, `device_type: DeviceType::Cpu`, `vram_total_mib: 0`, `vram_free_mib: 0`, `driver_version: "n/a"`
- [ ] `CpuDetector::refresh_vram()` returns `Ok((0, 0))` for any index
- [ ] `cargo test -p anvilml-hardware -- cpu` exits 0 with all tests passing
