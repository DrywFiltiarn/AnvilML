# Plan Report: P6-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A1                                       |
| Phase       | 006 — Model Registry & Artifacts            |
| Description | database/: migrations dir + 001_initial.sql (models, device_capabilities) |
| Depends on  | P3-A11                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T13:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `database/migrations/` directory and the first SQL migration file
`database/migrations/001_initial.sql`, establishing the two tables the `anvilml-registry`
crate needs before any Rust code can run queries against them: the `models` table for
persisted `ModelMeta` rows and the `device_capabilities` PCI-ID hint table. The
acceptance criterion is that `sqlite3 :memory: < database/migrations/001_initial.sql`
exits 0.

## Scope

### In Scope
- Create `database/migrations/001_initial.sql` containing:
  - `CREATE TABLE models (...)` — columns: `id`, `name`, `path`, `kind`, `dtype`,
    `format`, `size_bytes`, `mtime_unix`, `scanned_at`, matching the schema in
    `ANVILML_DESIGN.md §7` and the task context.
  - `CREATE TABLE device_capabilities (...)` — columns: `vendor_id`, `device_id`,
    `name`, `arch`, `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`, with
    `PRIMARY KEY (vendor_id, device_id)`, matching the DDL reference in
    `SUPPORTED_DEVICES_DB.md §Migration DDL reference`.
  - `CREATE UNIQUE INDEX idx_device_capabilities_pci ON device_capabilities(vendor_id, device_id)`
    as specified in the DDL reference.
- All boolean columns use `INTEGER NOT NULL DEFAULT 0` (SQLite has no native BOOLEAN type).
- Acceptance: `sqlite3 :memory: < database/migrations/001_initial.sql` exits 0.

### Out of Scope
None. This task's `defers_to` field is `[]` — no scope may be deferred.
No `jobs` or `artifacts` tables (those are P6-A2 and P6-B2 respectively).
No seed data, no Rust code, no `lib.rs` changes, no test files.

## Existing Codebase Assessment

No prior source exists in the `database/` directory — it does not exist yet on disk.
The `crates/anvilml-registry/` crate exists as an empty stub (its `lib.rs` contains
only a crate-level doc comment, and its `Cargo.toml` has no dependencies beyond
`anvilml-core`). The `crates/anvilml-registry/tests/` directory does not exist yet.

The `anvilml-core` crate already defines the domain types this migration targets:
`ModelMeta` (in `crates/anvilml-core/src/types/model.rs`) with fields `id`, `name`,
`path`, `kind`, `dtype`, `format`, `size_bytes`, `scanned_at`, and
`InferenceCaps` (in `crates/anvilml-core/src/types/hardware.rs`) with fields
`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`. The `device_capabilities`
table schema in `SUPPORTED_DEVICES_DB.md §Migration DDL reference` maps these fields
directly — booleans become `INTEGER 0/1`.

No external crate versions need resolution for this task since it produces only a SQL
file with no Rust code changes.

## Resolved Dependencies

None. This task writes a single SQL migration file. No new Rust dependencies, no
Cargo.toml changes, no Python packages. The `anvilml-registry` crate's existing
dependency on `anvilml-core` is unchanged.

## Approach

1. **Create the directory structure.**
   - Run `mkdir -p database/migrations` from the repository root.
   - This creates both `database/` and `database/migrations/`.

2. **Write `database/migrations/001_initial.sql`.**
   - Write the file using a heredoc (as required by FORGE_AGENT_RULES §8 for technical
     content that must not be corrupted).
   - The file contains three statements in order:
     a. `CREATE TABLE models (...)` — the models table with all columns from the task
        context, using `IF NOT EXISTS` for idempotent migration safety.
     b. `CREATE TABLE device_capabilities (...)` — the PCI-ID capability hint table
        with `IF NOT EXISTS`, matching the DDL reference in
        `SUPPORTED_DEVICES_DB.md §Migration DDL reference` exactly.
     c. `CREATE UNIQUE INDEX IF NOT EXISTS idx_device_capabilities_pci ...` — the
        unique index on the composite PCI-ID key, as specified in the DDL reference.
   - The `models` table columns map from `ModelMeta` as follows:
     - `id TEXT PRIMARY KEY` — SHA256 hex string
     - `name TEXT NOT NULL` — human-readable model name
     - `path TEXT NOT NULL UNIQUE` — filesystem path (unique to prevent duplicate
       registrations of the same file)
     - `kind TEXT NOT NULL` — `ModelKind` enum value as text (e.g. `"diffusion"`)
     - `dtype TEXT NOT NULL` — `ModelDtype` enum value as text (e.g. `"fp8"`)
     - `format TEXT NOT NULL` — `ModelFormat` enum value as text (e.g. `"safetensors"`)
     - `size_bytes INTEGER NOT NULL` — file size in bytes
     - `mtime_unix INTEGER NOT NULL` — last modification time as Unix timestamp
       (populated by the scanner in P6-A4; not a field on `ModelMeta` itself)
     - `scanned_at TEXT NOT NULL` — ISO 8601 UTC timestamp when the model was scanned
   - The `device_capabilities` table columns follow the DDL reference verbatim:
     `vendor_id INTEGER NOT NULL`, `device_id INTEGER NOT NULL`, `name TEXT NOT NULL`,
     `arch TEXT NOT NULL`, and six boolean columns as `INTEGER NOT NULL DEFAULT 0`.
   - Primary key on `device_capabilities` is the composite `(vendor_id, device_id)`.

3. **Verify acceptance.**
   - Run `sqlite3 :memory: < database/migrations/001_initial.sql` and confirm exit 0.
   - Run `.tables` in an interactive sqlite3 session to confirm both tables are created:
     `echo '.tables' | sqlite3 :memory: < database/migrations/001_initial.sql`
     should print `device_capabilities  models`.

## Public API Surface

None. This task creates a SQL migration file only — no Rust types, functions, traits,
or re-exports are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/001_initial.sql` | First migration: `models` and `device_capabilities` tables plus unique index |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (none — SQL-only) | migration_accepts_sqlite | The SQL file is valid SQLite and creates both tables in an in-memory database | None | `database/migrations/001_initial.sql` | Exit 0 from `sqlite3 :memory:` | `sqlite3 :memory: < database/migrations/001_initial.sql` exits 0 |

## CI Impact

No CI changes required. A SQL migration file does not affect any CI job's behaviour.
The file is committed and will be picked up by the migration runner in the next task
(P6-A2), but that task's CI impact is handled separately.

## Platform Considerations

None identified. The migration file is pure SQL — no `#[cfg(unix)]` / `#[cfg(windows)]`
guards, no path-separator handling, no line-ending differences. The Windows cross-check
in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `models` table schema may diverge from what `ModelMeta` in `anvilml-core` expects — e.g. the design doc's `ModelMeta` has `scanned_at: DateTime<Utc>` but the migration also adds `mtime_unix` which is not a field on `ModelMeta` (it's an extra persisted column used by the scanner). A mismatch would cause P6-A3's ModelStore to fail on INSERT. | Low | Medium | The task context explicitly lists `mtime_unix` as a column in the `models` table. Cross-check the column list against the task context string before writing: `id`, `name`, `path`, `kind`, `dtype`, `format`, `size_bytes`, `mtime_unix`, `scanned_at`. |
| SQLite `INTEGER PRIMARY KEY` on `device_capabilities` creates an implicit `ROWID` alias for the first column (`vendor_id`), which could silently change the intended composite primary key semantics. | Low | Low | The DDL reference in `SUPPORTED_DEVICES_DB.md` uses `PRIMARY KEY (vendor_id, device_id)` — a composite key, not a single-column alias. This is the correct SQLite syntax for a composite PK. Verify the syntax by running the migration in `sqlite3 :memory:`. |
| The `path` column uses `UNIQUE` constraint, but `PathBuf` values from `ModelMeta` may contain platform-specific separators (e.g. `C:\models\` on Windows). The unique constraint would treat these as different strings from `C:/models/`. | Low | Low | The `path` field stores the absolute filesystem path as-is from the scanner. The scanner runs on the same platform where paths use native separators, so this is not a practical concern for the MVP. Document the assumption in a comment if needed. |

## Acceptance Criteria

- [ ] `sqlite3 :memory: < database/migrations/001_initial.sql` exits 0
- [ ] `echo '.tables' | sqlite3 :memory: < database/migrations/001_initial.sql` outputs both `device_capabilities` and `models`
