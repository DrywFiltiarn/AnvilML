# Plan Report: P6-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A5                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml: initial model scan at startup + registry in AppState |
| Depends on  | P6-A4                                               |
| Project     | anvilml                                             |
| Planned at  | 2026-06-04T07:30:00Z                                |
| Attempt     | 1                                                   |

## Objective

Integrate the `ModelRegistry` into AnvilML's application lifecycle: add `Arc<ModelRegistry>` to `AppState`, build it from the opened SQLite pool in `main.rs`, spawn a non-blocking tokio task to perform the initial model directory rescan (logging the count), and store the registry Arc in AppState without blocking the server bind.

## Scope

### In Scope
- Add `pub registry: Arc<ModelRegistry>` field to `AppState` in `crates/anvilml-server/src/state.rs`
- Update both `AppState::new()` and `AppState::new_with_hardware()` constructors to accept an optional `Arc<ModelRegistry>` parameter (defaulting to empty scan when None)
- Update `AppState::Clone` impl to clone the registry Arc
- In `backend/src/main.rs`: after DB open and ghost-job reset, create `ModelRegistry::new(db.clone())`, spawn a non-blocking `tokio::spawn` task calling `registry.rescan(&cfg.model_dirs)` and logging the count, pass `Arc::new(registry)` to `AppState::new_with_hardware()`
- No new dependencies — `anvilml-server` already depends on `anvilml-registry` (re-exporting `ModelRegistry`)

### Out of Scope
- Model-scanning REST handlers (GET /v1/models, GET /v1/models/:id, POST /v1/models/rescan) — these are P6-A6 and P6-A7
- Config surface changes (no new ServerConfig fields)
- Database schema changes
- Tests for the main.rs integration (verified via REST in next task)

## Approach

1. **Modify `crates/anvilml-server/src/state.rs`:**
   - Add `use std::sync::Arc;` import (already present).
   - Add `pub registry: Arc<ModelRegistry>` field to the `AppState` struct.
   - Update `new()` to accept an optional `registry: Option<Arc<ModelRegistry>>` parameter, converting `None` to `Arc::new(ModelRegistry::new(pool))` when db is `Some`, or empty default otherwise.
   - Update `new_with_hardware()` identically — add `registry: Option<Arc<ModelRegistry>>` parameter.
   - Update the `Clone` impl to clone `self.registry`.

2. **Modify `backend/src/main.rs`:**
   - After the ghost-job reset block (after line ~145), create the registry:
     ```rust
     let registry = anvilml_registry::ModelRegistry::new(db.clone());
     ```
   - Spawn a non-blocking tokio task for the initial rescan:
     ```rust
     let scan_reg = registry.clone();
     let scan_dirs = cfg.model_dirs.clone();
     tokio::spawn(async move {
         match scan_reg.rescan(&scan_dirs).await {
             Ok(count) => tracing::info!(models_scanned = count, "initial model scan complete"),
             Err(e) => tracing::warn!("initial model scan failed: {}", e),
         }
     });
     ```
   - Pass `Arc::new(registry)` to `AppState::new_with_hardware()`:
     ```rust
     let state = AppState::new_with_hardware(
         env!("CARGO_PKG_VERSION"),
         hw_info,
         Some(db),
         Some(Arc::new(registry)),
     );
     ```

3. **Verify compilation** with `cargo check --workspace --features mock-hardware`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `registry: Arc<ModelRegistry>` field; update constructors and Clone impl |
| Modify | `backend/src/main.rs` | Build ModelRegistry after DB open, spawn rescan task, pass registry to AppState |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (existing) `crates/anvilml-server/src/lib.rs` | `health_returns_200` | Must be updated to pass a default registry when constructing AppState |
| (existing) `crates/anvilml-server/src/lib.rs` | `env_returns_200_with_stub_report` | Same update needed |
| (existing) `crates/anvilml-server/src/lib.rs` | `system_returns_200_with_hardware_info` | Same update needed |

The existing unit tests in `anvilml-server` construct `AppState::new()` / `AppState::new_with_hardware()` directly. They will need to be updated to pass the new registry parameter (e.g., `Some(Arc::new(ModelRegistry::new(pool)))` or a dedicated test helper). The plan does not write new test files — existing tests are adapted in-place as part of the implementation, per FORGE_AGENT_RULES §5.1.

## CI Impact

No CI workflow changes required. This task modifies only Rust source files within the existing crate dependency graph. The `cargo test --workspace --features mock-hardware` and `cargo clippy` gates will naturally cover the changes. No new jobs, no platform cross-check additions beyond the standard Windows check.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `SqlitePool` moved into `ModelRegistry`, unavailable for `AppState::db` | Use `db.clone()` — `SqlitePool` is an `Arc`-interned handle; cloning is cheap and safe. Both `ModelRegistry` and `AppState` share the same underlying pool. |
| `cfg.model_dirs` not `Clone` — would prevent moving it into the tokio closure | Verify `ModelDirConfig` derives/has `Clone`. It does (it's a simple struct with `PathBuf` and `Option<ModelKind>`). If needed, clone before spawning. |
| Rescan panics on invalid paths crashing the whole process | Wrap `rescan()` in `match` with `.await`, log errors via `tracing::warn!` rather than `.unwrap()`. The spawned task cannot propagate panics to the main task. |
| `ModelRegistry` not yet accessible from `main.rs` | `anvilml-registry` re-exports `ModelRegistry` in its `lib.rs`; `backend` already depends on `anvilml-registry` transitively via `anvilml-server`. No new imports needed. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (existing tests adapted for new AppState field)
- [ ] `AppState` struct contains a `pub registry: Arc<ModelRegistry>` field
- [ ] `main.rs` creates `ModelRegistry::new(db.clone())` after DB open and ghost-job reset
- [ ] `main.rs` spawns a non-blocking tokio task that calls `registry.rescan(&cfg.model_dirs)` and logs the scanned count
- [ ] The initial rescan does not block the server bind (server starts while scan runs in background)
- [ ] `Arc<ModelRegistry>` is stored in `AppState` and accessible to handlers via axum's `State` extractor
