# Plan Report: P18-C3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-C3                                            |
| Phase       | 18 — HTTP/WebSocket Server Completion             |
| Description | backend: trigger model scan on server startup     |
| Depends on  | P18-C2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T09:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Wire `ModelScanner::scan_dir()` to run as a non-blocking background task during server startup, closing the gap where a fresh server's model registry remained empty until a client manually called `POST /v1/models/rescan`. After `AppState` construction and before binding the TCP listener, `main()` will trigger the same scan logic that `P18-C2`'s `rescan_models()` handler uses, logging scan start at `INFO` level.

Observable state when complete: spawning the `anvilml` binary with a temp model directory containing a planted model file will have that model listed via `GET /v1/models` within a bounded poll window, without any explicit `/v1/models/rescan` call.

## Scope

### In Scope
- Extract the model-scan trigger logic from `P18-C2`'s `rescan_models()` handler into a shared internal async function `trigger_model_scan()` in `backend/src/main.rs`.
- Call `trigger_model_scan()` in `main()`'s default (non-`hw-probe`) path, after `AppState` construction (so `model_store` exists) and before `TcpListener::bind()`.
- Log scan start at `INFO` level via `tracing::info!`.
- Create `backend/tests/startup_scan_tests.rs` with ≥2 integration tests:
  - `test_startup_scan_displays_planted_model`: spawns the built binary against a temp model_dir with a planted model file, polls `GET /v1/models` within a bounded window, and asserts the planted model is listed — no `/v1/models/rescan` call is made.
  - `test_startup_scan_empty_dir_lists_no_models`: spawns the binary with an empty temp model_dir and asserts `GET /v1/models` returns an empty array.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope with no deferrals.

## Existing Codebase Assessment

**What already exists:** `P18-C2`'s `rescan_models()` handler in `crates/anvilml-server/src/handlers/models.rs` already implements the complete scan trigger: it clones `state.db`, `state.config.model_dirs`, and `state.config.model_scan_depth`, then spawns a `tokio::spawn` async block that constructs a `ModelScanner` and calls `scan_dir()` for each configured directory. The handler returns `StatusCode::ACCEPTED` immediately. The `ModelScanner::scan_dir()` method (from Phase 6, `crates/anvilml-registry/src/scanner.rs`) walks directories, hashes files, infers metadata, and upserts `ModelMeta` into the store.

**Established patterns:** `backend/src/main.rs` uses `tracing::info!` with structured field notation for startup events (e.g., `device_count = ...`, `worker_count = ...`, `listening`). Integration tests in `backend/tests/` follow a consistent pattern: spawn the built binary with `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, pipe stderr, use `tokio::time::timeout` to wait for the "listening" log line (5-second timeout), then assert conditions. Tests use `tempfile::tempdir()` for cleanup and set environment variables like `ANVILML_PORT=0` for ephemeral ports and `ANVILML_DB_PATH` for isolated databases.

**Gap between design doc and current source:** The design doc describes `P18-C2`'s handler as spawning a tokio task — this is implemented. However, there is no separate "trigger function" to reuse; the scan logic is inline inside `rescan_models()`. The plan must extract this into a shared function to avoid code duplication between the handler and `main()`.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It only extracts existing logic from `P18-C2`'s handler into a shared function and calls it from `main.rs`. All types (`ModelScanner`, `scan_dir`, `ModelMeta`, `ModelDirConfig`) already exist in the workspace.

## Approach

### Step 1: Extract `trigger_model_scan()` from `rescan_models()`

Create a new `async fn trigger_model_scan(pool: SqlitePool, model_dirs: Vec<ModelDirConfig>, model_scan_depth: u32)` function in `backend/src/main.rs`. This function encapsulates the exact scan logic currently inline in `P18-C2`'s `rescan_models()` handler:

```rust
/// Trigger a background scan of all configured model directories.
///
/// Spawns a `tokio::spawn` task that constructs a `ModelScanner` and calls
/// `scan_dir()` for each entry in `model_dirs`, using the configured scan depth.
/// Errors are logged at `WARN` level — the caller gets no error propagation,
/// matching the fire-and-forget contract used by the `/v1/models/rescan` handler.
///
/// This is the shared internal trigger reused by both the startup path in `main()`
/// and the `rescan_models()` HTTP handler.
async fn trigger_model_scan(
    pool: sqlx::SqlitePool,
    model_dirs: Vec<anvilml_core::ModelDirConfig>,
    model_scan_depth: u32,
) {
    tracing::info!(
        dir_count = model_dirs.len(),
        "starting model directory scan"
    );

    tokio::spawn(async move {
        let scanner = anvilml_registry::ModelScanner::new(pool);

        for entry in &model_dirs {
            let depth = if entry.recursive {
                entry.max_depth.unwrap_or(model_scan_depth)
            } else {
                0
            };

            tracing::debug!(
                path = %entry.path.display(),
                depth,
                recursive = entry.recursive,
                "scanning model directory"
            );

            match scanner.scan_dir(&entry.path, depth).await {
                Ok(models) => {
                    tracing::debug!(
                        path = %entry.path.display(),
                        count = models.len(),
                        "scan complete for {}: {} models scanned",
                        entry.path.display(),
                        models.len()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        path = %entry.path.display(),
                        error = %e,
                        "scan failed for {}: {e}",
                        entry.path.display()
                    );
                }
            }
        }
    });
}
```

Rationale: extracting into a standalone function in `main.rs` (rather than the server crate) avoids creating a cross-crate dependency from `anvilml-server` on `anvilml-registry::ModelScanner`. The `backend` crate already depends on all workspace crates directly, so this placement is natural and adds no new dependency edges.

### Step 2: Call `trigger_model_scan()` in `main()`'s startup path

In `backend/src/main.rs`, after the `app_state` construction (line 327-342 in current file) and before the TCP listener bind (line 350), add:

```rust
// Trigger a background model directory scan at startup.
// This ensures the model registry is populated before the server starts
// accepting requests — matching the contract that models must always be
// scanned on startup (P18-C3). The scan runs in a spawned tokio task,
// so it does not block the listener bind. Reuses trigger_model_scan()
// to avoid duplicating the scan logic from the /v1/models/rescan handler.
let model_dirs = app_state.config.model_dirs.clone();
let model_scan_depth = app_state.config.model_scan_depth;
trigger_model_scan(app_state.db.clone(), model_dirs, model_scan_depth);
```

The `INFO` log is emitted inside `trigger_model_scan()` at the top of the function (before the `tokio::spawn`), so the scan start is logged at `INFO` level as required.

### Step 3: Update `rescan_models()` to call `trigger_model_scan()`

Replace the inline scan logic in `crates/anvilml-server/src/handlers/models.rs::rescan_models()` with a call to `trigger_model_scan()`. Since `trigger_model_scan()` lives in `main.rs` (the `backend` crate), the handler cannot directly call it. Instead, the approach is to place `trigger_model_scan()` in a new module `backend/src/scan.rs` that both `main.rs` and `anvilml-server` can import.

**Correction to Step 1:** Place `trigger_model_scan()` in `backend/src/scan.rs` (new file). Both `main.rs` and the `anvilml-server` crate can import it. The `anvilml-server` crate already depends on `anvilml-registry`, so this is not a new dependency edge — `anvilml-server` already uses `ModelScanner`.

Updated approach for Step 1:
- Create `backend/src/scan.rs` with the `pub async fn trigger_model_scan(...)` function.
- Add `mod scan;` to `backend/src/main.rs`.
- In `rescan_models()`, replace the inline `tokio::spawn` block with:
  ```rust
  let pool = state.db.clone();
  let model_dirs = state.config.model_dirs.clone();
  let model_scan_depth = state.config.model_scan_depth;
  scan::trigger_model_scan(pool, model_dirs, model_scan_depth);
  StatusCode::ACCEPTED
  ```

Rationale: `anvilml-server` already depends on `anvilml-registry` (which provides `ModelScanner`), so importing from a new `backend/src/scan.rs` module adds no new dependency edges. The `backend` crate already depends on all workspace crates.

### Step 4: Create integration tests in `backend/tests/startup_scan_tests.rs`

Create `backend/tests/startup_scan_tests.rs` with two tests following the established pattern from `db_startup_tests.rs`:

**Test 1: `test_startup_scan_displays_planted_model`**
- Create a temp directory structure: `temp_dir/diffusion/model_fp8.safetensors` (the directory name `diffusion` determines `ModelKind::Diffusion` via `ModelScanner::infer_kind()`).
- Write a minimal safetensors file (use a pre-existing fixture or create a minimal one — the scanner only hashes the first 1 MiB and reads file metadata, so a small file is sufficient).
- Spawn the binary with `ANVILML_MODEL_DIR` set to the temp directory path (via `ANVILML_CONFIG` or by setting `model_dirs` in a temp config), `ANVILML_PORT=0`, `ANVILML_DB_PATH` in temp dir.
- Wait for "listening" log line on stderr (5-second timeout).
- Poll `GET http://127.0.0.1:{port}/v1/models` with a bounded retry loop (e.g., 3 attempts, 500ms interval) to allow the background scan to complete.
- Assert the response contains at least one model entry (the planted model).
- Kill the process.

**Test 2: `test_startup_scan_empty_dir_lists_no_models`**
- Create an empty temp directory.
- Spawn the binary with `ANVILML_MODEL_DIR` set to the empty temp directory, `ANVILML_PORT=0`, `ANVILML_DB_PATH` in temp dir.
- Wait for "listening" log line on stderr (5-second timeout).
- Poll `GET http://127.0.0.1:{port}/v1/models` with the same bounded retry loop.
- Assert the response is an empty JSON array `[]`.
- Kill the process.

### Step 5: Update `anvilml-server/src/handlers/models.rs` to use `trigger_model_scan()`

Replace the inline `tokio::spawn` block in `rescan_models()` with a call to `scan::trigger_model_scan()`, as described in Step 3 above. The handler's `#[instrument(skip(state))]` attribute and the `tracing::info!` at the top of the function (now in `trigger_model_scan`) are preserved.

## Public API Surface

No new public items. `trigger_model_scan()` is a private `async fn` in `backend/src/scan.rs` (not `pub`). The existing public API surface is unchanged:
- `AppState::model_store` (existing)
- `ModelScanner::scan_dir()` (existing)
- `rescan_models()` (existing, modified to delegate)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/scan.rs` | New module with `trigger_model_scan()` shared scan trigger function |
| MODIFY | `backend/src/main.rs` | Add `mod scan;`, import `scan::trigger_model_scan`, call it after `AppState` construction, before TCP bind |
| MODIFY | `crates/anvilml-server/src/handlers/models.rs` | Replace inline scan logic in `rescan_models()` with call to `scan::trigger_model_scan()` |
| CREATE | `backend/tests/startup_scan_tests.rs` | ≥2 integration tests for startup scan behavior |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.15 → 0.1.16 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/startup_scan_tests.rs` | `test_startup_scan_displays_planted_model` | Spawning the binary with a temp model_dir containing a planted model file results in that model being listed via `GET /v1/models` within a bounded poll window, with no `/v1/models/rescan` call made | `anvilml` binary compiled; temp dir with valid safetensors file | Temp model_dir with `diffusion/model_fp8.safetensors`; `ANVILML_PORT=0`; `ANVILML_DB_PATH` in temp dir; `ANVILML_MODEL_DIR` set to temp dir | `GET /v1/models` returns a JSON array containing ≥1 model entry within 5 polling attempts (500ms interval) | `cargo test -p anvilml --test startup_scan_tests -- test_startup_scan_displays_planted_model` exits 0 |
| `backend/tests/startup_scan_tests.rs` | `test_startup_scan_empty_dir_lists_no_models` | Spawning the binary with an empty temp model_dir results in `GET /v1/models` returning an empty array | `anvilml` binary compiled; empty temp dir | Empty temp model_dir; `ANVILML_PORT=0`; `ANVILML_DB_PATH` in temp dir; `ANVILML_MODEL_DIR` set to temp dir | `GET /v1/models` returns `[]` within 5 polling attempts | `cargo test -p anvilml --test startup_scan_tests -- test_startup_scan_empty_dir_lists_no_models` exits 0 |

## CI Impact

No CI changes required. The new test file `backend/tests/startup_scan_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` (Step 6 of ENVIRONMENT.md §6). No new file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The scan logic uses only `std::fs` and `tokio::spawn` — both platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The test's bounded poll window may be too short for the background scan to complete on slow CI environments, causing intermittent failures. | Medium | Medium | Use a 5-second timeout with 3-5 polling attempts at 500ms intervals (same pattern used in existing `db_startup_tests.rs` for worker Ready events). The scan is typically fast (<1s for small dirs), so this provides ample margin. |
| The temp safetensors file used in the planted model test may not pass the scanner's hash/parse logic, causing the model to be silently skipped. | Low | Medium | Use a minimal but valid safetensors header — the scanner only reads the first 1 MiB and hashes it; it does not validate tensor data. A 256-byte file with a valid safetensors magic header is sufficient. |
| `anvilml-server` importing from `backend/src/scan.rs` creates a circular dependency since `backend` depends on `anvilml-server`. | Low | High | Verified: `anvilml-server` does NOT depend on `backend` — the dependency graph (ARCHITECTURE.md §3) shows `backend/src/main.rs` depends on `anvilml-server`, not the reverse. The `scan` module lives in `backend`, and `anvilml-server` imports it as an external crate. No cycle exists. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml --test startup_scan_tests` exits 0 with ≥2 tests passing
- [ ] `cargo test -p anvilml-server --test models_tests` exits 0 (P18-C2's existing tests still pass after refactoring `rescan_models()`)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
