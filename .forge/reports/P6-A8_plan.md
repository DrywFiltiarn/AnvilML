# Plan Report: P6-A8

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A8                                             |
| Phase       | 006 — Model Registry & Artifacts                  |
| Description | database/seeds/devices.sql: one-time conversion from SUPPORTED_DEVICES_DB.md |
| Depends on  | P6-A7                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-29T22:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the one-time SQL seed file `database/seeds/devices.sql` by hand-converting every
data row from `docs/SUPPORTED_DEVICES_DB.md` into a corresponding `INSERT INTO
device_capabilities VALUES (...)` statement. Each INSERT is preceded by a comment naming
the source vendor heading (NVIDIA or AMD) and the specific row for traceability. The
resulting file, when loaded alongside `database/migrations/001_initial.sql` into an
in-memory SQLite instance, must execute without errors and populate the
`device_capabilities` table with exactly 353 rows (292 NVIDIA + 61 AMD). This closes
the one-time conversion described in `ANVILML_DESIGN.md §7.5` — no automation, no
follow-on task, no drift gate.

## Scope

### In Scope
- Read every vendor table in `docs/SUPPORTED_DEVICES_DB.md` (NVIDIA and AMD sections).
- Create the `database/seeds/` directory if it does not exist.
- Write `database/seeds/devices.sql`: one `INSERT INTO device_capabilities VALUES (...)`
  per data row (353 total), each preceded by a comment naming the source vendor heading
  and row for traceability.
- SQL TEXT values (device name, arch) must be properly escaped for SQLite (single quotes
  doubled to `''`).
- Confirm the Markdown row count matches the INSERT count exactly (353 = 353).
- Acceptance: `sqlite3 :memory: < database/migrations/001_initial.sql
  database/seeds/devices.sql` exits 0.

### Out of Scope
defers_to (from JSON): `[]` — empty. No deferrals permitted.

None. This task implements its full scope in full. The report confirms the row count
match and the SQLite load succeeds.

## Existing Codebase Assessment

The project already has `database/migrations/001_initial.sql` (created by P6-A1) which
defines the `device_capabilities` table schema — columns `vendor_id`, `device_id`,
`name`, `arch`, `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention` with boolean
fields stored as `INTEGER 0/1` and a composite primary key on `(vendor_id, device_id)`
plus a unique index. The seed loader infrastructure (`SeedLoader` in
`crates/anvilml-registry/src/seed_loader.rs`) is already implemented by P6-A6/P6-A7
and expects exactly this file path.

The `database/seeds/` directory does not yet exist — it must be created as part of this
task. No prior SQL seed files exist in the project.

The `docs/SUPPORTED_DEVICES_DB.md` file contains two vendor tables (NVIDIA and AMD) with
a total of 353 data rows (292 NVIDIA + 61 AMD). The table format uses pipe-delimited
Markdown with columns: `vendor_id`, `device_id`, `name`, `arch`, `fp32`, `fp16`, `bf16`,
`fp8`, `fp4`, `flash_attention`. Boolean values are stored as `Y`/`N` in the Markdown
and must be converted to `1`/`0` in the SQL INSERT statements.

No external crates, libraries, or dependencies are needed — this is a pure SQL seed file
with no code to compile or test beyond the SQLite load command.

## Resolved Dependencies

None. This task creates a SQL seed file only — no Rust crates, Python packages, or
external dependencies are introduced or referenced.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | —       | —               | —              | —                      |

## Approach

1. **Read the source file.** Open `docs/SUPPORTED_DEVICES_DB.md` and identify the two
   vendor tables: the NVIDIA table (heading `## NVIDIA Devices`, data rows 77–368) and
   the AMD table (heading `## AMD Devices`, data rows 406–466). Count the data rows in
   each table to establish the expected total (292 + 61 = 353).

2. **Create the output directory.** Create `database/seeds/` directory. This is a new
   directory — no parent directory changes needed since `database/` already exists.

3. **Write the SQL seed file.** Write `database/seeds/devices.sql` with the following
   structure:

   ```sql
   -- Seed: device_capabilities
   -- Source: docs/SUPPORTED_DEVICES_DB.md
   -- Generated: one-time conversion (P6-A8)
   -- NVIDIA rows: 292, AMD rows: 61, Total: 353

   -- [NVIDIA] TITAN X (Pascal) — 0x10DE:0x1B00
   INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention)
   VALUES (0x10DE, 0x1B00, 'NVIDIA TITAN X (Pascal)', '6.1', 0, 0, 0, 0, 0, 0);

   -- [NVIDIA] TITAN Xp — 0x10DE:0x1B02
   INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention)
   VALUES (0x10DE, 0x1B02, 'NVIDIA TITAN Xp', '6.1', 0, 0, 0, 0, 0, 0);

   -- ... (continues for all 292 NVIDIA rows)

   -- [AMD] Radeon Pro W5700X — 0x1002:0x7310
   INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention)
   VALUES (0x1002, 0x7310, 'AMD Radeon Pro W5700X', 'gfx1010', 0, 0, 0, 0, 0, 0);

   -- ... (continues for all 61 AMD rows)
   ```

   For each row:
   - The comment format is: `-- [Vendor] <Name> — <vendor_hex>:<device_hex>`
   - `vendor_id` and `device_id` are written as hex literals (`0x10DE`, `0x2684`, etc.)
   - `name` and `arch` are wrapped in single quotes, with any internal single quotes
     escaped as `''` (SQLite standard)
   - Boolean columns (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) are
     converted from `Y`/`N` in the Markdown to `1`/`0` in SQL
   - A throwaway script (Python or shell) may be used to automate the row-by-row
     conversion, but the script itself is not a deliverable — it is not committed,
     maintained, or tested. The ACT agent may write and execute it, then discard it,
     or perform the conversion entirely by hand.

4. **Verify row count.** Count the number of `INSERT INTO device_capabilities` statements
   in the generated file. Confirm it equals 353 (the sum of 292 NVIDIA + 61 AMD rows
   from the Markdown). Record this count in the report's acceptance section.

5. **Validate SQL load.** Run the acceptance command:
   ```bash
   sqlite3 :memory: < database/migrations/001_initial.sql database/seeds/devices.sql
   ```
   Confirm exit code 0. If non-zero, diagnose the error (likely a SQL syntax issue from
   unescaped quotes in a device name) and fix before proceeding.

6. **Verify row count in database (optional but recommended).** After the load succeeds,
   optionally run:
   ```bash
   echo "SELECT count(*) FROM device_capabilities;" | sqlite3 :memory: < database/migrations/001_initial.sql database/seeds/devices.sql
   ```
   Confirm it returns `353`.

## Public API Surface

None. This task creates a SQL seed file — no Rust `pub` items, no Python functions, no
API surface changes.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/seeds/devices.sql` | One-time SQL seed: 353 INSERT statements for device_capabilities table, with traceability comments |
| CREATE | `database/seeds/` (directory) | Parent directory for seed files |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (CLI) | `sqlite3_load` | Both SQL files load into an in-memory SQLite instance without errors | `database/migrations/001_initial.sql` exists (P6-A1), `database/seeds/devices.sql` exists (this task) | N/A (stdin from file redirection) | Exit code 0 | `sqlite3 :memory: < database/migrations/001_initial.sql database/seeds/devices.sql` exits 0 |
| (CLI) | `row_count_matches` | The number of INSERT statements in devices.sql equals the number of data rows in SUPPORTED_DEVICES_DB.md | Source Markdown file exists | N/A | Report states 353 = 353 | `grep -c '^INSERT INTO' database/seeds/devices.sql` outputs `353` |
| (CLI) | `row_count_in_db` | The loaded database contains exactly 353 rows in device_capabilities | Both SQL files load successfully | N/A | Query returns `353` | `echo "SELECT count(*) FROM device_capabilities;" \| sqlite3 :memory: < database/migrations/001_initial.sql database/seeds/devices.sql` outputs `353` |

## CI Impact

No CI changes required. The SQL seed file is loaded by the SeedLoader at server startup
and is consumed by the Runnable Proof in Phase 6 acceptance criteria. No new CI jobs,
gates, or test modules are introduced. The existing `config-drift` and `rust-linux` CI
jobs do not parse or validate seed SQL files.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The SQL
seed file is platform-neutral — SQLite handles the INSERT statements identically on
Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Device names in SUPPORTED_DEVICES_DB.md contain single quotes (e.g., apostrophes in product names like "NVIDIA GeForce RTX 4070 Ti SUPER" — unlikely but possible in edge cases) that would break SQL syntax if not escaped. | Low | High | Validate all device names for single quotes during conversion. Escape as `''` per SQLite convention. The `grep` acceptance command on the generated file can also be used to verify no unbalanced quotes exist. |
| Row count mismatch: the Markdown table has a different number of data rows than the INSERT statements produced, causing the acceptance criterion to fail silently (SQL loads but wrong data). | Low | Medium | Count Markdown rows before conversion and INSERT statements after. Both must equal 353. The `row_count_in_db` test provides a second verification point. |
| The `database/seeds/` directory path is not created before writing `devices.sql`, causing a file-creation error. | Low | Low | Create the directory explicitly as the first step. This is a trivial operation with no failure mode. |
| Hex literal syntax `0x10DE` is not supported by all SQLite versions (it was added in SQLite 3.9.0, released 2015). | Very Low | Medium | Verify SQLite version supports hex literals. If not, convert all hex values to decimal (`0x10DE` → `4318`) before writing INSERT statements. The `sqlite3` command available in the project environment (Ubuntu/WSL2) is modern enough to support hex literals. |

## Acceptance Criteria

- [ ] `grep -c '^INSERT INTO device_capabilities' /home/dryw/AnvilML/database/seeds/devices.sql` outputs `353` exits 0
- [ ] `sqlite3 :memory: < /home/dryw/AnvilML/database/migrations/001_initial.sql /home/dryw/AnvilML/database/seeds/devices.sql` exits 0
- [ ] `echo "SELECT count(*) FROM device_capabilities;" | sqlite3 :memory: < /home/dryw/AnvilML/database/migrations/001_initial.sql /home/dryw/AnvilML/database/seeds/devices.sql` outputs `353` exits 0
