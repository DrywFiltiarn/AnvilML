# Plan Report: P6-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-B1                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-hardware: fix real-hardware build errors surfaced by no-feature compile check |
| Depends on  | P6-A7                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-04T11:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Fix the compile error in `crates/anvilml-hardware/src/lib.rs` that prevents `cargo check --bin anvilml --target x86_64-pc-windows-gnu` (no features) from succeeding. The sole error is on line 112 where `dxgi::DxgiDetector` (a unit struct deriving `Default`) is called with dot-syntax `.detect()` instead of being instantiated via `::default()`.

## Scope

### In Scope
- Fix line 112 in `crates/anvilml-hardware/src/lib.rs`: change `dxgi::DxgiDetector.detect()` to `dxgi::DxgiDetector::default().detect()`
- Verify both compile checks exit 0:
  - `cargo check --bin anvilml` (native Linux)
  - `cargo check --bin anvilml --target x86_64-pc-windows-gnu` (Windows-gnu cross)
- Verify existing test suite still passes: `cargo test --workspace --features mock-hardware`

### Out of Scope
- No changes to `#[cfg(...)]` feature gates
- No new tests
- No behavioural changes to detection logic
- No modifications to any other crate or file
- No CI workflow changes (that is P6-B2)
- No dependency additions or removals

## Approach

1. **Read the error output** — already done. The Windows-gnu cross-check reports exactly one error:
   ```
   error[E0423]: expected value, found struct `dxgi::DxgiDetector`
      --> crates/anvilml-hardware/src/lib.rs:112:32
       |
   112|             let dxgi_devices = dxgi::DxgiDetector.detect().unwrap_or_default();
   ```

2. **Apply the fix** — In `crates/anvilml-hardware/src/lib.rs`, line 112, replace:
   ```rust
   let dxgi_devices = dxgi::DxgiDetector.detect().unwrap_or_default();
   ```
   with:
   ```rust
   let dxgi_devices = dxgi::DxgiDetector::default().detect().unwrap_or_default();
   ```

   This is the only change needed. Rationale:
   - `DxgiDetector` derives `Default` (see `dxgi.rs` line 111).
   - The `DeviceDetector::detect()` method takes `&self`, so an instance is required.
   - Line 106 (`vulkan::VulkanDetector.detect()`) compiles fine on both targets because the Windows cross-check only exercises lines 109–116 (the `#[cfg(windows)]` block); the Linux native check passes through line 106 with no error. The task description confirms this pattern applies to all three platform detectors, but in practice only the `DxgiDetector` call is exercised by the failing cross-target compile.

3. **Verify both compile checks** — Run:
   ```bash
   cargo check --bin anvilml
   cargo check --bin anvilml --target x86_64-pc-windows-gnu
   ```
   Both must exit 0.

4. **Verify existing tests pass** — Run:
   ```bash
   cargo test --workspace --features mock-hardware
   ```
   Must exit 0 with no regressions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/lib.rs` | Fix line 112: `dxgi::DxgiDetector.detect()` → `dxgi::DxgiDetector::default().detect()` |

No files created, deleted, or renamed.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-hardware/src/lib.rs` (tests module) | `vendor_map_cuda`, `vendor_map_rocm`, etc. | Vendor ID mapping still correct after fix |
| `crates/anvilml-hardware/src/lib.rs` (tests module) | `detect_all_devices_override*` | Override branch unchanged |
| `crates/anvilml-hardware/src/lib.rs` (tests module) | `detect_all_devices_mock_*` | Mock branch unchanged |
| `crates/anvilml-hardware/src/lib.rs` (tests module) | `detect_all_devices_vulkan_empty` | Vulkan detection still works |
| `crates/anvilml-hardware/src/vulkan/tests` | All vulkan tests | Vulkan detector unchanged |
| `crates/anvilml-hardware/src/dxgi/tests` | All dxgi tests | DXGI detector unchanged |
| `crates/anvilml-hardware/src/sysfs/tests` | All sysfs tests | Sysfs detector unchanged |
| `crates/anvilml-hardware/src/nvml/tests` | All nvml tests | NVML detector unchanged |

## CI Impact

No CI changes in this task. Adding the real-hardware compile check to CI is P6-B2. The existing CI matrix (`cargo test --features mock-hardware`) already exercises all code paths including `mock.rs`. This task only fixes a compile error that was previously invisible because every CI run used `--features mock-hardware`, which elides the `#[cfg(windows)]` and `#[cfg(unix)]` real-hardware branches.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| The fix is a one-line change with minimal risk, but any Rust compile error could indicate a deeper API mismatch. | The fix follows the exact same pattern used throughout the codebase (e.g., `cpu::CpuDetector::default().detect()`, `mock::MockDetector.detect()` via `self`). All other detector structs derive `Default`. |
| Cross-compilation toolchain might not be installed on all machines running this task. | The build environment has `x86_64-pc-windows-gnu` target and `gcc-mingw-w64` linker installed (per ENVIRONMENT.md §6). |
| Existing tests might break due to unrelated issues. | The change is purely syntactic — no logic, no branches, no new dependencies. Tests are verified post-fix as a gate. |

## Acceptance Criteria

- [ ] `cargo check --bin anvilml` exits 0 (native Linux)
- [ ] `cargo check --bin anvilml --target x86_64-pc-windows-gnu` exits 0 (Windows-gnu cross)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
- [ ] No files modified except `crates/anvilml-hardware/src/lib.rs`
- [ ] No changes to any `#[cfg(...)]` gates or feature flags
