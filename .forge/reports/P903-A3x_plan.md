# Plan Report: P903-A3x

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P903-A3x                                          |
| Phase       | 903 — IPC Transport Rework                        |
| Description | Fix GenericFilePath/GenericNamespaced Windows name-type error in spawn and test |
| Depends on  | P903-A3                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-09T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Fix a cross-platform bug in `crates/anvilml-worker/src/managed.rs` where `GenericFilePath` is used to convert IPC socket paths to `interprocess` names on all platforms. On Windows, `build_socket_path()` returns a named pipe path (`\\.\pipe\anvilml-worker-...`) which must be converted via `GenericNamespaced` instead of `GenericFilePath`. This causes a compile-time or runtime failure on the Windows target because `GenericFilePath` explicitly rejects non-filesystem paths.

## Scope

### In Scope
- Add a private `to_socket_name()` helper in `managed.rs` with `#[cfg(unix)]` and `#[cfg(windows)]` arms
- Replace all 6 call sites of `.to_fs_name::<GenericFilePath>()` on `build_socket_path()` results with `to_socket_name(&path)?`
- Update module-level imports to include `ToNsName` and `GenericNamespaced` (windows-only)
- Remove unused `GenericFilePath` from the unconditional import list

### Out of Scope
- Any changes to `spawn()` logic, `respawn()` logic, or other methods
- Any changes to test assertions or test logic
- Changes to any file other than `managed.rs`
- Version bumps (no source API changes)
- Changes to `Cargo.toml`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `to_socket_name` helper, replace 6 `to_fs_name::<GenericFilePath>()` call sites, update imports |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` (mod tests) | `respawn_after_death` | Both `socket_path` and `socket_path2` bind+connect now use `to_socket_name`, exercising both Unix and Windows code paths at compile time |

Note: No new test files are written. The existing `respawn_after_death` test already covers the affected code paths; it just needs the compilation fix to pass on Windows.

## CI Impact

The fix exercises `#[cfg(windows)]` code paths during cross-compilation (`--target x86_64-pc-windows-gnu`). This ensures the Windows build compiles cleanly. No CI workflow files are modified. The acceptance criterion `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` must exit 0.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `GenericNamespaced` / `ToNsName` API differs from what the task description shows | Low | High | The interprocess 2.4.2 docs confirm both exist; the task description provides the exact API shape. Verified via `rust-docs` lookup. |
| `#[cfg(unix)]` / `#[cfg(windows)]` mismatch causes compilation on other platforms | Very low | Medium | The crate only targets Linux and Windows per ARCHITECTURE.md §7. The `interprocess` crate itself is cfg-gated for these platforms. No other platforms are in scope. |
| `to_socket_name` returns `Name<'_>` which may differ in lifetime from `Name<'static>` | Low | Medium | The existing `ListenerOptions::new().name(...)` accepts `impl Into<Name<'a>>` — the lifetime is handled by the `ListenerOptions` builder pattern, same as before. |
| Test `respawn_after_death` uses `#[cfg(unix)]` blocks that skip socket creation on Windows | Medium | Low | The test has `#[cfg(unix)]` guards for directory creation but the `to_fs_name::<GenericFilePath>()` calls are NOT cfg-gated — they compile on both platforms. The fix makes them compile correctly on Windows. On Linux, the `#[cfg(unix)]` helper simply returns the same result as before. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 on Linux
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
