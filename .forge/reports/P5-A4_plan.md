# Plan Report: P5-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A4                                       |
| Phase       | 005 — Hardware Detection: Orchestration     |
| Description | anvilml-hardware: lib.rs re-export detect_all_devices, 80-line check |
| Depends on  | P5-A1, P5-A2, P5-A3                         |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T12:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Finalize the `anvilml-hardware` crate's public surface by re-exporting `detect_all_devices` from the `detect` module, and confirm every module from Phase 4 and this phase (`detect`, `cpu`, `mock`, `vulkan`, `dxgi`, `sysfs`) is correctly declared with its `cfg`/feature gate. This is a re-export and gate-verification pass only — no implementation logic changes. The file must remain under the 80-line hard cap.

## Scope

### In Scope
- Add `pub use detect::detect_all_devices;` alongside the existing `pub use detect::DeviceDetector;` in `crates/anvilml-hardware/src/lib.rs`.
- Confirm all six module declarations (`detect`, `cpu`, `mock`, `vulkan`, `dxgi`, `sysfs`) are present with correct `cfg`/feature gates per `ANVILML_DESIGN.md §6.3`'s module layout table.
- Verify the file stays ≤80 lines.
- Build with `--features mock-hardware` and with no features — both must exit 0.

### Out of Scope
defers_to (from JSON): []

No deferrals are permitted — this task's `defers_to` field is empty. All scope described in the task context is in scope.

## Existing Codebase Assessment

The `anvilml-hardware` crate already has all six modules declared in `lib.rs` (24 lines): `detect`, `cpu`, `vulkan` are unconditional; `mock` is gated on `feature = "mock-hardware"`; `dxgi` is gated on `target_os = "windows"`; `sysfs` is gated on `target_os = "linux"`. The `detect.rs` module contains the `DeviceDetector` trait and the fully-implemented `detect_all_devices()` function (313 lines), built up across P5-A1, P5-A2, and P5-A3.

The established pattern for `lib.rs` in this project is: crate-level `//!` doc comment, `pub mod` declarations, `pub use` re-exports for key types, and `#[cfg(...)]` gates on platform/feature-specific modules. The file currently has 24 lines — well under the 80-line hard cap.

No gap exists between the design doc and current source: `ANVILML_DESIGN.md §6.3` lists exactly the six modules present in the source tree, and the gates match the design's specification (mock behind feature flag, dxgi/sysfs behind target_os gates).

## Resolved Dependencies

None. This task introduces no new dependencies — it only adds a `pub use` re-export statement.

## Approach

1. **Read the current `lib.rs`** (already read at `/home/dryw/AnvilML/crates/anvilml-hardware/src/lib.rs`, 24 lines).

2. **Add the `detect_all_devices` re-export.** Insert `pub use detect::detect_all_devices;` on a new line immediately after the existing `pub use detect::DeviceDetector;` line (line 7). This places both `detect` module re-exports together, following the established pattern.

3. **Verify all six module declarations** are present with correct gates per `ANVILML_DESIGN.md §6.3`:
   - `pub mod detect;` — unconditional ✓
   - `pub mod cpu;` — unconditional ✓
   - `pub mod vulkan;` — unconditional ✓
   - `pub mod mock;` — `#[cfg(feature = "mock-hardware")]` ✓
   - `pub mod dxgi;` — `#[cfg(target_os = "windows")]` ✓
   - `pub mod sysfs;` — `#[cfg(target_os = "linux")]` ✓

4. **Verify the file length.** After adding one line (the new re-export), the file will be 25 lines — well under the 80-line hard cap.

5. **Build with mock-hardware feature.** Run `cargo build -p anvilml-hardware --features mock-hardware` to confirm the mock-gated modules compile correctly.

6. **Build with no features.** Run `cargo build -p anvilml-hardware` (no features) to confirm the platform-gated modules (dxgi on windows, sysfs on linux) are correctly excluded and the build still succeeds.

## Public API Surface

One new public re-export added:

```rust
pub use detect::detect_all_devices;
```

This exposes `detect_all_devices` at the crate root level, so callers can write:
```rust
use anvilml_hardware::detect_all_devices;
```
instead of:
```rust
use anvilml_hardware::detect::detect_all_devices;
```

No other public items are added, removed, or modified.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub use detect::detect_all_devices;` re-export; verify all module gates |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (build verification) | `build_mock_hardware` | `cargo build -p anvilml-hardware --features mock-hardware` exits 0 — confirms mock-gated module declarations and new re-export compile | None | `--features mock-hardware` | Clean build, exit 0 | `cargo build -p anvilml-hardware --features mock-hardware` exits 0 |
| (build verification) | `build_no_features` | `cargo build -p anvilml-hardware` exits 0 — confirms platform-gated modules are correctly excluded and new re-export compiles | None | no features | Clean build, exit 0 | `cargo build -p anvilml-hardware` exits 0 |
| (build verification) | `line_count_cap` | `wc -l crates/anvilml-hardware/src/lib.rs` reports ≤80 | None | N/A | Line count ≤80 | `wc -l crates/anvilml-hardware/src/lib.rs` outputs ≤80 |

## CI Impact

No CI changes required. This task modifies only a `lib.rs` re-export file — it does not add new file types, new gates, new test modules, or new CI dependencies. The existing CI jobs (`rust-linux`, `rust-windows`) already build `anvilml-hardware` with `--features mock-hardware`, so the new re-export will be exercised by those jobs automatically.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The task only adds a single `pub use` statement — no `#[cfg]` attributes, no platform-specific code, no path-separator or line-ending handling. The existing module gates (`dxgi` on `windows`, `sysfs` on `linux`) are already correct and unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `detect_all_devices` function was added with a different signature in a prior task (e.g. it takes a `SqlitePool` parameter that doesn't exist yet), making the re-export fail to compile. | Low | High | The function was built in P5-A1 through P5-A3 and is already at `detect.rs` line 83 with signature `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>`. The re-export will compile. Verified by reading `detect.rs`. |
| Adding the re-export pushes the file over the 80-line cap. | Very Low | Medium | The file is currently 24 lines. Adding one line results in 25 lines — a margin of 55 lines to the cap. This risk is effectively nil. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-hardware/src/lib.rs` outputs a number ≤80 (exit 0)
- [ ] `cargo build -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `cargo build -p anvilml-hardware` exits 0
