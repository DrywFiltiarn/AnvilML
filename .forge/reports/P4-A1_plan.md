# Plan Report: P4-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A1                                         |
| Phase       | 004 — Hardware Detection                      |
| Description | anvilml-hardware: DeviceDetector trait + CpuDetector |
| Depends on  | P3 (Phase 003 complete — HardwareInfo, GpuDevice, AnvilError types exist) |
| Project     | anvilml                                       |
| Planned at  | 2026-06-15T00:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Define the `DeviceDetector` trait in `anvilml-hardware/src/lib.rs` and implement `CpuDetector` in a new `cpu.rs` module that returns one synthetic CPU `GpuDevice` entry populated from `sysinfo`. This establishes the interface that all subsequent detectors (Vulkan, DXGI, sysfs, NVML, mock) will implement, and provides the always-available CPU fallback device. The observable state when complete: `cargo test -p anvilml-hardware -- cpu` exits 0, and the trait is ready for downstream crates to depend on via `use anvilml_hardware::DeviceDetector`.

## Scope

### In Scope
- **CREATE** `crates/anvilml-hardware/src/cpu.rs`: `CpuDetector` struct and `impl DeviceDetector` for it. `detect()` returns `Ok(vec![GpuDevice])` with one synthetic CPU entry. `refresh_vram()` returns `Ok((0, 0))`.
- **MODIFY** `crates/anvilml-hardware/src/lib.rs`: Add `pub mod cpu;`, `pub trait DeviceDetector: Send + Sync` with `detect()` and `refresh_vram()` methods, `pub use` for the trait, crate-level doc comment update.
- **MODIFY** `crates/anvilml-hardware/Cargo.toml`: Add `sysinfo` dependency and `serial_test` dev-dependency. Bump patch version.
- **CREATE** `crates/anvilml-hardware/tests/cpu_tests.rs`: Integration tests for `CpuDetector`.
- Update `docs/TESTS.md` with new test entries.

### Out of Scope
- Vulkan, DXGI, sysfs, NVML, or Mock detector implementations (future tasks P4-A2 through P4-B1).
- `detect_all_devices` orchestration function (P4-A5).
- device_db.rs PCI-ID capability table (P4-A4).
- Any changes to `anvilml-core` types (already exist from Phase 003).
- Any changes to `anvilml-server`, `anvilml-worker`, or `anvilml-scheduler`.

## Existing Codebase Assessment

The `anvilml-hardware` crate currently contains only a stub `lib.rs` with a `pub fn stub()` and a doc comment describing the full scope of the crate. No other source files exist yet — `cpu.rs`, `vulkan.rs`, `dxgi.rs`, and all other detector modules are absent. The `tests/` directory does not exist.

The domain types required by this task already exist in `anvilml-core`:
- `GpuDevice` (13 fields) in `crates/anvilml-core/src/types/hardware.rs` — fully defined with all fields including `enumeration_source: EnumerationSource::Override` (which is the right source for a CPU synthetic device).
- `DeviceType::Cpu` — already defined.
- `HostInfo` (os, cpu, ram_total_mib) — already defined.
- `AnvilError` — fully defined with all 14 variants.
- `InferenceCaps` — defaults via `Default` derive (all `false`).
- `EnumerationSource::Override` and `CapabilitySource::Fallback` — both available.

The established patterns in the codebase:
- Error handling uses `Result<T, AnvilError>` with `?` propagation; no `.unwrap()` in production code.
- Public items have `///` doc comments describing what they do, preconditions, and return types.
- Crate `lib.rs` contains only `pub mod`, `pub use`, and `//!` doc comment — no implementation code.
- Tests go in `crates/{name}/tests/` as separate test crate files.
- The `mock-hardware` feature follows the forwarding pattern declared in the workspace.

No gap or discrepancy was found between the design doc (ANVILML_DESIGN.md §6.4) and the current source — the design doc's trait definition matches what needs to be implemented, and all required types already exist.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | sysinfo    | 0.33.0          | crates.io (MCP unavailable; version from crates.io API fallback) | n/a |
| crate  | serial_test | 3.5.0          | Cargo.lock (already in workspace) | n/a |

Note: `serial_test 3.5.0` is already present in `Cargo.lock`. `sysinfo` is a new dependency not yet in the lockfile. The rust-docs MCP tool was unavailable for live verification; the version 0.33.0 was obtained from the crates.io API. At ACT time, the acting agent must confirm this version via MCP before writing the manifest entry.

## Approach

1. **Add `sysinfo` dependency to `crates/anvilml-hardware/Cargo.toml`.** Add `sysinfo = "0.33"` under `[dependencies]`. This provides `System`, `HostInfo`, and `ProcessorInfo` APIs to populate `HostInfo` and CPU device metadata. Rationale: `sysinfo 0.33` is the latest stable that supports Rust 2021 edition and provides the `HostInfo::os_version()` / `cpu_brand()` / `ram_total()` methods needed to populate the domain types.

2. **Add `serial_test` dev-dependency to `crates/anvilml-hardware/Cargo.toml`.** Add `serial_test = "3.5"` under `[dev-dependencies]`. This is needed because `CpuDetector::detect()` reads system info which is process-global state — tests must be serialised to avoid interference. Rationale: `serial_test` is already in the workspace lockfile, so no version resolution is needed.

3. **Create `crates/anvilml-hardware/src/cpu.rs`.** Define `pub struct CpuDetector` (zero-sized, unit struct — it holds no state). Implement `DeviceDetector` for `CpuDetector`:
   - `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>`: Use `sysinfo::System::new_all()` to read host info. Build one `GpuDevice` with `index = 0`, `name = "CPU (synthetic)"`, `device_type = DeviceType::Cpu`, `vram_total_mib = 0`, `vram_free_mib = 0`, `driver_version = "n/a"`, `pci_vendor_id = 0`, `pci_device_id = 0`, `arch = None`, `caps = InferenceCaps::default()`, `enumeration_source = EnumerationSource::Override`, `capabilities_source = CapabilitySource::Fallback`. Log at DEBUG: "cpu device synthesised".
   - `fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError>`: Return `Ok((0, 0))` — CPUs have no VRAM. Log at DEBUG: "refresh_vram returns (0,0) for CPU".
   - Add `///` doc comments on the struct and both methods per §12.1 documentation obligation.

4. **Modify `crates/anvilml-hardware/src/lib.rs`.** Replace the stub with:
   - Update the `//!` crate-level doc comment to mention `CpuDetector` and the `DeviceDetector` trait.
   - Add `pub mod cpu;` to declare the module.
   - Add `pub trait DeviceDetector: Send + Sync` with:
     - `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;` — enumerate available devices.
     - `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;` — refresh VRAM for device at index.
   - Add `pub use cpu::CpuDetector;` and `pub use crate::DeviceDetector;`.
   - Ensure the file stays under 80 lines and contains only `//!`, `pub mod`, and `pub use`/`pub trait` items — no implementation code.

5. **Create `crates/anvilml-hardware/tests/cpu_tests.rs`.** Write integration tests:
   - `test_cpu_detector_detect_returns_one_device`: Create `CpuDetector`, call `detect()`, verify the returned vec has exactly one element, that `device_type == DeviceType::Cpu`, and that `index == 0`.
   - `test_cpu_detector_refresh_vram_returns_zero`: Create `CpuDetector`, call `refresh_vmar(0)`, verify it returns `Ok((0, 0))`.
   - `test_cpu_detector_is_send_sync`: Static assertion that `CpuDetector` implements `Send + Sync` (compile-time check via `fn assert_send_sync<T: Send + Sync>() {}`).
   - Mark tests with `#[serial]` to prevent env/state interference from concurrent test threads.

6. **Bump `anvilml-hardware` patch version** in `Cargo.toml` from `0.1.0` to `0.1.1` per §14 of FORGE_AGENT_RULES and §12 of ENVIRONMENT.md.

7. **Update `docs/TESTS.md`** with entries for the three new tests per §5.10 (test catalogue sync).

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `DeviceDetector` | trait | `anvilml_hardware` (lib.rs) | `pub trait DeviceDetector: Send + Sync { fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>; fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>; }` |
| `CpuDetector` | struct | `anvilml_hardware::cpu` | `pub struct CpuDetector;` (unit struct) |
| `impl DeviceDetector for CpuDetector` | impl | `anvilml_hardware::cpu` | `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` |
| `impl DeviceDetector for CpuDetector` | impl | `anvilml_hardware::cpu` | `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-hardware/src/cpu.rs | CpuDetector struct + impl DeviceDetector |
| MODIFY | crates/anvilml-hardware/src/lib.rs | Add DeviceDetector trait, pub mod cpu, pub use items |
| CREATE | crates/anvilml-hardware/tests/cpu_tests.rs | Integration tests for CpuDetector |
| MODIFY | crates/anvilml-hardware/Cargo.toml | Add sysinfo dep, serial_test dev-dep, bump version to 0.1.1 |
| MODIFY | docs/TESTS.md | Add test catalogue entries for cpu_tests.rs tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| crates/anvilml-hardware/tests/cpu_tests.rs | test_cpu_detector_detect_returns_one_device | CpuDetector::detect() returns exactly one GpuDevice with device_type=Cpu and index=0 | None (sysinfo available) | None | Ok([GpuDevice { device_type: Cpu, index: 0, ... }]) | `cargo test -p anvilml-hardware -- cpu --test-threads=1` exits 0 |
| crates/anvilml-hardware/tests/cpu_tests.rs | test_cpu_detector_refresh_vram_returns_zero | CpuDetector::refresh_vram() returns (0, 0) for any index | None | index=0 | Ok((0, 0)) | `cargo test -p anvilml-hardware -- cpu --test-threads=1` exits 0 |
| crates/anvilml-hardware/tests/cpu_tests.rs | test_cpu_detector_is_send_sync | CpuDetector implements Send + Sync (compile-time check) | None | None | Compiles successfully | `cargo test -p anvilml-hardware -- cpu --test-threads=1` exits 0 |

## CI Impact

No CI changes required. The new test file lives under `crates/anvilml-hardware/tests/` which is already picked up by the workspace test suite (`cargo test --workspace --features mock-hardware`). No new file types, gates, or test modules are introduced — just a new test binary for the existing crate.

## Platform Considerations

None identified. The `sysinfo` crate is cross-platform and provides the same API on Linux, Windows, and macOS. The synthetic CPU device does not use any platform-specific paths, syscalls, or file reads. The `#[cfg(unix)]` / `#[cfg(windows)]` guards are not needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sysinfo` 0.33 API shape (method names, struct field names) differs from what the plan assumes — e.g., `System::new_all()` may be `System::new_with_specifics()` in the actual version. The MCP tool was unavailable for live verification. | Medium | High | At ACT time, before writing any code, confirm the `sysinfo` API surface via MCP or `cargo add sysinfo --dry-run`. If method names differ, adjust the plan's Approach step 3 to use the actual API. |
| `sysinfo::System::new_all()` is slow (~100ms) because it reads all system info. For a test that runs on every `cargo test`, this adds measurable wall-clock time. | Low | Low | The test runs once per test process. The 100ms overhead is acceptable for a single test. If it becomes a problem, cache the `System` instance or use `new_with_specifics()` for only the fields needed. |
| `AnvilError` does not have a variant that maps naturally to a sysinfo read failure (e.g., permission denied on `/proc/meminfo`). The plan uses `Internal` as fallback. | Low | Medium | Use `AnvilError::Internal(sysinfo_error.to_string())` for any unexpected sysinfo error. This is appropriate because sysinfo failures are internal operational errors, not client errors. |
| `docs/TESTS.md` does not exist yet (Phase 004 is early in the build). Creating it from scratch is required. | Low | Low | Create `docs/TESTS.md` with entries covering only the tests introduced by this task, using the format defined in ANVILML_DESIGN.md §16.1. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- cpu` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings from this task)
- [ ] `head -1 .forge/reports/P4-A1_plan.md` prints `# Plan Report: P4-A1`
- [ ] `grep "^## " .forge/reports/P4-A1_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P4-A1_plan.md` reports > 40 lines
