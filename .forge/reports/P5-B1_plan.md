# Plan Report: P5-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-B1                                       |
| Phase       | 005 — SQLite Persistence                    |
| Description | backend: SqlitePool in AppState, real DB wired in main.rs |
| Depends on  | P5-A1, P5-A2, P5-A3                         |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T16:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire a real file-backed SQLite database into the AnvilML server lifecycle. Add `db: SqlitePool` to `AppState` so all HTTP handlers can access the persistent database. In `main.rs`, replace the in-memory placeholder pool with `registry::open(&cfg.db_path)`, pass the real pool to `detect_all_devices`, and log the database path at INFO level. When the task completes, starting the server creates `anvilml.db` on disk (observable via `ls anvilml.db`), and the config drift gate (`config_reference`) continues to pass.

## Scope

### In Scope
- **`crates/anvilml-server/src/state.rs`**: Add `pub db: sqlx::SqlitePool` field to `AppState`. Update `new_with_hardware` constructor to accept `SqlitePool` and store it.
- **`backend/src/main.rs`**: Replace the `open_in_memory` pool with `registry::open(&cfg.db_path).await`. Add `tracing::info!(path = %cfg.db_path.display(), "database opened")`. Pass real pool to `detect_all_devices`. Pass pool to `AppState::new_with_hardware`.
- **`crates/anvilml-server/Cargo.toml`**: Add `anvilml-registry = { path = "../anvilml-registry" }` dependency (needed so `main.rs` can call `registry::open` through the re-export).
- **`crates/anvilml-server/Cargo.toml`**: Bump patch version `0.1.6 → 0.1.7`.

### Out of Scope
- Modifying any handler functions to use the new `db` field (future tasks will query the DB through handlers).
- Adding migration files or seed files (P5-A1 and P5-A2 handle those).
- Adding tests for the wiring itself — the `config_reference` gate and the Runnable Proof (start server, check DB file) are sufficient acceptance criteria.
- Modifying `anvilml-registry` crate source code (P5-A1 already provides `open()`).

## Existing Codebase Assessment

The codebase is at Phase 004 state. `anvilml-registry::open()` (from P5-A1) is already implemented in `crates/anvilml-registry/src/db.rs` — it creates a file-backed `SqlitePool` with WAL mode, runs migrations via `sqlx::migrate!()`, and resets ghost jobs. It is re-exported as `pub use db::{open, open_in_memory}` in `lib.rs`.

`AppState` in `crates/anvilml-server/src/state.rs` currently has four fields: `start_time`, `version`, `env_report`, and `hardware`. The `new_with_hardware` constructor takes `version` and `hardware` and initialises the other two fields with defaults.

In `backend/src/main.rs`, a placeholder in-memory pool is created via `sqlx::SqlitePool::connect("sqlite::memory:")` at line 75. This pool is passed to `detect_all_devices()` but is never stored in `AppState`. The `AppState::new_with_hardware` call at line 102 does not receive the pool.

The `anvilml-server` crate already has `sqlx` in the workspace dependencies but does not list it as a direct dependency in its own `Cargo.toml`. However, `main.rs` (in the `backend` crate) already imports `sqlx::SqlitePool` directly, so no new dependency is needed in `anvilml-server` for the `AppState` struct itself.

The `config_reference` test compares TOML key sets between `ServerConfig::default()` and the checked-in `anvilml.toml`. This task does not modify `ServerConfig`, so the test will continue to pass without changes.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source       | Feature flags confirmed |
|--------|-------------------|-----------------|------------------|------------------------|
| crate  | sqlx              | 0.9.0           | Cargo.lock (MCP unavailable for rust-docs) | runtime-tokio, sqlite, json |
| crate  | anvilml-registry  | 0.1.3           | Cargo.toml path dep | none (path dependency) |

Note: The `rust-docs` MCP tool was not available for live verification. The version `0.9.0` was confirmed from the project's `Cargo.lock` and workspace `Cargo.toml`. The `SqlitePool::connect()` and `SqlitePool::connect_with()` APIs, `SqliteConnectOptions`, and `SqliteJournalMode::Wal` are confirmed to exist in sqlx 0.9.0 from the existing source code usage in `db.rs` and `main.rs`.

## Approach

1. **Add `db` field to `AppState` in `state.rs`.** Add `pub db: sqlx::SqlitePool` as a new field on the `AppState` struct. The `SqlitePool` type is already available from the workspace `sqlx` dependency. `SqlitePool` implements `Clone` (required for axum's `State` extractor), and it is `Send + Sync`, so it is safe to share across handler tasks.

2. **Update `new_with_hardware` constructor in `state.rs`.** Change the signature from `fn new_with_hardware(version: impl Into<String>, hardware: Arc<tokio::sync::RwLock<HardwareInfo>>) -> Self` to add a third parameter: `db: sqlx::SqlitePool`. Store it in the struct. Rationale: keeping the constructor chain simple — the existing `new()` constructor does not need a database (it is a stub), only `new_with_hardware` is called at real startup where the pool exists.

3. **Add `anvilml-registry` dependency to `anvilml-server/Cargo.toml`.** The `backend` crate already has `anvilml-registry` as a direct dependency, and `main.rs` is in the `backend` crate. However, the task says to pass the real pool through `AppState` into the server crate, and the registry's `open()` function is called from `main.rs` (backend crate). The `anvilml-server` crate itself does not call `open()` — only `main.rs` does. Therefore, the `anvilml-registry` dependency is NOT needed in `anvilml-server/Cargo.toml`. The pool is passed as a `sqlx::SqlitePool` value, not as a registry type. Update: this step is removed — no Cargo.toml change needed for `anvilml-server`.

   Correction: The `backend` crate already imports `anvilml_registry` (as `registry` in main.rs via `use anvilml_registry::open`). Looking at `main.rs` line 19, it imports `anvilml_hardware::detect_all_devices` directly. The `anvilml_registry` crate is already listed in `backend/Cargo.toml` at line 13. So the `registry::open()` call can be made from `main.rs` without any Cargo.toml changes.

4. **Replace in-memory pool with real pool in `main.rs`.** Remove the `open_in_memory` placeholder (lines 75–77). Replace it with:
   ```rust
   let pool = registry::open(&cfg.db_path).await.expect("failed to open database");
   ```
   Add a log line immediately after:
   ```rust
   tracing::info!(path = %cfg.db_path.display(), "database opened");
   ```
   This is the mandatory INFO log point per ENVIRONMENT.md §9 (Database subsystem, "SQLite file created" event).

5. **Pass real pool to `detect_all_devices`.** The `detect_all_devices` call at line 82 already accepts a `&SqlitePool`. Pass `&pool` instead of the in-memory pool variable.

6. **Pass pool to `AppState::new_with_hardware`.** Update the call at line 102 to include the pool: `AppState::new_with_hardware(env!("CARGO_PKG_VERSION"), Arc::new(tokio::sync::RwLock::new(hardware_info)), pool)`.

7. **Bump `anvilml-server` patch version.** Change `version = "0.1.6"` to `version = "0.1.7"` in `crates/anvilml-server/Cargo.toml` per §14 of FORGE_AGENT_RULES (crate version bump convention).

8. **Update the comment block in `main.rs`** at lines 71–77 that references "Phase 005" placeholder. Replace with a comment reflecting that the real database is now wired in.

## Public API Surface

| Item | Kind | Module Path | Description |
|------|------|-------------|-------------|
| `AppState::db` | `pub field` | `anvilml_server::state::AppState` | New `pub db: sqlx::SqlitePool` field on AppState |
| `AppState::new_with_hardware` | `pub fn` | `anvilml_server::state::AppState` | Updated signature: adds `db: sqlx::SqlitePool` parameter |

No new `pub` items are introduced in other crates. The `registry::open()` function was already public from P5-A1.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `db: SqlitePool` field; update `new_with_hardware` constructor |
| MODIFY | `backend/src/main.rs` | Replace in-memory pool with real `registry::open`; add log call; pass pool to `AppState` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.6 → 0.1.7` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/config_reference.rs` | `config_reference` | The `ServerConfig::default()` key set matches `anvilml.toml` key set. This task does not modify `ServerConfig`, so the test must still pass. | Workspace builds with `mock-hardware`. `anvilml.toml` exists. | None | Exit code 0 | `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0 |
| `backend/tests/config_reference.rs` | `config_reference` (full suite) | All workspace tests pass with `mock-hardware`. The new `AppState` field does not break any existing test. | Workspace builds with `mock-hardware`. | None | All tests pass, exit code 0 | `cargo test --workspace --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. The task modifies source files but does not add new test modules, new CI gates, or new file types. The existing `config-drift` CI job runs `cargo test -p anvilml --features mock-hardware -- config_reference`, which is unaffected by this task's changes. The `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which includes the `config_reference` test and will pass as long as the `AppState` changes compile correctly.

## Platform Considerations

None identified. The `SqlitePool` type and `sqlx::SqlitePool::connect()` are platform-neutral — SQLite is a single-file database with no platform-specific behavior at the connection level. The `cfg.db_path` is a `PathBuf` that resolves correctly on both Linux and Windows. The `anvilml.db` file will be created in the working directory, which is consistent across platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AppState::new_with_hardware` is called from tests that do not have a `SqlitePool` to pass. | Low | Medium | Only `new_with_hardware` is called from `main.rs` (production code). Tests use `AppState::new()` which does not require a pool. If any test calls `new_with_hardware`, add an `open_in_memory()` call in that test before constructing `AppState`. |
| `SqlitePool` does not implement `Clone`, breaking axum's `State` extractor. | Low | High | `SqlitePool` from sqlx 0.9.0 implements `Clone` (it wraps an `Arc` internally). Verified by checking the existing usage pattern in the codebase and sqlx documentation. If this changes, wrap in `Arc<SqlitePool>` instead. |
| `registry::open()` panics or returns `Err` when the database directory does not exist. | Low | Medium | `SqliteConnectOptions::create_if_missing(true)` (used in `open()`) creates the database file if it does not exist. The parent directory must exist; if `cfg.db_path` points to a non-existent parent, the error will be surfaced as a startup failure. This is acceptable — the operator should create the directory or use the default `./anvilml.db`. |
| `config_reference` test fails because adding `db` field to `AppState` changes some serialization path. | Very Low | Medium | The `config_reference` test only serialises `ServerConfig`, not `AppState`. No risk of failure from this task's changes. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] Starting the server creates `anvilml.db` in the working directory: `cargo run --features mock-hardware & sleep 2 && ls anvilml.db && kill %1` succeeds (file exists after server starts)
- [ ] Server log output contains `database opened` with the configured path: `cargo run --features mock-hardware 2>&1 | head -20 | grep -q "database opened"` exits 0
