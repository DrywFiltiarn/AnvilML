# Plan Report: P11-C3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-C3                                      |
| Phase       | 011 — Graph Validation                      |
| Description | anvilml-worker: fix venv path resolution base — use repo root not current_dir |
| Depends on  | P11-C2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-07T12:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the venv path resolution in `anvilml-worker::spawn()` so that relative `venv_path` values are resolved against the repository root (via `_repo_root_for_worker()`) instead of `std::env::current_dir()`. This eliminates ENOENT / ERROR_PATH_NOT_FOUND failures when `cargo test` sets the process CWD to the crate directory rather than the repo root.

## Scope

### In Scope
- Modify `crates/anvilml-worker/src/managed.rs`: replace `std::env::current_dir().unwrap_or_default()` with `_repo_root_for_worker()` in the `abs_venv` resolution block inside `spawn()`.
- Bump `anvilml-worker` crate patch version from `0.1.8` to `0.1.9` in `crates/anvilml-worker/Cargo.toml`.

### Out of Scope
- Any changes to `resolve_python_path`, `_repo_root_for_worker`, or the test logic.
- Changes to any other crate, config file, CI workflow, or documentation.
- New tests — this is a targeted bug fix with no new test files required.

## Approach

1. **Read** `crates/anvilml-worker/src/managed.rs` lines 128–140 to confirm the exact current code block:
   ```rust
   let abs_venv = if cfg.venv_path.is_absolute() {
       cfg.venv_path.clone()
   } else {
       std::env::current_dir()
           .unwrap_or_default()
           .join(&cfg.venv_path)
   };
   ```

2. **Replace** the `else` branch base from `std::env::current_dir().unwrap_or_default()` to `_repo_root_for_worker()`:
   ```rust
   let abs_venv = if cfg.venv_path.is_absolute() {
       cfg.venv_path.clone()
   } else {
       _repo_root_for_worker().join(&cfg.venv_path)
   };
   ```
   This is a single-line change (removing `.unwrap_or_default()` and replacing the function call). The `is_absolute()` guard and surrounding code remain untouched.

3. **Bump** the crate version: update `crates/anvilml-worker/Cargo.toml` line 3 from `version = "0.1.8"` to `version = "0.1.9"`.

4. **Verify** with `cargo check -p anvilml-worker --features mock-hardware` and `cargo test -p anvilml-worker --features mock-hardware`. Both must exit 0.

5. **Cross-check** with `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` (per ENVIRONMENT.md §7). Must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Replace `std::env::current_dir().unwrap_or_default()` with `_repo_root_for_worker()` in `spawn()` venv path resolution (line ~134) |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.8 → 0.1.9` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (existing) | `cargo test -p anvilml-worker --features mock-hardware` | All existing anvilml-worker tests pass, including spawn integration tests that exercise the venv path resolution code path |

No new test files are added. The existing test suite already exercises the `spawn()` code path through the four serialised spawning tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) introduced in P11-C2.

## CI Impact

No CI workflow file changes are required. The existing CI gates (format, clippy, test, cross-check) already cover this crate. The change is a single-line fix that reduces the number of function calls in `spawn()` (removing `.unwrap_or_default()` and replacing `std::env::current_dir()` with `_repo_root_for_worker()`) — no new dependencies, no new feature flags, no API surface changes.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `_repo_root_for_worker()` returns a different path than expected on some invocation contexts | Low | Medium | The function derives its path from `CARGO_MANIFEST_DIR` (compile-time constant) and walks up two parent directories — this is the same mechanism already used for `.current_dir()` on the child `Command`. It is exercised by all existing spawn tests. |
| Removing `std::env::current_dir()` leaves unused import in the module | Low | Low | `std::env` is used elsewhere in the file (e.g., env var lookups in test fixtures). Verify with `cargo clippy -- -D warnings` that no dead-code warning appears. If it does, remove only the unused import line. |
| Version bump introduces unexpected diff noise in Cargo.lock | Low | Low | `Cargo.lock` is regenerated automatically on next build; no manual edits needed. The lockfile diff will reflect the version bump only. |

## Acceptance Criteria

- [ ] `crates/anvilml-worker/src/managed.rs` contains `_repo_root_for_worker().join(&cfg.venv_path)` in place of the old `std::env::current_dir()` resolution, with no other changes to that block
- [ ] `std::env::current_dir()` call removed from the venv resolution; no dead-code warning from clippy
- [ ] `crates/anvilml-worker/Cargo.toml` version bumped from `0.1.8` to `0.1.9`
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all tests pass)
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
