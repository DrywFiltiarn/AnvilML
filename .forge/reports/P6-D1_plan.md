# Plan Report: P6-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-D1                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-server: fix api_models test isolation (shared temp db causes parallel test failures) |
| Depends on  | P6-C1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix `crates/anvilml-server/tests/api_models.rs` so that its three parallel tests no longer race on a shared SQLite database file. Replace the `std::process::id()`-based temp directory strategy with `tempfile::TempDir::new()`, ensuring each test gets its own unique, isolated temporary directory that lives for the duration of the test.

## Scope

### In Scope
- Modify `crates/anvilml-server/tests/api_models.rs`: replace `setup_test_env()` to return `(TempDir, PathBuf, PathBuf)` using `tempfile::TempDir::new()`, and update all three test functions to bind the returned `TempDir` guard.
- Verify `tempfile = "3"` is present in `[dev-dependencies]` of `crates/anvilml-server/Cargo.toml` (already confirmed present — no change needed).
- Run `cargo test -p anvilml-server --test api_models` and confirm all 3 tests pass, including when run with `--test-threads=1` (sequential) and the default parallel mode.

### Out of Scope
- Any changes to production source code in `anvilml-server`.
- Changes to any other test file or crate.
- Adding new tests beyond the existing three.
- Modifying CI workflow files.
- Changing test assertions or test logic — only setup scaffolding changes.
- Upgrading the `tempfile` dependency version.

## Approach

1. **Verify dependency already present.** Confirm `tempfile = "3"` exists in `[dev-dependencies]` of `crates/anvilml-server/Cargo.toml`. (Already confirmed on line 22.)

2. **Update `setup_test_env()` signature and body** in `crates/anvilml-server/tests/api_models.rs`:
   - Add `use tempfile::TempDir;` at the top.
   - Change return type from `(PathBuf, PathBuf)` to `(TempDir, PathBuf, PathBuf)`.
   - Create a `TempDir` via `tempfile::TempDir::new()` (returns an OS-managed unique path under `/tmp`).
   - Inside that temp dir, create the `diffusion/` subdirectory and write the model file.
   - Pre-create the `.db` file inside the temp dir (as before).
   - Return `(tmp, model_dir_path, db_path)`.

3. **Update all three test functions** (`list_models_returns_scanned_models`, `list_models_kind_filter_diffusion`, `list_models_kind_filter_no_match`):
   - Change destructuring from `let (model_dir, db_path) = setup_test_env();` to `let (_tmp, model_dir, db_path) = setup_test_env();`.
   - The `_tmp` binding keeps the `TempDir` alive for the test duration; it is dropped after the test completes, cleaning up the directory automatically.

4. **Run tests** with both sequential and parallel execution:
   - `cargo test -p anvilml-server --test api_models` (parallel, default)
   - `cargo test -p anvilml-server --test api_models -- --test-threads=1` (sequential)
   - Both must exit 0 with all 3 tests passing.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/tests/api_models.rs` | Replace `setup_test_env()` with TempDir-based isolation; update 3 test functions |
| (No change) | `crates/anvilml-server/Cargo.toml` | `tempfile = "3"` already present in `[dev-dependencies]` — no edit needed |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/tests/api_models.rs` | `list_models_returns_scanned_models` | GET /v1/models returns 200 with one scanned diffusion model (name, kind, dtype_hint) |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_diffusion` | GET /v1/models?kind=diffusion returns exactly one diffusion model |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_no_match` | GET /v1/models?kind=vae returns empty array when no VAE models exist |

All three tests are in the same file and will be updated to hold their own `TempDir` guard, eliminating shared-state races. No new test files are created.

## CI Impact

No CI workflow changes required. The task only modifies a test file within `anvilml-server`. The existing CI matrix runs `cargo test --workspace --features mock-hardware`, which includes this test crate. Since the fix only changes test scaffolding (not assertions or production code), and `tempfile` is already in dev-dependencies, no new jobs or steps are needed. The only gate is: `cargo test -p anvilml-server --test api_models` exits 0 with all 3 tests passing under parallel execution.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `tempfile::TempDir::new()` returns a path that may have permission issues on some CI environments | `tempfile` is already used successfully in `anvilml-registry/tests/` (store_get.rs, rescan.rs, etc.) — same pattern proven in this codebase |
| The `TempDir` guard could be dropped too early if test functions don't bind it to a local variable | Each test function will explicitly destructure and name the `TempDir` (e.g. `let (_tmp, model_dir, db_path) = ...`), ensuring it lives until end of scope |
| Pre-existing compilation errors in `anvilml-server` could mask this task's success | Run `cargo check -p anvilml-server --features mock-hardware` first to confirm clean build before running tests |
| Parallel test failure could be caused by something other than shared DB (e.g. port conflict, artifact dir) | The task description and task doc (§P6-D1) identify the root cause as shared temp db; the fix is narrowly scoped. If other failures appear, they will be documented as blockers |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test api_models` exits 0 with all 3 tests passing (parallel mode)
- [ ] `cargo test -p anvilml-server --test api_models -- --test-threads=1` exits 0 with all 3 tests passing (sequential mode)
- [ ] No changes to production source files (`src/`) — only test file modified
- [ ] `tempfile = "3"` is confirmed present in `[dev-dependencies]` of `crates/anvilml-server/Cargo.toml`
