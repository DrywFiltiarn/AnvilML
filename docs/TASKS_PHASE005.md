# Tasks: Phase 005 — SQLite Persistence

| Field | Value |
|-------|-------|
| Phase | 005 |
| Name | SQLite Persistence |
| Milestone group | Observable system state |
| Depends on phases | 1-4 |
| Task file | `forge/tasks/tasks_phase005.json` |
| Tasks | 4 |

## Overview

Phase 5 brings up the database: migration files for `jobs`/`models`/`artifacts`, a `db::open` that sets WAL PRAGMAs and runs migrations, and the startup ghost-job reset. After this phase the running binary creates and migrates `anvilml.db` on first start and resets any jobs left in `Running`/`Queued` from a previous run.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P5-A1 | `backend/migrations/001_jobs.sql` | anvilml-registry: SQLite migration files (jobs, models, artifacts) |
| P5-A2 | `src/db.rs` | anvilml-registry: db::open with PRAGMAs and migration runner |
| P5-A3 | anvilml-registry | anvilml-registry: ghost-job reset query |
| P5-A4 | anvilml | anvilml: open DB and run migrations + ghost reset at startup |

## Task details

#### P5-A1: anvilml-registry: SQLite migration files (jobs, models, artifacts)

- **Prereqs:** P4-A6
- **Tags:** —

Create backend/migrations/001_jobs.sql, 002_models.sql, 003_artifacts.sql exactly per ANVILML_DESIGN 13 (column names/types/indices verbatim). No code yet. These are consumed by P5-A2. Verify they are valid SQL by eye against the design; the migration runner test in P5-A2 will execute them.

#### P5-A2: anvilml-registry: db::open with PRAGMAs and migration runner

- **Prereqs:** P5-A1
- **Tags:** —

Add sqlx (sqlite, runtime-tokio, macros, migrate) + anvilml-core to anvilml-registry. Create src/db.rs: async fn open(path:&Path)->Result<SqlitePool,AnvilError>. Set PRAGMA journal_mode=WAL, synchronous=NORMAL, foreign_keys=ON, then sqlx::migrate!(relative path to backend/migrations). Re-export from lib.rs. cargo test -p anvilml-registry -- db exits 0: tempfile DB, run open, query sqlite_master, assert jobs+models+artifacts tables exist.

#### P5-A3: anvilml-registry: ghost-job reset query

- **Prereqs:** P5-A2
- **Tags:** —

Add to db.rs: async fn reset_ghost_jobs(pool:&SqlitePool)->Result<u64,AnvilError> running UPDATE jobs SET status='Failed', error='server_restart' WHERE status IN ('Running','Queued'); return rows affected. cargo test -p anvilml-registry -- ghost exits 0: insert 2 Running + 1 Completed job rows, call reset, assert 2 updated and Completed untouched.

#### P5-A4: anvilml: open DB and run migrations + ghost reset at startup

- **Prereqs:** P5-A3
- **Tags:** —

Add anvilml-registry + sqlx to backend; add db: SqlitePool to AppState. In main.rs startup BEFORE binding: db = anvilml_registry::db::open(&cfg.db_path), then reset_ghost_jobs(&db) logging count reset. Store pool in AppState. Verify: delete any anvilml.db, cargo run --features mock-hardware, confirm anvilml.db created with WAL files; sqlite3 anvilml.db '.tables' lists jobs/models/artifacts; restart logs '0 ghost jobs reset'.


## Runnable Proof

Delete any existing DB, start the server, and confirm the database is created and migrated.

```bash
rm -f anvilml.db anvilml.db-wal anvilml.db-shm
cargo run --features mock-hardware
# another terminal:
sqlite3 anvilml.db '.tables'      # jobs  models  artifacts
ls anvilml.db*                    # anvilml.db  anvilml.db-wal  anvilml.db-shm
```

Expected: `anvilml.db` exists with WAL sidecar files; `.tables` lists `jobs`, `models`, `artifacts`. On a second startup the log shows `0 ghost jobs reset` (or N if you manually insert a Running row). Phase done when the DB is created/migrated on startup and `cargo test -p anvilml-registry` is green.
