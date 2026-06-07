# Plan Report: P11-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P11-C1                                            |
| Phase       | 011 — Graph Validation                            |
| Description | anvilml-worker: fix relative venv path causing Windows spawn ERROR_PATH_NOT_FOUND |
| Depends on  | P10-B4                                             |
| Project     | anvilml                                            |
| Planned at  | 2026-06-07T13:28:00Z                              |
| Attempt     | 1                                                  |

## Objective

Fix a Windows `CreateProcess` `ERROR_PATH_NOT_FOUND` bug in `anvilml-worker::spawn()` caused by passing a relative `venv_path` to `resolve_python_path()` while the child process's working directory has been set to `_repo_root_for_worker()`. On Windows, `CreateProcess` resolves a relative executable path against the child's CWD, not the parent's — so when the two differ, the interpreter path does not exist and spawn fails.

## Scope

### In Scope
- Modify `crates/anvilml-worker/src/managed.rs`: in `spawn()`, resolve `cfg.venv_path` to an absolute path before calling `resolve_python_path()`.
- Bump `anvilml-worker` crate patch version from `0.1.6` to `0.1.7` in `crates/anvilml-worker/Cargo.toml`.

### Out of Scope
- Changes to `resolve_python_path()` function.
- Changes to `_repo_root_for_worker()` function.
- Changes to test logic or test files.
- Changes to any other crate.
- CI workflow modifications.

## Approach

1. **Open** `crates/anvilml-worker/src/managed.rs`.
2. **Locate** the `spawn()` method (line ~128) and find the line:
   ```rust
   let python_path = resolve_python_path(&cfg.venv_path);
   ```
3. **Replace** that single line with three lines that resolve the venv path to absolute before passing it to `resolve_python_path`:
   ```rust
   let abs_venv = if cfg.venv_path.is_absolute() {
       cfg.venv_path.clone()
   } else {
       std::env::current_dir().unwrap_or_default().join(&cfg.venv_path)
   };
   let python_path = resolve_python_path(&abs_venv);
   ```
4. **Bump** `crates/anvilml-worker/Cargo.toml` patch version: change `version = "0.1.6"` to `version = "0.1.7"`.
5. **Verify** locally with the two acceptance commands:
   - `cargo test -p anvilml-worker --features mock-hardware` — exits 0
   - `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` — exits 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | In `spawn()`, resolve `cfg.venv_path` to absolute path before calling `resolve_python_path()` |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.6 → 0.1.7` |

## Tests

No new test files are written. The existing spawning integration tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) exercise the `spawn()` code path and will validate the fix. These tests use `ANVILML_VENV_PATH` (or a fallback absolute path) so they pass on both platforms after this change.

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` (tests module) | `spawn_ping_pong` | Worker spawns, receives Ping→Pong, exits on Shutdown |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `status_transitions` | Status flows Initializing → Idle → Dead |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `handshake_completes_once` | Exactly one Ready event during spawn handshake |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `spawn_reaches_idle` | Spawn completes and reaches Idle without timing workarounds |

## CI Impact

No CI workflow files are modified. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`, etc.) will exercise this change as part of the Phase 11 test suite. No new CI jobs or steps are required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `std::env::current_dir()` returns an error in a confined environment (sandbox, chroot) | Low | Medium | Fallback to `PathBuf::from(".")` via `.unwrap_or_default()` — this matches the existing `_repo_root_for_worker()` fallback pattern and will still produce a valid path. |
| The fix changes behaviour on Linux where the bug may not manifest | Low | Low | On Linux, `spawn()` uses `fork()+exec()` which resolves relative paths against the parent's CWD regardless of child CWD; the absolute-path resolution is a no-op for correctness and only adds a negligible string comparison. |
| Existing tests hard-code an absolute venv path in test fixtures, masking the bug locally | Medium | Low | Tests already pass because they use an absolute path (`/home/dryw/forge/.venv`); the fix does not change that code path. The bug only manifests when `ANVILML_VENV_PATH` is set to a relative value (e.g., `.ci-venv` in CI). |

## Acceptance Criteria

- [ ] `spawn()` resolves `cfg.venv_path` to an absolute path before calling `resolve_python_path()`
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests pass)
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `anvilml-worker` crate version bumped to `0.1.7` in `Cargo.toml`
- [ ] No other files modified beyond those listed in "Files Affected"
