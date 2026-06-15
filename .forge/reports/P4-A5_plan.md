# Plan Report: P4-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P4-A5                                             |
| Phase       | 004 — Hardware Detection                          |
| Description | anvilml-hardware: detect_all_devices orchestration function |
| Depends on  | P4-A1, P4-A2, P4-A3, P4-A4                        |
| Project     | anvilml                                           |
| Planned at  | 2026-06-15T10:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo, AnvilError>` in `anvilml-hardware/src/lib.rs`. This function orchestrates the full hardware detection pipeline with a defined priority chain: hardware override → mock (when `mock-hardware` feature is active) → Vulkan → platform fallbacks (DXGI on Windows, sysfs on Linux) → CPU. For each GPU detected, it resolves capabilities via the PCI-ID device database and populates `HostInfo` using `sysinfo`. Every detected device emits a `tracing::info!` log with `index`, `name`, `device_type`, `vram_total_mib`, and `fp8` fields. The acceptance criterion is that `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with the mock detector returning a single CUDA device when `ANVILML_MOCK_DEVICE_TYPE=cuda`.

## Scope

### In Scope
- **`crates/anvilml-hardware/src/lib.rs`** — Add `sqlx` dependency; implement `detect_all_devices()` async function with full priority chain; add `mock` module conditional compilation; export `MockDetector` behind `#[cfg(feature = "mock-hardware")]`; re-export `sqlx::SqlitePool` type alias or use it directly.
- **`crates/anvilml-hardware/src/mock.rs`** — New file: `MockDetector` struct implementing `DeviceDetector`, reading `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_DEVICE_NAME` env vars; returns one synthetic `GpuDevice` or empty vec.
- **`crates/anvilml-hardware/Cargo.toml`** — Add `sqlx` dependency (workspace version 0.9.0 with `runtime-tokio`, `sqlite`, `json` features).
- **`crates/anvilml-hardware/tests/mock_tests.rs`** — New test file: integration tests for `MockDetector` and `detect_all_devices` with mock-hardware feature.
- **`crates/anvilml-hardware/Cargo.toml`** — Bump patch version from `0.1.4` to `0.1.5`.

### Out of Scope
- Real Vulkan/sysfs/DXGI detection logic (implemented in P4-A2, P4-A3).
- Device DB seeding via SQL (the pool parameter is accepted but the actual seed SQL is deferred; the function logs a `DEBUG` note when the pool is passed and devices are detected).
- `GET /v1/system` handler wiring (P4-C1).
- VRAM refresh via NVML (P4-A3).
- Mock tests for ROCm and CPU device types (handled in P4-B1 which adds ≥8 mock tests).

## Existing Codebase Assessment

The `anvilml-hardware` crate already has the `DeviceDetector` trait defined in `lib.rs` with `detect()` and `refresh_vram()` methods. Concrete implementations exist: `CpuDetector` (cpu.rs, uses `sysinfo`), `VulkanDetector` (vulkan.rs, uses `ash`), `DxgiDetector` (dxgi.rs, Windows-only), `SysfsPciDetector` (sysfs.rs, Unix-only), and `NvmlDetector` (nvml.rs, Unix-only, VRAM supplement only). The `device_db.rs` module provides `resolve_caps_from_row()` and the `DEVICE_DB` constant with 15 curated PCI-ID entries.

The `anvilml-core` crate defines all domain types: `HardwareInfo`, `GpuDevice`, `HostInfo`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`, and `ServerConfig` (with `hardware_override: Option<HardwareOverrideConfig>`). `ServerConfig::default()` sets `hardware_override: None`.

The `SqlitePool` type does not yet exist as a dependency of `anvilml-hardware`. The `anvilml-registry` crate is a stub (only `pub fn stub()`). The workspace declares `sqlx = { version = "0.9.0", features = ["runtime-tokio", "sqlite", "json"] }` in `[workspace.dependencies]`.

Established patterns: all detectors are zero-sized unit structs with `new()` and `default()` constructors; they implement `DeviceDetector`; they never panic — failures return `Ok(vec![])` or `Ok((0, 0))`; logging follows the mandatory INFO log point for each detected device with structured fields; test files use `#[serial_test::serial]` annotation; `lib.rs` contains only `pub mod`, `pub use`, and the crate-level `//!` doc comment (≤ 80 lines).

Gap: `MockDetector` does not exist yet — it is created by this task (and referenced by P4-B1 for its test suite). The `sqlx` dependency is absent from `anvilml-hardware`'s `Cargo.toml` and must be added.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | sqlx       | 0.9.0           | Cargo.lock     | runtime-tokio, sqlite, json |
| crate  | serial_test| 3.5             | Cargo.lock (dev-dep) | n/a |

Note: `sqlx` 0.9.0 is the workspace-level version already declared in the root `Cargo.toml`. The `SqlitePool` type is `sqlx::SqlitePool` — a type alias for `sqlx::pool::Pool<sqlx::Sqlite>`. This type has been present since sqlx 0.7.x and is stable in 0.9.0.

## Approach

1. **Add `sqlx` dependency to `anvilml-hardware/Cargo.toml`.**
   Add `sqlx = { workspace = true }` to `[dependencies]`. This brings in `sqlx 0.9.0` with the workspace features (`runtime-tokio`, `sqlite`, `json`). The `SqlitePool` type is available as `sqlx::SqlitePool`.

2. **Create `crates/anvilml-hardware/src/mock.rs`.**
   Implement `MockDetector` as a zero-sized unit struct behind `#[cfg(feature = "mock-hardware")]`. It implements `DeviceDetector`:
   - `detect()`: Read env vars `ANVILML_MOCK_DEVICE_TYPE` (default `"cpu"`), `ANVILML_MOCK_VRAM_MIB` (default `8192`), `ANVILML_MOCK_DEVICE_NAME` (default `"Mock GPU"`). Map the device type string to `DeviceType` variant (`"cuda"` → `Cuda`, `"rocm"` → `Rocm`, `"cpu"` → `Cpu`). Build a single `GpuDevice` with `enumeration_source = EnumerationSource::Mock`, `capabilities_source = CapabilitySource::Fallback`, `arch = None`, `caps = InferenceCaps::default()`, `driver_version = "mock"`, PCI IDs = 0. Log at INFO with `index=0`, `name`, `device_type`, `vram_total_mib`, `fp8=false`. Return `Ok(vec![device])`. If the env var value is invalid (not one of `"cuda"`, `"rocm"`, `"cpu"`), return `Ok(vec![])` (graceful fallback).
   - `refresh_vram(_index)`: Return `Ok((0, 0))` — mock has no live VRAM.
   Include `///` doc comments on the struct and both methods per §12.1.

3. **Implement `detect_all_devices` in `lib.rs`.**
   Add the following async function:

   ```rust
   pub async fn detect_all_devices(
       cfg: &ServerConfig,
       pool: &SqlitePool,
   ) -> Result<HardwareInfo, AnvilError>
   ```

   The function follows this priority chain:

   a. **Hardware override check** — If `cfg.hardware_override.is_some()`, construct a synthetic `GpuDevice` from `HardwareOverrideConfig`: use `device_type` field (mapped to `DeviceType`), `vram_total_mib` from config, `enumeration_source = EnumerationSource::Override`, `capabilities_source = CapabilitySource::Fallback`. Resolve caps via `resolve_caps_from_row(&mut dev, None)` to look up the PCI-ID table (though override devices have synthetic PCI IDs, so caps remain at defaults). Log at INFO.

   b. **Mock detector** — If `#[cfg(feature = "mock-hardware")]`, instantiate `MockDetector::new()` and call `detect()`. If it returns non-empty devices, skip to step d. If empty, fall through to real detection.

   c. **Vulkan detection** — Instantiate `VulkanDetector::new()` and call `detect()`. If it returns non-empty devices, proceed to step d. If empty, try platform fallbacks: on Windows, `DxgiDetector::new().detect()`; on Unix, `SysfsPciDetector::new().detect()`. If all real detection paths return empty, proceed to CPU fallback.

   d. **CPU fallback** — Always instantiate `CpuDetector::new()` and call `detect()`. This always returns exactly one CPU device. Merge the CPU device into the device list (appending after GPU devices).

   e. **Resolve capabilities** — For each GPU device in the list (not the CPU device), call `resolve_caps_from_row(&mut dev, None)` to look up the PCI-ID table and populate `arch`, `caps`, and canonical `name`. Set `capabilities_source = CapabilitySource::DeviceTable` for matched devices.

   f. **Populate HostInfo** — Use `sysinfo::System` (same approach as `CpuDetector::detect()`) to read OS version, CPU brand, and total RAM. Build `HostInfo { os, cpu, ram_total_mib }`.

   g. **Build HardwareInfo** — Construct `HardwareInfo { host, gpus, inference_caps }`. Compute `inference_caps` as the union of all `GpuDevice.caps` values (OR across all fields).

   h. **Seed device DB** — Use `pool` to insert device capability rows. Since `anvilml-registry` is not yet implemented, this step executes a minimal seed: run `INSERT OR IGNORE` for each known PCI ID from `DEVICE_DB` into a `device_capabilities` table. If the table doesn't exist, log at DEBUG and skip (the registry task will create it). Use `sqlx::query!` or `sqlx::query()` with parameterised inserts.

   i. **Return** `Ok(HardwareInfo)`.

   Every detected device (GPU and CPU) emits `tracing::info!(index=..., name=..., device_type=..., vram_total_mib=..., fp8=..., "...")` per the mandatory INFO log point table.

   Apply `#[tracing::instrument]` to the function per §11.6.

4. **Update `lib.rs` module declarations.**
   Add `#[cfg(feature = "mock-hardware")] pub mod mock;` to the module declarations. Add `#[cfg(feature = "mock-hardware")] pub use mock::MockDetector;` to the re-exports. Update the crate-level `//!` doc comment to mention `MockDetector`.

5. **Create `crates/anvilml-hardware/tests/mock_tests.rs`.**
   Write integration tests for `MockDetector` and `detect_all_devices`:
   - `test_mock_detect_cuda`: Set `ANVILML_MOCK_DEVICE_TYPE=cuda`, verify device has `DeviceType::Cuda`, correct VRAM, `EnumerationSource::Mock`.
   - `test_mock_detect_rocm`: Same with `rocm`.
   - `test_mock_detect_cpu`: Same with `cpu`.
   - `test_mock_detect_invalid_type`: Set invalid type, verify returns empty vec (graceful).
   - `test_detect_all_devices_mock_cuda`: Full pipeline with mock-hardware feature and `ANVILML_MOCK_DEVICE_TYPE=cuda`, verify `HardwareInfo` has one GPU and one CPU.
   - `test_detect_all_devices_hardware_override`: With `ServerConfig { hardware_override: Some(...) }`, verify override takes priority over mock.
   - `test_detect_all_devices_cpu_fallback`: With no GPUs detected (mock returns empty), verify CPU device is always present.
   - `test_detect_all_devices_inference_caps_union`: Verify `inference_caps` is the union of all GPU caps.
   All tests use `#[serial]` because they mutate process-global env vars. Each test captures and restores env vars unconditionally.

6. **Bump version.** Update `crates/anvilml-hardware/Cargo.toml` patch version from `0.1.4` to `0.1.5`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `MockDetector` | struct | `anvilml_hardware::mock` | `pub struct MockDetector;` (behind `mock-hardware` feature) |
| `MockDetector::new` | fn | `anvilml_hardware::mock` | `pub const fn new() -> Self` |
| `MockDetector::detect` | fn (trait impl) | `anvilml_hardware::mock` | `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` |
| `MockDetector::refresh_vram` | fn (trait impl) | `anvilml_hardware::mock` | `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` |
| `detect_all_devices` | async fn | `anvilml_hardware` (lib root) | `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo, AnvilError>` |

New `pub use` re-exports in `lib.rs`:
- `pub use mock::MockDetector;` (behind `#[cfg(feature = "mock-hardware")]`)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Add `sqlx` workspace dependency; bump version 0.1.4 → 0.1.5 |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `detect_all_devices()` function; add mock module declaration and re-export; update crate doc comment |
| CREATE | `crates/anvilml-hardware/src/mock.rs` | New file: `MockDetector` struct and `DeviceDetector` impl |
| CREATE | `crates/anvilml-hardware/tests/mock_tests.rs` | New file: integration tests for mock detection pipeline |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/mock_tests.rs` | `test_mock_detect_cuda` | MockDetector with `ANVILML_MOCK_DEVICE_TYPE=cuda` returns one CUDA device | mock-hardware feature enabled | env: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=16384`, `ANVILML_MOCK_DEVICE_NAME=Mock CUDA` | One `GpuDevice` with `device_type=Cuda`, `vram_total_mib=16384`, `enumeration_source=Mock` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_cuda` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_rocm` | MockDetector with `ANVILML_MOCK_DEVICE_TYPE=rocm` returns one ROCm device | mock-hardware feature enabled | env: `ANVILML_MOCK_DEVICE_TYPE=rocm` | One `GpuDevice` with `device_type=Rocm` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_rocm` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_cpu` | MockDetector with `ANVILML_MOCK_DEVICE_TYPE=cpu` returns one CPU-type mock device | mock-hardware feature enabled | env: `ANVILML_MOCK_DEVICE_TYPE=cpu` | One `GpuDevice` with `device_type=Cpu` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_cpu` exits 0 |
| `tests/mock_tests.rs` | `test_mock_detect_invalid_type` | MockDetector with invalid device type returns empty vec (graceful fallback) | mock-hardware feature enabled | env: `ANVILML_MOCK_DEVICE_TYPE=invalid` | Empty `Vec<GpuDevice>` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_invalid_type` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_mock_cuda` | Full pipeline: `detect_all_devices` with mock-hardware + cuda returns one CUDA GPU + one CPU | mock-hardware feature, env set | `ANVILML_MOCK_DEVICE_TYPE=cuda` | `HardwareInfo` with `gpus.len() >= 1` (CUDA GPU), CPU device appended, `host` populated | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_mock_cuda` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_hardware_override` | Hardware override takes priority over mock detector | mock-hardware feature, override config | `ServerConfig { hardware_override: Some(...) }` | `HardwareInfo` with override device, not mock device | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_hardware_override` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_cpu_fallback` | CPU device always present even when GPU detection returns empty | mock-hardware, mock returns empty (invalid type) | `ANVILML_MOCK_DEVICE_TYPE=invalid` | `HardwareInfo` has at least one CPU device | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_cpu_fallback` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_inference_caps_union` | `inference_caps` is union of all GPU caps | mock-hardware with a device that has fp8=true in device_db | Mock CUDA device (PCI ID matches RTX 4090 in DEVICE_DB) | `inference_caps.fp8 = true`, `flash_attention = true` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_inference_caps_union` exits 0 |
| `tests/mock_tests.rs` | `test_detect_all_devices_returns_ok` | `detect_all_devices` always returns `Ok` (never `Err`) under mock feature | mock-hardware feature enabled | Any valid mock config | `Result<HardwareInfo, AnvilError>` is `Ok` | `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_returns_ok` exits 0 |

## CI Impact

No CI changes required. The `mock-hardware` feature is already declared and forwarded by all dependent crates (`anvilml-worker`, `anvilml-scheduler`, `anvilml-server`, `backend`). The CI test command `cargo test --workspace --features mock-hardware` already exercises the `anvilml-hardware` crate with this feature. Adding a new test file under `crates/anvilml-hardware/tests/` is automatically picked up by the Rust test runner.

## Platform Considerations

None identified. The `detect_all_devices` function and `MockDetector` are platform-neutral — they do not use any `#[cfg(unix)]` or `#[cfg(windows)]` guards. The mock detector reads env vars and constructs synthetic data. The `sqlx` dependency is cross-platform (SQLite is embedded). The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::SqlitePool` type may have a different path or name in sqlx 0.9.0 than expected. The workspace declares `sqlx = { version = "0.9.0", features = ["runtime-tokio", "sqlite", "json"] }` but the exact type path (`sqlx::SqlitePool` vs `sqlx::pool::Pool<sqlx::Sqlite>`) needs confirmation. | Medium | High | At ACT time, confirm the type exists via `rust-docs MCP` or by reading `sqlx` crate source. If the path differs, use the correct path. The type alias `type SqlitePool = sqlx::SqlitePool` in `lib.rs` provides a stable reference. |
| The pool parameter is unused in the seed logic if the `device_capabilities` table doesn't exist yet, triggering a `dead_code` warning for `pool`. | High | Medium | Use `let _ = pool;` after the detection chain to suppress the warning, or implement a minimal `sqlx::query!("SELECT 1")` ping. Better: use `#[allow(dead_code)]` on the parameter with an inline comment explaining the seed logic is deferred to P4-B1. |
| Mock tests that mutate `ANVILML_MOCK_*` env vars may interfere with each other under parallel test execution. | Medium | Medium | All mock tests use `#[serial_test::serial]` annotation. Each test captures pre-existing env values and restores them unconditionally in a `drop` guard or `match prior { ... }` pattern per ENVIRONMENT.md §11.3. |
| `detect_all_devices` is `async` but the mock/hardware detection paths are synchronous (no async I/O). The async marker is needed for future pool operations but adds unnecessary overhead in the mock path. | Low | Low | The `async` is dictated by the design doc signature. The ACT agent should keep it as-is — the overhead is negligible (one task spawn). If clippy warns about unnecessary async, suppress with `#[allow(clippy::redundant_async_block)]`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_mock_cuda -- --nocapture` exits 0 and outputs a CUDA device with correct fields
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `head -1 .forge/reports/P4-A5_plan.md` prints `# Plan Report: P4-A5`
- [ ] `grep "^## " .forge/reports/P4-A5_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P4-A5_plan.md` outputs a value > 40
