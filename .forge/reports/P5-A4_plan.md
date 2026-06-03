# Plan Report: P5-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A4                                         |
| Phase       | 005 — SQLite Persistence                      |
| Description | anvilml: open DB and run migrations + ghost reset at startup |
| Depends on  | P5-A1, P5-A2, P5-A3                           |
| Project     | anvilml                                       |
| Planned at  | 2026-06-03T19:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Integrate the existing `anvilml-registry` database module (P5-A2/P5-A3) into the backend launcher (`backend/src/main.rs`) so that on every startup the SQLite database is opened, migrations are run, and any ghost jobs from a previous unclean exit are reset to `Failed` — all before binding the HTTP server.

## Scope

### In Scope
- Add `anvilml-registry` dependency to `backend/Cargo.toml`
- Add `db: SqlitePool` field to `AppState` in `crates/anvilml-server/src/state.rs`
- Update `AppState::new_with_hardware()` constructor signature and `Clone` impl to accept/store the pool
- Modify `backend/src/main.rs` startup sequence to:
  - Open the database via `anvilml_registry::db::open(&cfg.db_path)` after hardware detection (step 3 of §16.2)
  - Call `anvilml_registry::db::reset_ghost_jobs(&db)` and log the count reset
  - Pass the pool to `AppState::new_with_hardware()` when building state
- No new migration files (already created in P5-A1)
- No new database functions (already created in P5-A2/P5-A3)

### Out of Scope
- Modifying any handler code to use `AppState.db` (future tasks will wire up job/model/artifact CRUD through the pool)
- Adding a `db` field to `anvilml-server/src/lib.rs` router builder (the pool is already in AppState, which is passed to `build_router`)
- Changes to CI workflow files
- Schema changes to migration SQL files
- Worker management or scheduler integration with DB
- Tests for the main.rs integration (runtime smoke test only, per task description)

## Approach

1. **Update `backend/Cargo.toml`** — add `anvilml-registry = { path = "../crates/anvilml-registry" }` to `[dependencies]`. No new top-level sqlx dependency is needed since the backend gets it transitively through `anvilml-registry`.

2. **Update `AppState` in `crates/anvilml-server/src/state.rs`** — add a `pub db: sqlx::SqlitePool` field to the struct, update `new()` and `new_with_hardware()` constructors to accept a `SqlitePool` parameter, and update the `Clone` impl to clone the pool reference. Add `sqlx` as a dependency to `anvilml-server/Cargo.toml` if not already present (it is needed for the `SqlitePool` type in the struct).

3. **Update `backend/src/main.rs`** — after hardware detection (line ~123) and before building AppState (line ~136):
   - Call `let db = anvilml_registry::db::open(&cfg.db_path).await?;`
   - Call `let ghost_count = anvilml_registry::db::reset_ghost_jobs(&db).await?;`
   - Log: `tracing::info!(ghost_jobs_reset = ghost_count, "ghost jobs reset");`
   - Pass `db` to `AppState::new_with_hardware(version, hw_info, db)`

4. **Verify** — delete any existing `anvilml.db`, run `cargo run --features mock-hardware`, confirm DB files are created with WAL mode sidecars (`anvilml.db-wal`, `anvilml.db-shm`), and that the log line `X ghost jobs reset` appears on startup.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `anvilml-registry` dependency |
| Modify | `crates/anvilml-server/src/state.rs` | Add `db: SqlitePool` field, update constructors and Clone impl |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `sqlx` dependency (for `SqlitePool` type in AppState) |
| Modify | `backend/src/main.rs` | Open DB, reset ghost jobs, pass pool to AppState before server bind |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (existing) `crates/anvilml-registry/src/db.rs::tests::test_open_creates_tables` | P5-A2 test | DB open creates jobs/models/artifacts tables |
| (existing) `crates/anvilml-registry/src/db.rs::tests::test_reset_ghost_jobs` | P5-A3 test | Ghost job reset marks Running/Queued as Failed, leaves Completed untouched |
| (runtime smoke) `cargo run --features mock-hardware` | Manual verification | DB file created with WAL sidecars; `.tables` shows jobs/models/artifacts; restart log shows ghost count |

No new unit test files are added. The existing `anvilml-registry` tests cover the db module. The task description specifies runtime smoke testing (delete DB, run, check files, restart).

## CI Impact

No CI workflow file changes required. The existing CI commands (`cargo fmt`, `cargo clippy --features mock-hardware`, `cargo test --features mock-hardware`) will automatically pick up the new code. The platform cross-check (`cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`) must also pass since no platform-specific code is introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `anvilml-server/Cargo.toml` may not have `sqlx` as a dependency, causing compilation failure when `AppState` references `SqlitePool` | Add `sqlx = { version = "0.9", features = ["sqlite", "runtime-tokio"] }` to `anvilml-server/Cargo.toml`; verify the version matches what `anvilml-registry` uses (0.9). |
| Migration path in `anvilml-registry/src/db.rs` (`sqlx::migrate!("../../backend/migrations")`) is relative to `CARGO_MANIFEST_DIR` — if this resolves incorrectly when building from the workspace root, migrations won't run | The path was already verified working in P5-A2 (tests pass). No change needed. |
| Changing `AppState` constructor signature may break other callers (e.g., tests, `anvilml-openapi`) | Update all call sites: `backend/src/main.rs` is the primary consumer; check `anvilml-server` tests and `anvilml-openapi` for any direct `AppState::new` usage. |
| Database open failure panics before server bind | Use `.expect()` or `?` to propagate error early (matching current pattern: hardware detection already uses `.expect()`). The task description says "open BEFORE binding" — if open fails, the process should exit with an error. |

## Acceptance Criteria

- [ ] `backend/Cargo.toml` lists `anvilml-registry` in `[dependencies]`
- [ ] `AppState` struct contains a `pub db: sqlx::SqlitePool` field
- [ ] `main.rs` opens the database via `anvilml_registry::db::open(&cfg.db_path)` before binding
- [ ] `main.rs` calls `reset_ghost_jobs(&db)` and logs the count (e.g., `"0 ghost jobs reset"`)
- [ ] `SqlitePool` is passed to `AppState` constructor
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes
- [ ] Delete `anvilml.db`, run `cargo run --features mock-hardware`, confirm `anvilml.db` + WAL files created
- [ ] `sqlite3 anvilml.db '.tables'` lists `jobs models artifacts`
- [ ] Second restart logs `0 ghost jobs reset` (or N if manual rows inserted)
