# Plan Report: P13-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-A1                                        |
| Phase       | 13 — Job Queue                                |
| Description | database/: jobs table migration               |
| Depends on  | P12-B1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-07-06T20:45:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `database/migrations/003_jobs.sql` — the SQLite schema for the `jobs` table that stores persisted `Job` rows, plus two indexes on `status` and `created_at`. This is the persistence foundation that P13-B1's `JobStore` CRUD code and P13-C1's ghost-job reset will operate against. The acceptance criterion is a clean `sqlite3 :memory:` execution of the file.

## Scope

### In Scope
- Create `database/migrations/003_jobs.sql` with:
  - `CREATE TABLE jobs (...)` with all columns specified in the task context.
  - `CREATE INDEX idx_jobs_status ON jobs(status)`.
  - `CREATE INDEX idx_jobs_created_at ON jobs(created_at)`.
- Follow the existing migration style: leading comment block describing the table, column comments mapping to `anvilml-core` types, `IF NOT EXISTS` guards, aligned column definitions.
- Verify the SQL runs cleanly against `sqlite3 :memory:`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope without deferring any functionality.

## Existing Codebase Assessment

**What already exists:** Two migration files (`001_initial.sql`, `002_artifacts.sql`) establish the style convention: a leading `-- Migration NNN:` comment block, `CREATE TABLE IF NOT EXISTS` with column-level comments mapping to the corresponding `anvilml-core` type, and `CREATE INDEX` statements for commonly-queried columns. The `anvilml-registry` crate's `db.rs` uses `sqlx::migrate!("../../database/migrations")` to auto-discover and apply all migration files in filename-sorted order. Integration tests in `crates/anvilml-registry/tests/db_tests.rs` exercise migration application via `create_pool()` against a temporary file.

**The Job struct** (`crates/anvilml-core/src/types/job.rs`) defines the canonical in-memory shape: `id: Uuid`, `status: JobStatus`, `graph: serde_json::Value`, `settings: JobSettings`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `worker_id: Option<String>`, `error: Option<String>`, `queue_position: Option<u32>`. The SQL table is a persistence mirror — `graph` and `settings` are stored as serialized JSON TEXT, not as normalized columns.

**Established patterns to follow:**
- Column comments reference the source type (e.g., `-- UUID string of the originating job`).
- Boolean columns use `INTEGER 0/1` (SQLite has no native BOOLEAN).
- All TEXT columns that hold enum values store the snake_case variant (e.g., `"queued"`, `"running"`).
- Timestamps use ISO 8601 UTC TEXT format.
- The `IF NOT EXISTS` guard is used on both `CREATE TABLE` and `CREATE INDEX`.

**Gap:** The existing `test_migrations_create_tables` test in `db_tests.rs` checks for `models` and `device_capabilities` tables only. It does not assert the presence of `jobs` (or `artifacts`). After this task ships, the test will silently pass regardless of whether `003_jobs.sql` exists — but that is acceptable for this phase: the test's purpose is to verify migration application doesn't fail, and it already covers that. The explicit table enumeration is a soft assertion, not a hard gate. P13-B1 or a later task may strengthen this if needed.

## Resolved Dependencies

None. This task produces a pure SQL migration file — no external crates, no Rust dependencies, no Python packages. The acceptance criterion uses the system `sqlite3` CLI tool (version 3.45.1 confirmed available).

## Approach

1. **Create `database/migrations/003_jobs.sql`** with the following content:

   a. A leading comment block:
      ```
      -- Migration 003: Jobs table
      --
      -- Creates the `jobs` table for persisted Job rows.
      -- Columns map from Job (anvilml-core/src/types/job.rs):
      --   id, status, graph, settings, created_at, started_at,
      --   completed_at, worker_id, error, queue_position
      -- graph and settings are TEXT (serialized JSON via serde_json),
      -- not normalized columns — the Job struct owns the canonical shape.
      ```

   b. The `jobs` table definition:
      ```sql
      CREATE TABLE IF NOT EXISTS jobs (
          id           TEXT PRIMARY KEY,  -- UUID string (stable unique identifier)
          status       TEXT NOT NULL,     -- JobStatus enum value as text ("queued", "running", "completed", "failed", "cancelled")
          graph        TEXT NOT NULL,     -- serialized JSON computation graph (serde_json::to_string)
          settings     TEXT NOT NULL,     -- serialized JSON JobSettings (serde_json::to_string)
          created_at   TEXT NOT NULL,     -- ISO 8601 UTC timestamp when queued
          started_at   TEXT,              -- ISO 8601 UTC timestamp when execution began (null while queued)
          completed_at TEXT,              -- ISO 8601 UTC timestamp when finished (null while running/queued)
          worker_id    TEXT,              -- worker identity string (set after execution begins)
          error        TEXT,              -- failure diagnostic message (set only for Failed jobs)
          queue_position INTEGER          -- position in the queue when Queued (cleared when picked up)
      );
      ```

      Rationale for column choices:
      - `id` is `TEXT PRIMARY KEY` (not `INTEGER AUTOINCREMENT`) because `Job.id` is a `Uuid`, serialised to a hex string.
      - `status` is `TEXT NOT NULL` matching the `#[serde(rename_all = "snake_case")]` on `JobStatus` — values are `"queued"`, `"running"`, `"completed"`, `"failed"`, `"cancelled"`.
      - `graph` and `settings` are `TEXT NOT NULL` because the task specifies JSON serialization, not normalised columns.
      - `started_at`, `completed_at`, `worker_id`, `error`, `queue_position` are nullable (`TEXT`/`INTEGER` without `NOT NULL`) matching the `Option<T>` fields on `Job`.
      - `queue_position` is `INTEGER` (not `TEXT`) because `Job.queue_position` is `Option<u32>`.

   c. Two indexes:
      ```sql
      CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
      CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);
      ```
      Rationale: `idx_jobs_status` supports the scheduler's `JobQueue` filtering by status (P13-B1 `JobStore::list()`). `idx_jobs_created_at` supports FIFO ordering and "list before timestamp" queries.

2. **Verify the SQL** runs cleanly:
   ```bash
   sqlite3 :memory: < database/migrations/003_jobs.sql
   # exits 0
   ```

## Public API Surface

None. This task produces a SQL file only — no Rust or Python symbols are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/003_jobs.sql` | Jobs table migration + two indexes |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| (verification) | `sqlite3 :memory:` execution | The SQL file is syntactically valid, semantically coherent, and creates the table + indexes without errors | `sqlite3 :memory: < database/migrations/003_jobs.sql` exits 0 |

## CI Impact

No CI changes required. The migration file is discovered automatically by `sqlx::migrate!()` at compile time — no Cargo.toml changes, no new test modules, no new build steps. The existing `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace` which exercises `anvilml-registry`'s `db_tests.rs`, and those tests will automatically pick up the new migration via the embedded migrator.

## Platform Considerations

None identified. The SQL is platform-neutral — SQLite syntax is consistent across Linux, Windows, and macOS. No `#[cfg(unix)]` / `#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `graph` and `settings` TEXT columns may need a `CHECK` constraint to enforce valid JSON, which SQLite 3.38+ supports via `json()` — omitting it means invalid JSON can be stored silently. | Low | Medium | The task context specifies `TEXT NOT NULL` without mentioning JSON validation. The `JobStore` CRUD layer (P13-B1) will handle validation at the application level; the migration stays simple. If later tasks require DB-level enforcement, a migration 004 can add a `CHECK` constraint. |
| The `status` column has no `CHECK` constraint or foreign key, so a typo in the status string produces a silent data integrity issue rather than a constraint violation. | Low | Low | SQLite has no foreign-key enforcement on TEXT enums by convention. The `JobStatus` enum in Rust provides type safety at the application layer. This matches the pattern in `001_initial.sql` where `kind` and `dtype` are also unconstrained TEXT. |
| The `sqlite3 :memory:` acceptance check may differ from the actual SQLite version used by the Rust `sqlx` runtime, potentially missing version-specific syntax issues. | Low | Low | The project uses sqlx with SQLite; sqlx bundles its own SQLite driver. The migration uses only basic DDL (CREATE TABLE, CREATE INDEX) which is universally supported across all SQLite versions. No advanced features (CTEs, window functions, CHECK constraints) are used. |

## Acceptance Criteria

- [ ] `sqlite3 :memory: < database/migrations/003_jobs.sql` exits 0
- [ ] `sqlite3 :memory: database/migrations/003_jobs.sql -cmd "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;"` outputs a table named `jobs`
- [ ] `sqlite3 :memory: database/migrations/003_jobs.sql -cmd "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_jobs_%' ORDER BY name;"` outputs two indexes: `idx_jobs_created_at` and `idx_jobs_status`
- [ ] `head -1 database/migrations/003_jobs.sql` starts with `-- Migration 003:`
