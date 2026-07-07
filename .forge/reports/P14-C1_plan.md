# Plan Report: P14-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-C1                                        |
| Phase       | 14 — Dispatch & Execute                      |
| Description | anvilml-server: AppState gains scheduler/workers/db fields |
| Depends on  | P14-A5, P14-B1                                |
| Project     | anvilml                                       |
| Planned at  | 2026-07-07T21:55:00Z                          |
| Attempt     | 1                                             |

## Objective

Extend the `AppState` struct in `crates/anvilml-server/src/state.rs` with three new fields — `scheduler: Arc<JobScheduler>`, `workers: Arc<WorkerPool>`, and `db: SqlitePool` — continuing Phase 11's incremental, non-speculative field growth pattern. The `node_registry` field already exists and must not be duplicated. The remaining `ANVILML_DESIGN.md §13.2` fields (`hardware`, `broadcaster`, `artifact_store`, `env_report`) stay absent. Add corresponding integration tests so `state_tests.rs` reaches ≥5 tests total, and add the `sqlx` dependency that `SqlitePool` requires.

## Scope

### In Scope
- Add three fields to `AppState`: `scheduler: Arc<JobScheduler>`, `workers: Arc<WorkerPool>`, `db: SqlitePool`
- Add `sqlx` dependency (with `sqlite`, `runtime-tokio`, `migrate` features) to `crates/anvilml-server/Cargo.toml` so `SqlitePool` is importable
- Add `chrono` and `uuid` dev-dependencies to `crates/anvilml-server/Cargo.toml` (needed for constructing `JobScheduler` in tests)
- Write ≥3 new integration tests in `crates/anvilml-server/tests/state_tests.rs` covering the new fields' construction and cloning semantics
- Bump `anvilml-server` crate version from `0.1.4` to `0.1.5`

### Out of Scope
- Wiring `backend/main.rs` to construct and spawn a real `WorkerPool` and `JobScheduler` — explicitly deferred to P14-C2
- Adding remaining `§13.2` fields (`hardware`, `broadcaster`, `artifact_store`, `env_report`) — added only when a later task actually needs them
- Modifying any handler functions — handlers consuming the new fields is P14-D1's scope
- Modifying `build_router()` or `lib.rs` — no route changes in this task

## Existing Codebase Assessment

**What exists:** `AppState` currently has three fields (`config: Arc<ServerConfig>`, `node_registry: Arc<NodeTypeRegistry>`, `start_time: std::time::Instant`) defined in `crates/anvilml-server/src/state.rs`. The struct derives `Clone` and is re-exported from `lib.rs`. Phase 11 (P11-B1) created this struct with the initial two fields (`config`, `node_registry`), then P11-C1 folded `HealthState.start_time` into it. The incremental-growth pattern is well-established: each task adds the fields its handlers need, never speculatively ahead.

**Established patterns:** The `state_tests.rs` file uses inline struct construction (no constructor method) with `Arc::new()` wrapping for shared fields. Tests assert field accessibility and `Arc`-sharing semantics through clone observation. The existing test style is synchronous — no `#[tokio::test]` used yet. The `anvilml-scheduler` crate's `JobScheduler::new()` takes `JobStore` + `Arc<NodeTypeRegistry>`. The `anvilml-worker` crate's `WorkerPool::new()` is async and binds a `RouterTransport`. The `sqlx::SqlitePool` is used by `anvilml-registry`'s `create_pool()` but is not yet a direct dependency of `anvilml-server`.

**Gap between design doc and source:** `ANVILML_DESIGN.md §13.2` shows the full ten-field `AppState` shape. The current struct has three fields. This task brings it to six fields. The design doc shows `registry: Arc<ModelRegistry>` as a separate field (distinct from `node_registry: Arc<NodeTypeRegistry>`), but this field does not exist in the current source — it is added later (Phase 18, P18-C1). This is not a gap for this task; the design doc's full shape is aspirational, and the incremental pattern is the governing convention.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sqlx    | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |
| crate  | chrono  | 0.4 (workspace) | Cargo.lock     | serde (already in anvilml-registry) |
| crate  | uuid    | 1.23 (workspace) | Cargo.lock    | serde (already in anvilml-registry) |

The `sqlx` version matches the version used by `anvilml-registry`'s `Cargo.toml`, ensuring workspace consistency. Feature flags `sqlite`, `runtime-tokio`, `migrate`, and `chrono` are confirmed via the rust-docs MCP. The `chrono` and `uuid` versions match the workspace convention (declared in `anvilml-registry`).

## Approach

1. **Add `sqlx` dependency to `crates/anvilml-server/Cargo.toml`.**
   Add `sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "migrate", "chrono"] }` under `[dependencies]`. This makes `sqlx::SqlitePool` importable for the new `db` field type. The feature set matches `anvilml-registry`'s own `sqlx` declaration for consistency.

2. **Add dev-dependencies for test construction.**
   Add `chrono = { version = "0.4", features = ["serde"] }` and `uuid = { version = "1.23", features = ["v4"] }` under `[dev-dependencies]`. These are needed because constructing a `JobScheduler` in tests requires `JobStore` (which needs a `SqlitePool`) and `JobSettings` (which uses `uuid::Uuid` internally). The `chrono` crate is needed because `JobStore::new()` takes a `SqlitePool` and `JobScheduler::new()` is called in test setup.

3. **Extend `AppState` struct in `state.rs`.**
   Add three fields to the existing struct:
   ```rust
   pub scheduler: Arc<JobScheduler>,
   pub workers: Arc<WorkerPool>,
   pub db: SqlitePool,
   ```
   Update the `use` block to import `JobScheduler` from `anvilml_scheduler`, `WorkerPool` from `anvilml_worker`, and `SqlitePool` from `sqlx`. Add `///` doc comments for each new field explaining what it owns and its role in the server architecture. The field order follows the pattern: config-like fields first, then subsystem fields (`scheduler`, `workers`, `db`), then the existing `node_registry` and `start_time` at the end.

4. **Write ≥3 new integration tests in `state_tests.rs`.**
   - `test_app_state_with_new_fields`: Constructs `AppState` with all six fields (the three pre-existing + three new). Uses `JobScheduler::new()` with a minimal `JobStore` backed by an in-memory SQLite pool, `WorkerPool::new()` via `#[tokio::test]` (async), and a minimal `SqlitePool`. Verifies all fields are accessible. This test uses `#[tokio::main]` to provide the async runtime for `WorkerPool::new()`.
   - `test_app_state_clone_preserves_all_fields`: Constructs `AppState`, clones it, then asserts that all six fields on the clone are accessible and that the `Arc`-wrapped fields share the same underlying allocation (same technique as the existing `test_app_state_clone_shares_node_registry` test).
   - `test_app_state_scheduler_arc_sharing`: Registers a node type via the original state's `node_registry`, then reads through the cloned state's `node_registry` to verify the scheduler's shared `Arc<NodeTypeRegistry>` is visible through both clones (this tests the Arc-sharing of the scheduler's internal registry, building on the existing test pattern).

5. **Bump `anvilml-server` crate version.**
   Update `crates/anvilml-server/Cargo.toml` `[package] version` from `0.1.4` to `0.1.5`. Per `ENVIRONMENT.md §12`, only the patch version (Z) changes.

## Public API Surface

No new `pub` items are introduced. The only change is to the existing `pub struct AppState` in `anvilml_server::state`:

**Before:**
```rust
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub node_registry: Arc<NodeTypeRegistry>,
    pub start_time: std::time::Instant,
}
```

**After:**
```rust
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub node_registry: Arc<NodeTypeRegistry>,
    pub start_time: std::time::Instant,
    pub scheduler: Arc<JobScheduler>,
    pub workers: Arc<WorkerPool>,
    pub db: SqlitePool,
}
```

All three new fields are `pub` (consistent with existing field visibility). No new functions, traits, or types are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/Cargo.toml | Add `sqlx` dependency; add `chrono` and `uuid` dev-dependencies; bump version 0.1.4 → 0.1.5 |
| Modify | crates/anvilml-server/src/state.rs | Add `scheduler`, `workers`, `db` fields to `AppState` struct with doc comments |
| Modify | crates/anvilml-server/tests/state_tests.rs | Add ≥3 new integration tests for the new fields |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| crates/anvilml-server/tests/state_tests.rs | test_app_state_with_new_fields | Constructs `AppState` with all six fields including the three new ones; asserts field accessibility | `sqlx`, `anvilml-scheduler`, `anvilml-worker` available as dependencies | In-memory `SqlitePool`, empty `JobStore`, fresh `WorkerPool` | All six fields accessible; no panics on construction | `cargo test -p anvilml-server --test state_tests -- test_app_state_with_new_fields` exits 0 |
| crates/anvilml-server/tests/state_tests.rs | test_app_state_clone_preserves_all_fields | Cloning `AppState` preserves all six fields; `Arc`-wrapped fields share the same allocation | An `AppState` constructed with all fields | A constructed `AppState` | Clone has all six fields accessible; `Arc` pointers are identical (verified via `std::ptr::eq` or reference count check) | `cargo test -p anvilml-server --test state_tests -- test_app_state_clone_preserves_all_fields` exits 0 |
| crates/anvilml-server/tests/state_tests.rs | test_app_state_scheduler_arc_sharing | The `Arc<NodeTypeRegistry>` inside the scheduler is shared between original and cloned `AppState`; registering a node type via one clone is visible through the other | An `AppState` constructed with a `JobScheduler` containing an `Arc<NodeTypeRegistry>` | A constructed `AppState` with a scheduler | `cloned.scheduler.node_registry.list()` returns the same registered nodes as `state.scheduler.node_registry.list()` | `cargo test -p anvilml-server --test state_tests -- test_app_state_scheduler_arc_sharing` exits 0 |

The file will contain 5 tests total (2 pre-existing + 3 new), meeting the ≥5 requirement.

## CI Impact

No CI changes required. The task only modifies source files and tests within the existing `anvilml-server` crate. The `cargo test --workspace --features mock-hardware` CI job already compiles and tests this crate. Adding `sqlx` as a dependency does not change any CI job's behavior — it is a compile-time dependency that does not affect test collection or execution. The new tests are standard Rust integration tests collected automatically by `cargo test`.

## Platform Considerations

None identified. The `SqlitePool` type is platform-neutral — `sqlx`'s SQLite backend works identically on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::new()` is async and binds a TCP port; constructing it in a test may fail if no port is available or if the test runs in a headless environment where ZeroMQ cannot bind. | Medium | High | Use `#[tokio::test]` for the construction test; add a `set_up_test_workers` fallback via the `test-utils` feature if `WorkerPool::new()` fails. Alternatively, construct `WorkerPool` in a separate test that is gated behind a feature flag, or use the `test-utils`-gated `set_up_test_workers` pattern from `anvilml-worker`'s pool tests to inject mock handles. |
| Adding `sqlx` as a dependency of `anvilml-server` increases compile time significantly (sqlx does compile-time SQL verification). | Low | Low | Accept the compile-time cost — it is a one-time cost per dependency resolution. The feature set is minimal (sqlite only), and the workspace already compiles sqlx for `anvilml-registry`. |
| The `JobStore` type requires a `SqlitePool` to construct, and the pool needs the database migrations applied. In tests, this could fail if the migration path is incorrect relative to the `anvilml-server` crate's working directory. | Medium | High | Use an in-memory SQLite database (`:memory:`) for test pools, which requires no migration files and no filesystem access. This is the same pattern used by `anvilml-registry`'s `db_tests.rs`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test state_tests` exits 0 (≥5 tests total in file)
- [ ] `grep -c "^fn test_" crates/anvilml-server/tests/state_tests.rs` outputs ≥5
- [ ] `grep "scheduler:" crates/anvilml-server/src/state.rs` finds the new `scheduler` field
- [ ] `grep "workers:" crates/anvilml-server/src/state.rs` finds the new `workers` field
- [ ] `grep "db:" crates/anvilml-server/src/state.rs` finds the new `db` field
- [ ] `grep "sqlx" crates/anvilml-server/Cargo.toml` finds the sqlx dependency
- [ ] `grep 'version = "0.1.5"' crates/anvilml-server/Cargo.toml` finds the bumped version
