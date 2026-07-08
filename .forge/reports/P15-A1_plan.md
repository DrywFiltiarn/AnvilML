# Plan Report: P15-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-A1                                      |
| Phase       | 15 — Artifact Storage Wiring                |
| Description | anvilml-server: AppState gains artifact_store field |
| Depends on  | P14-D2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T17:56:00Z                        |
| Attempt     | 1                                           |

## Objective

Add an `artifact_store: Arc<ArtifactStore>` field to `AppState` (the shared application state struct used by all HTTP handlers), and wire its construction in `backend/main.rs`'s normal startup path. This is the first structural step needed for Phase 15's Group B (HTTP artifact endpoints) and Group C (event-loop artifact persistence) tasks to access `ArtifactStore` from `anvilml-artifacts`. The field is constructed from `cfg.artifact_dir` and the existing `SqlitePool`, sharing the same database connection that `JobStore` and the ghost-job-reset already use.

## Scope

### In Scope
- Add `artifact_store: Arc<ArtifactStore>` field to `AppState` in `crates/anvilml-server/src/state.rs`.
- Update `backend/src/main.rs` to construct `ArtifactStore::new(artifact_dir, pool.clone())` and pass it into the `AppState` struct literal.
- Add >=2 new tests in `crates/anvilml-server/tests/state_tests.rs`: one covering `artifact_store` construction, one covering `artifact_store` cloning/Arc-sharing semantics.
- Bump `anvilml-server` crate patch version from `0.1.7` to `0.1.8`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope without deferring any functionality.

## Existing Codebase Assessment

**What already exists:** `AppState` (Phase 14's P14-C1) currently holds six fields: `config`, `node_registry`, `start_time`, `scheduler`, `workers`, and `db`. `ArtifactStore` (Phase 6's P6-B3) is fully implemented in `crates/anvilml-artifacts/src/store.rs` with `new()`, `save()`, `get()`, and `list()` methods. It is already declared as a path dependency in `anvilml-server/Cargo.toml` (line 11), so no new dependency is needed.

**Established patterns:** `AppState` fields use `Arc<T>` for shared ownership (config, node_registry, scheduler, workers). The `db` field is a bare `SqlitePool` (cheaply cloneable via its internal Arc). Construction in `backend/main.rs` follows a linear sequence: create pool → create subsystems → build `AppState` struct literal. Tests in `state_tests.rs` use a `make_full_state()` helper and per-test `AppState` struct literals with all fields named explicitly.

**Gap between design doc and source:** None. The design doc (§13.2) lists `artifact_store` as one of several future fields; the current source has it absent, which is the correct starting point for this task.

## Resolved Dependencies

| Type   | Name             | Version verified | MCP source     | Feature flags confirmed |
|--------|------------------|-----------------|----------------|------------------------|
| crate  | anvilml-artifacts| 0.1.3 (workspace path) | N/A (internal crate) | n/a |

No new external dependencies are introduced. `anvilml-artifacts` is already a declared path dependency of `anvilml-server`.

## Approach

1. **Add `artifact_store` field to `AppState` in `crates/anvilml-server/src/state.rs`.**
   - Import `anvilml_artifacts::ArtifactStore` at the top of the file (alongside the existing imports from `anvilml_core`, `anvilml_scheduler`, `anvilml_worker`).
   - Add a new field: `pub artifact_store: Arc<ArtifactStore>`, with a `///` doc comment describing its purpose: "Content-addressed PNG artifact storage, shared by HTTP handlers and the event loop."
   - This is a single-line addition to the struct body.

2. **Construct `ArtifactStore` in `backend/src/main.rs` and pass it into `AppState`.**
   - After the existing `let pool = create_pool(&config.db_path)` call (line 118), add a construction of `ArtifactStore`:
     ```rust
     let artifact_store = Arc::new(anvilml_artifacts::ArtifactStore::new(
         config.artifact_dir.clone(),
         pool.clone(),
     ));
     ```
   - The `artifact_dir` comes from `config.artifact_dir` (a `PathBuf` in `ServerConfig`). The `pool` is the same `SqlitePool` already created for `JobStore` — we clone it (cheap Arc ref-count bump) so both stores share the same database connection pool.
   - Add the `artifact_store` field to the `AppState` struct literal (lines 270–277), placing it after `db` to maintain logical grouping (data stores together).

3. **Add tests in `crates/anvilml-server/tests/state_tests.rs`.**
   - **Test 1 — `test_app_state_artifact_store_constructs`:** Construct an `AppState` with an `ArtifactStore` backed by a temp directory and in-memory SQLite pool. Assert that the `artifact_store` field is accessible and its `Arc` pointer is valid (non-null). This verifies the construction path works end-to-end.
   - **Test 2 — `test_app_state_artifact_store_clone_shares`:** Construct `AppState` with an `ArtifactStore`, clone it, then verify via pointer comparison (`std::ptr::eq(Arc::as_ptr(...))`) that both the original and cloned state share the same `Arc<ArtifactStore>` allocation. This verifies the cloning semantics match the established pattern used by other `Arc` fields.
   - Update `make_full_state()` helper to accept an `Arc<ArtifactStore>` parameter and include it in the constructed `AppState`.

4. **Bump `anvilml-server` crate version** from `0.1.7` to `0.1.8` in `crates/anvilml-server/Cargo.toml`.

## Public API Surface

| Item | Crate/Module | Description |
|------|-------------|-------------|
| `AppState::artifact_store` | `anvilml-server/src/state.rs` | New pub field: `pub artifact_store: Arc<ArtifactStore>` |

No new `pub` functions, structs, or traits are introduced. The `ArtifactStore` type and its public API (`new`, `save`, `get`, `list`) are already defined in `anvilml-artifacts`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `artifact_store: Arc<ArtifactStore>` field to `AppState` |
| MODIFY | `backend/src/main.rs` | Construct `ArtifactStore` and pass into `AppState` |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Add 2 new tests + update `make_full_state()` helper |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_artifact_store_constructs` | `AppState` constructs with an `ArtifactStore` field; field is accessible | In-memory SQLite pool, temp artifact dir | Default config values | No panic; `artifact_store` field accessible | `cargo test -p anvilml-server --test state_tests -- test_app_state_artifact_store_constructs` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_artifact_store_clone_shares` | Cloned `AppState` shares the same `Arc<ArtifactStore>` allocation as the original | `AppState` constructed with `ArtifactStore` | Two `AppState` clones | `Arc::as_ptr()` comparison returns true | `cargo test -p anvilml-server --test state_tests -- test_app_state_artifact_store_clone_shares` exits 0 |

## CI Impact

No CI changes required. The test module `state_tests` is already collected by `cargo test --workspace --features mock-hardware`. The `anvilml-artifacts` dependency was already declared in `anvilml-server/Cargo.toml`, so no manifest change affects CI dependency resolution.

## Platform Considerations

None identified. The `ArtifactStore::new()` takes a `PathBuf` and `SqlitePool` — both platform-neutral types. The `artifact_dir` path is user-configurable via `anvilml.toml` or `ANVILML_ARTIFACT_DIR` env var. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `config.artifact_dir` is a `PathBuf` that may need `.clone()` — if the type is actually `String` in `ServerConfig`, the `.clone()` call will fail at compile time. | Low | Medium | Read `anvilml-core/src/config.rs` to confirm the exact type of `artifact_dir` before writing. The type is `PathBuf` (confirmed: `ServerConfig` uses `PathBuf` for path fields per ENVIRONMENT.md §4). |
| `make_full_state()` helper change could break existing tests if the new `Arc<ArtifactStore>` parameter is not propagated to all call sites. | Medium | Low | All call sites of `make_full_state()` are in `state_tests.rs` — read the file, update every call site. The helper gains an `Arc<ArtifactStore>` parameter; update the 3 existing callers (`test_app_state_clone_shares_node_registry`, `test_app_state_with_new_fields`, `test_app_state_clone_preserves_all_fields`, `test_app_state_scheduler_arc_sharing`) to pass a fresh `Arc::new(ArtifactStore::new(...))`. |
| `anvilml-server`'s `build_router()` function signature is unchanged (takes `AppState` by value), so no handler code needs updating. | Low | None | Confirmed: `build_router` signature is `pub fn build_router(app_state: AppState) -> axum::Router`. No signature change needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test state_tests` exits 0
- [ ] `cargo build -p anvilml` exits 0
- [ ] `anvilml-server` crate version is `0.1.8` in `crates/anvilml-server/Cargo.toml`
- [ ] `crates/anvilml-server/src/state.rs` contains `pub artifact_store: Arc<ArtifactStore>` field
- [ ] `backend/src/main.rs` constructs `ArtifactStore` and passes it into `AppState`
- [ ] `crates/anvilml-server/tests/state_tests.rs` contains >=2 new tests for `artifact_store`
