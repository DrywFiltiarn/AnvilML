# Plan Report: P4-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A1                                       |
| Phase       | 004 — Persistence & Model Registry          |
| Description | anvilml-registry: SQLite migrations (jobs, models, artifacts) |
| Depends on  | P3-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T14:16:37Z                        |
| Attempt     | 1                                           |

## Objective

Create the three SQLite migration files that define the entire database schema for the AnvilML registry system, and implement the `db::open` function in `anvilml-registry` that initializes a `SqlitePool` with WAL mode pragmas and applies all migrations via `sqlx::migrate!`. This establishes the authoritative schema for `jobs`, `models`, and `artifacts` tables, which are shared dependencies of the scheduler (phase 006), the HTTP server (phases 007–008), and the model registry store (P4-A3).

## Scope

### In Scope
- Create `backend/migrations/001_jobs.sql` — `jobs` table + `idx_jobs_status` + `idx_jobs_created_at` indices, per ANVILML_DESIGN §13
- Create `backend/migrations/002_models.sql` — `models` table + `idx_models_kind` index, per ANVILML_DESIGN §13
- Create `backend/migrations/003_artifacts.sql` — `artifacts` table + `idx_artifacts_job_id` index, per ANVILML_DESIGN §13
- Add `sqlx` dependency (features: `sqlite`, `runtime-tokio-native-tls`, `macros`, `migrate`) and `tokio` (features: `full`) to `crates/anvilml-registry/Cargo.toml`
- Implement `crates/anvilml-registry/src/db.rs` with `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>` that sets PRAGMAs and runs migrations
- Update `crates/anvilml-registry/src/lib.rs` to expose `db::open`
- Write integration test in `anvilml-registry` that opens a temp DB, applies all migrations, and asserts all three tables exist via `sqlite_master`

### Out of Scope
- Model scanner implementation (P4-A2)
- ModelRegistry store CRUD operations (P4-A3)
- Ghost job reset logic (handled at server startup in phase 008)
- Any HTTP handler or API code
- Any worker process code
- CI workflow changes (no new CI jobs needed for this task)
- Offline mode (`sqlx prepare`) — not required at this phase per TASKS_PHASE004.md
- Any changes to the `backend` crate itself (migrations live in `backend/migrations/`, but only the SQL files are created here; the backend Cargo.toml is not modified)

## Approach

1. **Create migration 001_jobs.sql**: Write `CREATE TABLE IF NOT EXISTS jobs (...)` with columns `id TEXT PRIMARY KEY`, `status TEXT NOT NULL`, `graph TEXT NOT NULL`, `settings TEXT NOT NULL`, `device_index INTEGER`, `created_at TEXT NOT NULL`, `started_at TEXT`, `completed_at TEXT`, `worker_id TEXT`, `artifact_count INTEGER NOT NULL DEFAULT 0`, `error TEXT`. Add two indices: `CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)` and `CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at)`. Column names and types must match ANVILML_DESIGN §13 exactly.

2. **Create migration 002_models.sql**: Write `CREATE TABLE IF NOT EXISTS models (...)` with columns `id TEXT PRIMARY KEY`, `name TEXT NOT NULL`, `path TEXT NOT NULL UNIQUE`, `kind TEXT NOT NULL`, `size_bytes INTEGER NOT NULL`, `dtype_hint TEXT NOT NULL`, `vram_estimate_mib INTEGER NOT NULL`, `scanned_at TEXT NOT NULL`. Add index: `CREATE INDEX IF NOT EXISTS idx_models_kind ON models(kind)`.

3. **Create migration 003_artifacts.sql**: Write `CREATE TABLE IF NOT EXISTS artifacts (...)` with columns `hash TEXT PRIMARY KEY`, `job_id TEXT NOT NULL`, `width INTEGER NOT NULL`, `height INTEGER NOT NULL`, `format TEXT NOT NULL DEFAULT 'png'`, `seed INTEGER NOT NULL`, `steps INTEGER NOT NULL`, `prompt TEXT NOT NULL`, `created_at TEXT NOT NULL`. Add index: `CREATE INDEX IF NOT EXISTS idx_artifacts_job_id ON artifacts(job_id)`.

4. **Update anvilml-registry Cargo.toml**: Add `sqlx = { version = "0.9", features = ["sqlite", "runtime-tokio-native-tls", "macros", "migrate"] }` and `tokio = { version = "1", features = ["full"] }`. Keep existing package metadata. No workspace dependency table exists yet, so dependencies are declared inline.

5. **Implement db.rs**: Create `crates/anvilml-registry/src/db.rs` with:
   - `pub async fn open(path: &std::path::Path) -> Result<sqlx::SqlitePool, anvilml_core::error::AnvilError>`
   - Build connection string from path: `sqlite:<absolute_path>`
   - Create pool via `SqlitePoolOptions::new()` with appropriate max connections
   - Execute four PRAGMAs on a fresh connection using `sqlx::query("PRAGMA ...")`: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`
   - Run migrations via `sqlx::migrate!("../../backend/migrations")` — path is relative to the crate manifest at `crates/anvilml-registry/Cargo.toml` and must resolve to `backend/migrations/`
   - Return the configured pool
   - Convert sqlx errors to `AnvilError::DbError` via `map_err`

6. **Update lib.rs**: Replace the existing placeholder test with `pub mod db;` to expose the database module. Keep the existing test module.

7. **Write migration test**: In `crates/anvilml-registry/src/db.rs` (or a dedicated `tests/` directory), write an async test that:
   - Creates a temp file path via `tempfile::NamedTempFile` or `std::env::temp_dir()` + unique name
   - Calls `db::open(&path)` to apply all migrations
   - Queries `SELECT name FROM sqlite_master WHERE type='table' ORDER BY name` and asserts the result set contains exactly `"artifacts"`, `"jobs"`, `"models"`
   - Also queries `SELECT name FROM sqlite_master WHERE type='index' ORDER BY name` and asserts the migration-created indices exist

## Files Affected

| Action   | Path                                              | Description                                                    |
|----------|---------------------------------------------------|----------------------------------------------------------------|
| CREATE   | backend/migrations/001_jobs.sql                   | `jobs` table DDL + 2 indices per ANVILML_DESIGN §13           |
| CREATE   | backend/migrations/002_models.sql                 | `models` table DDL + 1 index per ANVILML_DESIGN §13           |
| CREATE   | backend/migrations/003_artifacts.sql              | `artifacts` table DDL + 1 index per ANVILML_DESIGN §13        |
| MODIFY   | crates/anvilml-registry/Cargo.toml                | Add `sqlx` (sqlite, runtime-tokio-native-tls, macros, migrate) and `tokio` (full) dependencies |
| CREATE   | crates/anvilml-registry/src/db.rs                 | `pub async fn open(path)` — pool init, PRAGMAs, migration runner |
| MODIFY   | crates/anvilml-registry/src/lib.rs                | Add `pub mod db;` to expose the database module               |

## Tests

| Test ID / Name            | File                                     | Validates                                                     |
|---------------------------|------------------------------------------|---------------------------------------------------------------|
| `migration_applies_all_tables` | `crates/anvilml-registry/src/db.rs` (or `tests/`) | Opening a fresh DB via `db::open()` creates all three tables (`jobs`, `models`, `artifacts`) and their indices, confirmed by querying `sqlite_master` |

## CI Impact

No CI changes required. This task does not modify any CI workflow files. The existing CI matrix in `.github/workflows/ci.yml` already runs `cargo test --workspace --features mock-hardware`, which will include the new `anvilml-registry` tests once the dependencies are added. No new CI jobs or steps are needed.

## Risks and Mitigations

| Risk                                      | Likelihood | Impact | Mitigation                                                      |
|-------------------------------------------|-----------|--------|-----------------------------------------------------------------|
| `sqlx::migrate!` path resolution fails    | Medium     | High   | Verify `../../backend/migrations` resolves from crate root; test with a real temp DB in the same session. Use absolute path via `env::current_dir()` as fallback if needed. |
| `sqlx` compile-time query check requires DATABASE_URL at compile time | Medium | Medium | Use runtime-only queries (`query!` without offline mode) for this phase; set `DATABASE_URL=sqlite::memory:` in dev `.env` only if compile-time checking is desired. Per TASKS_PHASE004.md, offline mode is not required. |
| Temp DB path conflicts in concurrent CI   | Low       | Low    | Use `tempfile::NamedTempFile` with automatic cleanup, or generate unique paths via `Uuid::new_v4()` appended to a temp directory base. |
| sqlx 0.9 API changes from task spec       | Low        | Medium | Verify feature names against docs.rs (confirmed: `sqlite`, `runtime-tokio-native-tls`, `macros`, `migrate` exist in 0.9.0). |

## Acceptance Criteria

- [ ] `backend/migrations/001_jobs.sql` exists and contains the exact `jobs` table DDL and two indices from ANVILML_DESIGN §13
- [ ] `backend/migrations/002_models.sql` exists and contains the exact `models` table DDL and one index from ANVILML_DESIGN §13
- [ ] `backend/migrations/003_artifacts.sql` exists and contains the exact `artifacts` table DDL and one index from ANVILML_DESIGN §13
- [ ] `crates/anvilml-registry/Cargo.toml` includes `sqlx` with features `sqlite`, `runtime-tokio-native-tls`, `macros`, `migrate`
- [ ] `crates/anvilml-registry/Cargo.toml` includes `tokio` with feature `full`
- [ ] `crates/anvilml-registry/src/db.rs` exports `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>`
- [ ] `db::open` sets `PRAGMA journal_mode=WAL`, `PRAGMA synchronous=NORMAL`, `PRAGMA foreign_keys=ON`
- [ ] `db::open` runs `sqlx::migrate!("../../backend/migrations")`
- [ ] `crates/anvilml-registry/src/lib.rs` exposes `pub mod db`
- [ ] `cargo test -p anvilml-registry -- db` exits 0 with the migration test passing
