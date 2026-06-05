# Plan Report: P7-F1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-F1                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-registry: migration 004_device_capabilities.sql |
| Depends on  | P7-F0                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-05T10:30:00Z                         |
| Attempt     | 1                                             |

## Objective

Create the `backend/migrations/004_device_capabilities.sql` migration file containing the DDL for the `device_capabilities` SQLite table as defined in `docs/SUPPORTED_DEVICES_DB.md`, and extend the existing `test_open_creates_tables` integration test to assert that the new table appears in `sqlite_master` after database initialization.

## Scope

### In Scope
- Create `backend/migrations/004_device_capabilities.sql` with the exact DDL from `docs/SUPPORTED_DEVICES_DB.md` § "Migration DDL reference"
- Extend `crates/anvilml-registry/tests/anvilml_registry_db.rs::test_open_creates_tables` to assert that `device_capabilities` exists in `sqlite_master` (count becomes 4 tables, plus individual existence check)

### Out of Scope
- Any Rust code changes (P7-F2 creates the store layer)
- Any migration data seeding (P7-F3 provides SEED_ENTRIES)
- Any changes to existing migration files (001–003)
- Any changes to CI, config, or documentation files
- Changes to `db.rs` unit tests (those are in a separate module; the integration test is the one specified by the task)

## Approach

1. **Create migration file** — Write `backend/migrations/004_device_capabilities.sql` using the exact DDL from `docs/SUPPORTED_DEVICES_DB.md` line 259–276. The file must contain:
   - `CREATE TABLE IF NOT EXISTS device_capabilities` with columns in canonical order: `vendor_id`, `device_id`, `model_name`, `arch`, `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `nvfp4`, `flash_attn`
   - All capability columns as `INTEGER NOT NULL DEFAULT 0`; `model_name` and `arch` as `TEXT NOT NULL`
   - Composite primary key on `(vendor_id, device_id)`
   - Unique index `idx_device_capabilities_pci` on `(vendor_id, device_id)`

2. **Extend integration test** — In `crates/anvilml-registry/tests/anvilml_registry_db.rs::test_open_creates_tables`:
   - Update the count assertion from 3 to 4 tables (jobs, models, artifacts, device_capabilities)
   - Add `"device_capabilities"` to the list of expected table names in the `IN (...)` clause
   - Add `"device_capabilities"` to the individual existence check loop

3. **Verify** — Run `cargo test -p anvilml-registry` and confirm exit code 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/migrations/004_device_capabilities.sql` | Migration DDL for device_capabilities table |
| Edit   | `crates/anvilml-registry/tests/anvilml_registry_db.rs` | Extend test_open_creates_tables to assert device_capabilities in sqlite_master |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/tests/anvilml_registry_db.rs` | `test_open_creates_tables` | After `db::open()`, all 4 tables (jobs, models, artifacts, device_capabilities) exist in sqlite_master |

## CI Impact

No CI changes required. This task only adds a migration file and extends an existing test. The existing CI matrix (`cargo test --workspace --features mock-hardware`) will naturally include `cargo test -p anvilml-registry` as part of the workspace test run. No workflow files are modified.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| DDL column order mismatch with canonical order (`fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`) causes silent data misalignment in later tasks (P7-F2) that use positional `query_as` mapping | Copy the DDL verbatim from `docs/SUPPORTED_DEVICES_DB.md` § "Migration DDL reference" — no manual reordering |
| Migration filename sort order conflict with existing files | `004_` sorts correctly after `003_artifacts.sql`; confirmed by listing existing migrations |
| Test assertion change could be fragile if table names in the query are hardcoded | Follow the exact same pattern used for the existing three tables — extend the `IN (...)` list and the iteration loop with `"device_capabilities"` |

## Acceptance Criteria

- [ ] `backend/migrations/004_device_capabilities.sql` exists and contains the correct DDL (CREATE TABLE + UNIQUE INDEX)
- [ ] `crates/anvilml-registry/tests/anvilml_registry_db.rs::test_open_creates_tables` asserts 4 tables including `device_capabilities`
- [ ] `cargo test -p anvilml-registry` exits 0

