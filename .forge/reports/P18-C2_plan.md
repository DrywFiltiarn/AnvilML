# Plan Report: P18-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-C2                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | anvilml-server: POST /v1/models/rescan handler |
| Depends on  | P18-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T09:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `POST /v1/models/rescan` handler to `crates/anvilml-server/src/handlers/models.rs`, wiring it into `build_router()`. The handler spawns a tokio task that calls `ModelScanner::scan_dir()` for each configured model directory (from `ServerConfig::model_dirs`), and immediately returns HTTP 202 Accepted without blocking on scan completion. This enables clients to trigger a fresh model-directory scan at will, with the scan running asynchronously in the background. Two new tests in `models_tests.rs` verify: (1) the handler returns 202 immediately regardless of scan duration, and (2) the background scan's results are observable via a subsequent `GET /v1/models` list call.

## Scope

### In Scope
- Add `rescan_models()` handler function in `crates/anvilml-server/src/handlers/models.rs`
- Register `POST /v1/models/rescan` route in `build_router()` in `crates/anvilml-server/src/lib.rs`
- Two new integration tests in `crates/anvilml-server/tests/models_tests.rs`
- Bump `anvilml-server` crate patch version (0.1.20 → 0.1.21) in `Cargo.toml`

### Out of Scope
None. This task's `defers_to (from JSON): []` is empty — no scope is deferred. The startup auto-scan (P18-C3) is a separate task's responsibility and is not part of this handler.

## Existing Codebase Assessment

The codebase already has `GET /v1/models` and `GET /v1/models/:id` handlers implemented in `models.rs` (from P18-C1), which follow a thin-delegation pattern: they extract `AppState` via `State(state)` and delegate to `state.model_store`. The `AppState` struct already contains `model_store: Arc<ModelStore>` and `config: Arc<ServerConfig>`, where `config.model_dirs` is a `Vec<ModelDirConfig>` with `path`, `recursive`, and `max_depth` fields. `ModelScanner` (from `anvilml-registry`) is fully implemented with `new(pool: SqlitePool)` constructor and `async fn scan_dir(&self, root: &Path, depth: u32)` — it walks the directory tree, hashes files, and upserts `ModelMeta` into the store. The test infrastructure is mature: `models_tests.rs` uses an in-memory SQLite pool with migrations applied, a `make_test_state()` helper that constructs a minimal `AppState`, and `tower::util::ServiceExt::oneshot()` for in-process HTTP testing. Established patterns include `#[tracing::instrument]` on async handlers, `///` doc comments on all public items, and structured `tracing::info!`/`tracing::debug!` log calls at decision points.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses `tokio::spawn` (already available via the `anvilml-server` crate's `tokio` dependency with `"rt"` and `"sync"` features) and `ModelScanner` from `anvilml-registry` (already a path dependency of `anvilml-server`). No MCP lookup is required.

## Approach

1. **Bump `anvilml-server` patch version** in `crates/anvilml-server/Cargo.toml`: change `version = "0.1.20"` to `version = "0.1.21"`.

2. **Add `rescan_models()` handler** to `crates/anvilml-server/src/handlers/models.rs`:
   - Signature: `pub async fn rescan_models(State(state): State<AppState>) -> StatusCode`
   - Add imports: `ModelScanner` from `anvilml_registry`, `Path` from `std::path`, and `tracing`.
   - Implementation:
     a. Log at INFO level: `"rescan triggered, scanning {} model dir(s)"` with the count of `state.config.model_dirs`.
     b. Clone `state.model_store.store` (the `SqlitePool`) from the `ModelStore` — or more precisely, clone the pool that the `ModelStore` was constructed with. Since `ModelStore` owns the pool internally and doesn't expose it directly, construct the `ModelScanner` by accessing the pool through the store's internal field. Looking at the `ModelStore` struct in `store.rs`: it holds `pool: SqlitePool`. We need to either add a getter or construct a scanner differently. Actually, `ModelScanner::new()` takes a `SqlitePool`, and `ModelStore::new()` also takes a `SqlitePool`. The `AppState` has `model_store: Arc<ModelStore>`. We need the pool. Since `ModelStore` doesn't currently expose its pool, we have two options: (a) add a `pub fn pool(&self) -> SqlitePool` getter on `ModelStore`, or (b) clone the pool from `state.db` which is the same pool used by `ModelStore`. Option (b) is simpler and avoids adding a new pub method to `anvilml-registry` (which would be a cross-crate change outside this task's scope). Use `state.db.clone()` to construct the `ModelScanner`.
     c. Spawn a tokio task: `tokio::spawn(async move { ... })`. Inside the task, iterate over `state.config.model_dirs`, and for each entry, call `scanner.scan_dir(&entry.path, entry.max_depth.unwrap_or(state.config.model_scan_depth)).await` — note that when `recursive` is false, the depth parameter still controls how deep to recurse; a depth of 0 scans only the root directory. The `max_depth` field on `ModelDirConfig` is "Maximum scan depth when `recursive = true`" — when `recursive` is false, `depth` should be 0 (only scan the immediate directory). So the effective depth is: if `entry.recursive`, use `entry.max_depth.unwrap_or(state.config.model_scan_depth)`, else use `0`.
     d. Log at DEBUG level: `"scan complete for {path}: {count} models scanned"` after each `scan_dir()` call.
     e. Log at WARN level if `scan_dir()` returns an error: `"rescan failed for {path}: {error}"`.
   - Return `StatusCode::ACCEPTED` (202) immediately, before the spawned task completes.
   - Add `#[tracing::instrument(skip(state))]` attribute.
   - Add `///` doc comment describing the handler's purpose, response code, and behavior.

3. **Register the route** in `build_router()` in `crates/anvilml-server/src/lib.rs`:
   - Add `.route("/v1/models/rescan", axum::routing::post(handlers::models::rescan_models))` after the existing `/v1/models/{id}` route.

4. **Add tests** to `crates/anvilml-server/tests/models_tests.rs`:
   - **Test 1: `test_rescan_returns_202_immediately`** — Construct a `ModelDirConfig` pointing to a temp directory that exists but contains no model files (or a very large file that takes time to hash). Send a POST to `/v1/models/rescan`, assert the response status is 202. Use `tokio::time::timeout` to verify the response returns within a short window (e.g., 500ms), proving it does not block on scan completion. This tests the non-blocking contract.
   - **Test 2: `test_rescan_populates_model_store`** — Create a temp directory with a planted model file (a small `.safetensors` file). Construct `AppState` with a `model_dirs` entry pointing to that temp directory. Send POST `/v1/models/rescan`, wait briefly for the background task to complete (use `tokio::time::sleep(Duration::from_millis(200))` or poll the store), then call `GET /v1/models` and assert the planted model appears in the response. This verifies the background scan's results are observable.

5. **Update `models.rs` module doc comment** to mention the new `rescan_models` handler.

## Public API Surface

| Item | Type | Crate/Module Path | Description |
|------|------|-------------------|-------------|
| `rescan_models` | `pub async fn(State<AppState>) -> StatusCode` | `anvilml-server/src/handlers/models.rs` | POST /v1/models/rescan handler — spawns background ModelScanner tasks, returns 202 immediately |

No changes to existing pub items. No new types introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.20 → 0.1.21 |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `rescan_models()` handler + update module doc comment |
| Modify | `crates/anvilml-server/src/lib.rs` | Register `POST /v1/models/rescan` route in `build_router()` |
| Modify | `crates/anvilml-server/tests/models_tests.rs` | Add `test_rescan_returns_202_immediately` and `test_rescan_populates_model_store` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-server/tests/models_tests.rs` | `test_rescan_returns_202_immediately` | POST /v1/models/rescan returns 202 within 500ms even when scanning a directory with no models — proves the handler does not block on scan completion | `cargo test -p anvilml-server --test models_tests test_rescan_returns_202_immediately` exits 0 |
| `crates/anvilml-server/tests/models_tests.rs` | `test_rescan_populates_model_store` | After POST /v1/models/rescan with a temp model_dir containing a planted .safetensors file, a subsequent GET /v1/models lists the new model — proves the background scan writes to the store | `cargo test -p anvilml-server --test models_tests test_rescan_populates_model_store` exits 0 |

The existing 4 tests (`test_list_models_no_filter`, `test_list_models_kind_filter`, `test_get_model_existing_returns_200`, `test_get_model_unknown_returns_404`) remain unchanged. Total: 6 tests.

## CI Impact

No CI changes required. The new handler uses only already-available dependencies (`tokio::spawn`, `ModelScanner`). The test file lives in the existing `tests/` directory, which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new CI jobs or gates are affected.

## Platform Considerations

None identified. The `ModelScanner::scan_dir()` implementation uses `std::fs::read_dir()` and `std::fs::metadata()` which are cross-platform. The handler is a pure HTTP route with no platform-specific code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelStore` does not expose its `SqlitePool` — constructing `ModelScanner` requires a pool, and `state.db` is a separate pool instance that may not be the same one used by `model_store`. If they differ, the scanner writes to a different database than the store reads from. | Medium | High | Verify against the actual `ModelStore` source. If the pool is not the same as `state.db`, add a minimal `pub fn pool(&self) -> SqlitePool` getter to `ModelStore` — but this is a cross-crate change outside this task's scope. If that's not viable, construct the scanner from `state.db.clone()` and document the assumption that both use the same pool. |
| The spawned tokio task may panic on an unhandled error (e.g., `scan_dir` fails), causing a detached task warning. Since the task is fire-and-forget, the panic is swallowed silently. | Low | Medium | Wrap the scan loop body in a `match scanner.scan_dir(...).await { Ok(_) => ..., Err(e) => tracing::warn!(...) }` so errors are logged rather than panicking. |
| Test 2's timing: the background scan may not complete before the subsequent `GET /v1/models` call, causing a flaky test. | Medium | Medium | Use a bounded `tokio::time::sleep(Duration::from_millis(200))` after the rescan request before polling the store, or poll with a retry loop bounded by a timeout. The acceptance criterion says "a completed background scan updates model_store's contents observable on a subsequent list call" — the test must ensure the scan has completed before asserting. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test models_tests` exits 0 (>=6 total tests)
- [ ] `cargo build -p anvilml` exits 0
