# Implementation Report: P4-A1

| Field          | Value                                       |
|----------------|---------------------------------------------|
| Task ID        | P4-A1                                       |
| Phase          | 004 — Persistence & Model Registry           |
| Description    | anvilml-registry: SQLite migrations (jobs, models, artifacts) |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T16:30:00Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the three SQLite migration files and the `db::open` function for the AnvilML registry system. Created `backend/migrations/001_jobs.sql`, `002_models.sql`, and `003_artifacts.sql` defining the complete database schema (jobs, models, artifacts tables with their indices) per ANVILML_DESIGN §13. Implemented `crates/anvilml-registry/src/db.rs` with `pub async fn open()` that creates a `SqlitePool`, sets WAL-mode pragmas (`journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`), and applies all migrations via `sqlx::migrate!("../../backend/migrations")`. Updated `Cargo.toml` to add `sqlx` (features: sqlite, runtime-tokio-rustls, macros, migrate) and `tokio` (full) dependencies. Added an integration test that opens a temporary database, runs migrations, and verifies all three tables exist via `sqlite_master`.

## Files Changed

| Action   | Path                                      | Description                                          |
|----------|-------------------------------------------|------------------------------------------------------|
| CREATE   | backend/migrations/001_jobs.sql           | jobs table + idx_jobs_status + idx_jobs_created_at   |
| CREATE   | backend/migrations/002_models.sql         | models table + idx_models_kind                       |
| CREATE   | backend/migrations/003_artifacts.sql      | artifacts table + idx_artifacts_job_id               |
| MODIFY   | crates/anvilml-registry/Cargo.toml        | Added sqlx, tokio, tempfile dependencies             |
| CREATE   | crates/anvilml-registry/src/db.rs         | pub async fn open() — pool init, pragmas, migrations |
| MODIFY   | crates/anvilml-registry/src/lib.rs        | Exposed db module + integration test                 |

## Test Results

### Unit tests (anvilml-registry)
```
running 1 test
test tests::test_migrations_create_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Full workspace tests (--features mock-hardware)
```
test result: ok. 52 passed; 0 failed (anvilml-core)
test result: ok. 43 passed; 0 failed (anvilml-hardware)
test result: ok. 18 passed; 0 failed (anvilml-ipc)
test result: ok. 1 passed; 0 failed (anvilml-registry)
test result: ok. 1 passed; 0 failed (anvilml-scheduler)
test result: ok. 1 passed; 0 failed (anvilml-server)
test result: ok. 1 passed; 0 failed (anvilml-worker)
```

### Clippy (--workspace --features mock-hardware -- -D warnings)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.71s
# Passed — zero warnings
```

### Format (`cargo fmt --all`)
```
# Passed — zero changes needed
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P4-A1_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
A  backend/migrations/001_jobs.sql
A  backend/migrations/002_models.sql
A  backend/migrations/003_artifacts.sql
M  crates/anvilml-registry/Cargo.toml
A  crates/anvilml-registry/src/db.rs
M  crates/anvilml-registry/src/lib.rs
```

## Acceptance Criteria — Verification

| Criterion                                                 | Status | Evidence                                        |
|-----------------------------------------------------------|--------|-------------------------------------------------|
| `cargo test -p anvilml-registry -- db` exits 0            | PASS   | `test tests::test_migrations_create_tables ... ok` |
| All three tables exist after migration                    | PASS   | sqlite_master query returns artifacts, jobs, models |
| WAL mode pragmas set (journal_mode, synchronous, foreign_keys) | PASS   | PRAGMAs executed before migrations in db::open  |
| Migrations applied via `sqlx::migrate!`                   | PASS   | `sqlx::migrate!("../../backend/migrations").run(&pool)` |
| `cargo clippy --workspace --features mock-hardware -D warnings` passes | PASS | Zero warnings across all crates                  |
| `cargo fmt --all` — no formatting changes needed         | PASS   | Clean format run                                 |
| Full workspace tests pass with zero failures             | PASS   | 115 tests passed, 0 failed                      |
