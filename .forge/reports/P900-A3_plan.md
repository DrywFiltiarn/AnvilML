# Plan Report: P900-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A3                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-hardware: retrofit DEBUG fallback log to lib.rs (Vulkan→DXGI/sysfs fallback) |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a mandatory DEBUG log call per FORGE_AGENT_RULES §11.5 (mandatory DEBUG point: "Hardware detection — fallback path used when primary enumeration is unavailable") inside `enumerate_gpus()` in `crates/anvilml-hardware/src/lib.rs`. The log fires only when Vulkan returns an empty device list and the code is about to invoke a platform-specific fallback detector (DXGI on Windows, sysfs+NVML on Unix).

## Scope

### In Scope
- One file: `crates/anvilml-hardware/src/lib.rs`
- Two `tracing::debug!` calls inside `enumerate_gpus()`, each gated by the matching `#[cfg(...)]`:
  - `#[cfg(windows)]`: `tracing::debug!(fallback = "dxgi", "Vulkan returned no devices; using DXGI")`
  - `#[cfg(unix)]`: `tracing::debug!(fallback = "sysfs_nvml", "Vulkan returned no devices; using sysfs+NVML")`
- Calls placed after Vulkan returns empty and before the respective fallback detector block, so they emit only when actually falling back.

### Out of Scope
- No changes to `vulkan.rs`, `dxgi.rs`, `sysfs.rs`, `nvml.rs`, or any other crate.
- No mock-hardware path modifications (the log is inside `#[cfg(not(feature = "mock-hardware"))]` code).
- No new tests — this task is logging-only and the existing test suite must continue to pass unchanged.
- No Cargo.toml changes (tracing is already a workspace dependency used in this crate).

## Approach

1. Locate the `enumerate_gpus()` function (lines 111–202) in `crates/anvilml-hardware/src/lib.rs`. This function is behind `#[cfg(not(feature = "mock-hardware"))]`, so it is never compiled under `--features mock-hardware`.

2. Inside the `Ok(_)` arm of the Vulkan detector (line 115–121), after the existing `tracing::warn!` call and before the closing `}` of that match arm — specifically, after `Vec::new()` is constructed but before the function continues to the fallback blocks — insert two cfg-gated debug log calls.

   The placement is at the top of the `Ok(_)` arm (the "empty result" path), ensuring the log fires only when Vulkan returned zero devices and control will flow into a fallback detector:

   ```rust
   Ok(_) => {
       tracing::warn!(
           detector = "Vulkan",
           "Vulkan detector returned empty device list"
       );
       #[cfg(windows)]
       tracing::debug!(fallback = "dxgi", "Vulkan returned no devices; using DXGI");
       #[cfg(unix)]
       tracing::debug!(fallback = "sysfs_nvml", "Vulkan returned no devices; using sysfs+NVML");
       Vec::new()
   }
   ```

3. The `#[cfg(windows)]` and `#[cfg(unix)]` guards ensure each debug call is only compiled on its target platform. On Linux, only the unix variant compiles; on Windows, only the windows variant compiles. macOS (which has neither) gets neither call — consistent with the existing code which returns an empty vec for unsupported platforms.

4. Verify the log uses structured `=` notation per §11.6: `fallback = "dxgi"` / `fallback = "sysfs_nvml"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/lib.rs` | Add two cfg-gated `tracing::debug!` calls in `enumerate_gpus()` Vulkan empty-result arm |

## Tests

None. This task adds only logging instrumentation; no new tests are required or written. All existing tests must continue to pass without modification. The acceptance criterion is that `cargo test -p anvilml-hardware --features mock-hardware` exits 0.

## CI Impact

No CI changes required. No new files, no Cargo.toml changes, no test file additions. The existing CI gates (format, clippy, test, cross-check) apply unchanged. The added code is behind `#[cfg(not(feature = "mock-hardware"))]` and uses structured tracing fields that are already present in the crate.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing::debug!` macro not available in this crate | Very Low | Build failure | `tracing` is already imported via `use anvilml_core::...` indirectly — actually, checking the file: `tracing` is used at lines 116–123 with full path (`tracing::warn!`), so it is already a dependency. No change needed. |
| Adding code inside the `Ok(_)` arm changes control flow | None | N/A | The debug calls are side-effect-free and placed before `Vec::new()` — they do not alter any variable or return value. |
| Windows cross-check fails due to cfg-gated code | Very Low | Build failure on cross-target | Both `#[cfg(windows)]` and `#[cfg(unix)]` are mutually exclusive; the Windows cross-check only compiles the windows branch. Standard Rust cfg resolution handles this. |
| Log message wording mismatch with §11.5 convention | None | N/A | Message follows the exact pattern specified in the task description and TASKS_PHASE900.md §P900-A3. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0
- [ ] `cargo check --bin anvilml` exits 0 (real-hardware path compiles)
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
- [ ] The two `tracing::debug!` calls are present in `enumerate_gpus()` inside the `Ok(_)` Vulkan arm, each gated by the correct `#[cfg(...)]`
- [ ] No other files were modified
