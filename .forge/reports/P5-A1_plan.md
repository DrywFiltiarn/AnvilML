# Plan Report: P5-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P5-A1                                             |
| Phase       | 005 — SQLite Persistence                          |
| Description | anvilml-registry: SQLite migration files (jobs, models, artifacts) |
| Depends on  | P4-A6                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-03T18:25:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create three SQL migration files (`backend/migrations/001_jobs.sql`, `002_models.sql`, `003_artifacts.sql`) that define the `jobs`, `models`, and `artifacts` SQLite tables exactly per ANVILML_DESIGN.md §13, including all column names, types, constraints, and indexes verbatim. These files are consumed by P5-A2 (migration runner).

## Scope

### In Scope
- Create `backend/migrations/001_jobs.sql` with the `jobs` table DDL: columns (`id`, `status`, `graph`, `settings`, `device_index`, `created_at`, `started_at`, `completed_at`, `worker_id`, `artifact_count`, `error`), constraints (PRIMARY KEY, NOT NULL, DEFAULT 0), and indexes (`idx_jobs_status`, `idx_jobs_created_at`).
- Create `backend/migrations/002_models.sql` with the `models` table DDL: columns (`id`, `name`, `path`, `kind`, `size_bytes`, `dtype_hint`, `vram_estimate_mib`, `scanned_at`), constraints (PRIMARY KEY, NOT NULL, UNIQUE on path), and index (`idx_models_kind`).
- Create `backend/migrations/003_artifacts.sql` with the `artifacts` table DDL: columns (`hash`, `job_id`, `width`, `height`, `format`, `seed`, `steps`, `prompt`, `created_at`), constraints (PRIMARY KEY, NOT NULL, DEFAULT 'png'), and index (`idx_artifacts_job_id`).
- Verify SQL validity by eye against ANVILML_DESIGN.md §13 (lines 904–947).

### Out of Scope
- Any Rust code to open the database or run migrations (P5-A2).
- Ghost-job reset logic (P5-A3).
- Integration with `main.rs` startup (P5-A4).
- Tests for migration files themselves (covered by P5-A2's `db::open` test).
- Any changes to Cargo.toml, workspace config, or other source files.

## Approach

1. **Read ANVILML_DESIGN.md §13** (lines 904–947) to obtain the exact DDL statements for all three tables and their indexes.
2. **Create `backend/migrations/001_jobs.sql`**:
   - Write `CREATE TABLE IF NOT EXISTS jobs (...)` with all 11 columns matching the design verbatim.
   - Append two `CREATE INDEX IF NOT EXISTS` statements: `idx_jobs_status ON jobs(status)` and `idx_jobs_created_at ON jobs(created_at)`.
3. **Create `backend/migrations/002_models.sql`**:
   - Write `CREATE TABLE IF NOT EXISTS models (...)` with all 8 columns matching the design verbatim, including `UNIQUE` on `path`.
   - Append `CREATE INDEX IF NOT EXISTS idx_models_kind ON models(kind)`.
4. **Create `backend/migrations/003_artifacts.sql`**:
   - Write `CREATE TABLE IF NOT EXISTS artifacts (...)` with all 9 columns matching the design verbatim, including `DEFAULT 'png'` on `format`.
   - Append `CREATE INDEX IF NOT EXISTS idx_artifacts_job_id ON artifacts(job_id)`.
5. **Verify by eye** each file against the source DDL in ANVILML_DESIGN.md §13, confirming:
   - Column names match exactly (case-sensitive).
   - Column types match (`TEXT`, `INTEGER`, `NOT NULL`, `DEFAULT` values).
   - All three indexes are present and target the correct columns.
   - No extra whitespace or formatting differences that would affect sqlx migration ordering.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/migrations/001_jobs.sql` | Jobs table DDL + 2 indexes |
| Create | `backend/migrations/002_models.sql` | Models table DDL + 1 index |
| Create | `backend/migrations/003_artifacts.sql` | Artifacts table DDL + 1 index |

## Tests

None. This task produces only migration SQL files; testing is covered by P5-A2's `db::open` integration test which will execute these migrations against a tempfile-based SQLite database and assert the tables exist.

## CI Impact

No CI changes required. These are plain SQL files under `backend/migrations/`, not touched by any existing CI job (fmt, clippy, cargo test, openapi-diff). The P5-A2 task will introduce sqlx dependency additions to `anvilml-registry/Cargo.toml` which may affect the CI matrix, but that is out of scope for this task.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Column names or types transcribed incorrectly from ANVILML_DESIGN.md §13 | Compare each line against the source DDL verbatim (lines 904–947) during verification step; no ambiguity in the spec. |
| Index column references do not match created columns | Each index targets a column that exists in its table definition; cross-check during verification. |
| sqlx migration ordering broken by file naming | Files follow the `NNN_` prefix convention (`001_`, `002_`, `003_`) which matches ANVILML_DESIGN.md §8 and the task description exactly. |

## Acceptance Criteria

- [ ] `backend/migrations/001_jobs.sql` exists with correct `jobs` table (11 columns, PRIMARY KEY on `id`, NOT NULL constraints, DEFAULT 0 on `artifact_count`, error column without NOT NULL) and two indexes (`idx_jobs_status`, `idx_jobs_created_at`)
- [ ] `backend/migrations/002_models.sql` exists with correct `models` table (8 columns, PRIMARY KEY on `id`, UNIQUE on `path`, all NOT NULL) and one index (`idx_models_kind`)
- [ ] `backend/migrations/003_artifacts.sql` exists with correct `artifacts` table (9 columns, PRIMARY KEY on `hash`, DEFAULT 'png' on `format`, all NOT NULL) and one index (`idx_artifacts_job_id`)
- [ ] All column names, types, and constraints match ANVILML_DESIGN.md §13 verbatim (cross-checked against lines 904–947)
