# Plan Report: P900-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A1                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-registry: retrofit INFO logging to db.rs (DB create, migrations, up-to-date) |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T21:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Retrofit three mandatory INFO-level log points into `anvilml_registry::db::open()` so that database lifecycle events are observable at the default log level, per FORGE_AGENT_RULES.md §11.3 and ENVIRONMENT.md §9 (Database sub-table).

## Scope

### In Scope
- Add `tracing::info!(path=%path.display(), "database created")` when the database file did not previously exist (checked via `Path::exists()` before `SqliteConnectOptions`).
- Add `tracing::debug!(path=%path.display(), "database exists")` when the database file already exists.
- After `Migrator::run()`, query `_sqlx_migrations` for applied migrations and log each at `tracing::info!(migration=%name, version=%ver)`.
- If zero migrations were applied (all already up to date), log `tracing::info!(migrations_applied=0, "database schema up to date")`.
- No logic changes: existing behaviour, error handling, and tests remain untouched.

### Out of Scope
- Any logging in `open_in_memory()` (no filesystem path; §11.3 does not apply).
- Logging in `reset_ghost_jobs()`.
- Any changes to Cargo.toml, migrations, or test files.
- Logging in any other crate or file.

## Approach

1. **Import check.** Confirm `use tracing;` is already available — `tracing` is declared as `{ workspace = true }` in `crates/anvilml-registry/Cargo.toml` (line 13). No import needed beyond what `sqlx::query` and other existing code may already bring in implicitly via the crate's own imports. Add `use tracing;` at the top of `db.rs` if not present (the `tracing` crate is used by other modules in this crate, so it is available as a dependency).

2. **Pre-connect path check.** In `open()`, before constructing `SqliteConnectOptions`, insert:
   ```rust
   if !path.exists() {
       tracing::info!(path = %path.display(), "database created");
   } else {
       tracing::debug!(path = %path.display(), "database exists");
   }
   ```
   This goes immediately before the existing `let opts = SqliteConnectOptions::new()...` line.

3. **Post-migration query.** After `MIGRATIONS.run(&pool).await.map_err(migrate_error)?;`, insert a query against `_sqlx_migrations`:
   ```rust
   let rows = sqlx::query("SELECT version, description FROM _sqlx_migrations WHERE success = TRUE ORDER BY installed_on")
       .fetch_all(&pool)
       .await
       .map_err(sqlx_error)?;

   if rows.is_empty() {
       tracing::info!(migrations_applied = 0, "database schema up to date");
   } else {
       for row in &rows {
           let version: i64 = row.get("version");
           let description: String = row.get("description");
           tracing::info!(migration = %description, version = version, "migration applied");
       }
   }
   ```
   This goes immediately after the `MIGRATIONS.run(...)` line and before the pragmas are set (the pragmas can run either way; placing migration logging right after `run()` keeps it logically grouped).

4. **Verify compilation.** Run `cargo check -p anvilml-registry --features mock-hardware` to confirm no compile errors.

5. **Run tests.** Execute `cargo test -p anvilml-registry -- db` and confirm exit 0 with no regressions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/db.rs` | Add pre-connect path check logging; add post-migration query and per-migration INFO logging; add "up to date" INFO log |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/db.rs` (tests module) | `test_open_creates_tables` | Existing test still passes — tables created correctly after logging added |
| `crates/anvilml-registry/src/db.rs` (tests module) | `test_reset_ghost_jobs` | Existing test still passes — ghost-job reset unaffected by logging changes |
| `crates/anvilml-registry/src/db.rs` (tests module) | `test_open_creates_file_if_missing` | Existing test still passes — file creation flow unchanged |

No new test files are required. The task adds only log calls; no behavioural change means existing tests continue to pass as-is.

## CI Impact

No CI changes required. No Cargo.toml modifications, no new dependencies, no new test files, and no changes to any CI workflow files. The `tracing` dependency is already declared in the crate's `Cargo.toml`. All existing CI gates (format, clippy, tests, cross-checks) will run unchanged and must continue to pass.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `_sqlx_migrations` column names differ from `version`/`description` in the installed sqlx version | Low | Medium — query would fail to compile | Task notes say to consult `rust-docs` MCP if needed; verify by checking sqlx 0.9 source or docs.rs for the actual column names |
| Adding log calls changes control flow or error paths, causing test regressions | Very low | Medium — tests would fail | Only insert non-branching tracing calls; no new `if`/`match` guards that affect logic |
| `path.exists()` check races with concurrent file creation | Low | Negligible — log may say "created" for a file created between the check and connect, but this is harmless INFO noise | Accept as-is; the task scope does not require race-free logging |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-registry --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-registry -- db` exits 0 with no regressions
- [ ] `open()` logs `tracing::info!(path=...) "database created"` when the file does not exist
- [ ] `open()` logs `tracing::debug!(path=...) "database exists"` when the file already exists
- [ ] After migrations run, each applied migration logs at INFO with `migration=` and `version=` fields
- [ ] When zero migrations apply, `tracing::info!(migrations_applied=0, "database schema up to date")` is emitted
- [ ] No logic changes: existing test assertions remain valid, no new dependencies added
