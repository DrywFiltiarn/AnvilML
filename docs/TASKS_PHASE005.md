# Tasks: Phase 005 — SQLite Persistence

| Field | Value |
|-------|-------|
| Phase | 005 |
| Name | SQLite Persistence |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 4 |

## Overview

Phase 005 implements the SQLite layer that all subsequent persistent features depend on. The database is opened via `sqlx` with WAL mode enabled, migrations are run automatically at startup, and ghost jobs (jobs left in Queued or Running state from a prior run) are reset to Failed.

The `SeedLoader` in `anvilml-registry` runs SHA256-gated SQL seed files from `backend/seeds/`. On first run it inserts the device capability rows; on subsequent runs it skips files whose SHA256 hash has not changed. This prevents re-seeding on every restart while still applying updates when seed files are modified.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-registry | P5-A1 … P5-A3 | db.rs open/migrate, ghost reset, SeedLoader |
| B | backend | P5-B1 | Wire SqlitePool into main.rs and AppState |

## Prerequisites

Phase 004 complete: `AnvilError` and `ServerConfig` exist. `backend/seeds/devices.sql` will be created in this phase.

## Task Descriptions

### Group A — anvilml-registry

#### P5-A1: anvilml-registry: db.rs open, migrate, ghost reset

**Goal:** Implement `pub async fn open(path: &Path) -> Result<SqlitePool>` in `crates/anvilml-registry/src/db.rs`. Enable WAL mode. Run `sqlx::migrate!("../backend/migrations")`. Reset ghost jobs. Log each migration applied and the "up-to-date" message when none apply.

**Files to create:**
- `backend/migrations/001_initial.sql` — tables: `jobs`, `models`, `artifacts`, `seed_history`
- `crates/anvilml-registry/src/db.rs` — `open()`, `open_in_memory()`, ghost reset logic

**Acceptance criterion:** `cargo test -p anvilml-registry -- db` exits 0; DB file created on first call, tables present.

#### P5-A2: anvilml-registry: SeedLoader SHA256-gated SQL runner

**Goal:** Implement `pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<()>` in `crates/anvilml-registry/src/seed_loader.rs`. Hash each `.sql` file; skip if hash matches `seed_history`; execute and record if changed.

**Files to create:**
- `crates/anvilml-registry/src/seed_loader.rs`
- `backend/seeds/devices.sql` — `INSERT OR IGNORE INTO device_capabilities` rows for major NVIDIA and AMD GPUs

**Acceptance criterion:** `cargo test -p anvilml-registry -- seed` exits 0; running twice skips on second run (hash match).

#### P5-A3: anvilml-registry: open_in_memory for tests

**Goal:** Implement `pub async fn open_in_memory() -> Result<SqlitePool>` that creates an in-memory SQLite database with migrations applied. Used by all test code in place of a real file.

**Acceptance criterion:** `cargo test -p anvilml-registry` exits 0 with all tests using in-memory pools (no temp files).

### Group B — backend

#### P5-B1: backend: SqlitePool in AppState + startup wiring

**Goal:** Add `db: SqlitePool` to `AppState`. In `main.rs` open the DB with `registry::open(cfg.db_path)`, wire real pool into `detect_all_devices`. Update `anvilml.toml` and `config_reference` test if any new config key is added.

**Acceptance criterion:** Start server; verify `anvilml.db` created; restart server; verify ghost-job reset log message visible; `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-registry
cargo test -p anvilml --features mock-hardware -- config_reference
cargo run --features mock-hardware &
sleep 2
ls anvilml.db
grep -q "database" /dev/stdin <<< "$(cargo run --features mock-hardware 2>&1 | head -20)"
kill %1
```

## Known Constraints and Gotchas

- `sqlx` requires the `sqlite` and `runtime-tokio` features. Add to workspace dependencies.
- The `seed_history` table must be part of `001_initial.sql` — it is needed before seeds can run.
- `open_in_memory()` uses `sqlite::memory:` URL with `sqlx::SqliteConnectOptions`. Run migrations the same way as the file-backed pool.
- Ghost-job reset: `UPDATE jobs SET status='Failed', error='server_restart' WHERE status IN ('Queued','Running')`. Run this after migrations, before workers start.
