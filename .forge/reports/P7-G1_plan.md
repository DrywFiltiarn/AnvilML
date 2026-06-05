# Plan Report: P7-G1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-G1                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | Create backend/seeds/devices.sql seed file  |
| Depends on  | P7-F4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-05T15:32:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `backend/seeds/devices.sql`, a SQL seed file containing one `INSERT OR REPLACE` statement per row from both device tables in `docs/SUPPORTED_DEVICES_DB.md`. The file seeds the `device_capabilities` SQLite table with all 126 known GPU capability entries, using the `replace_all` strategy.

## Scope

### In Scope
- Create `backend/seeds/devices.sql` with the two directive header lines:
  - `-- anvil:seed_table device_capabilities`
  - `-- anvil:seed_strategy replace_all`
- Generate one `INSERT OR REPLACE INTO device_capabilities(...)` per row from both the NVIDIA and AMD device tables in `docs/SUPPORTED_DEVICES_DB.md` (77 NVIDIA + 49 AMD = 126 total)
- Columns match `backend/migrations/004_device_capabilities.sql` DDL order: `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`
- `Y` in the source table maps to `1`, `N` maps to `0`
- `vendor_id` and `device_id` written as hex literals (e.g. `0x10DE`, `0x2684`)
- `model_name` and `arch` as single-quoted string literals
- Rows copied verbatim from the source document — no generation, reordering, or supplementation

### Out of Scope
- Any Rust code changes (no `seed_loader.rs`, no `device_store.rs`, no `main.rs` modifications)
- Running any build, test, or lint commands
- Git operations (commit, push, branch)
- Modifying the migration file `004_device_capabilities.sql`
- Creating additional seed files

## Approach

1. **Read `docs/SUPPORTED_DEVICES_DB.md`** — locate both device data tables:
   - NVIDIA Devices table (lines 69–145): 77 rows, vendor_id = 0x10DE
   - AMD Devices table (lines 183–231): 49 rows, vendor_id = 0x1002
   - Confirm the table name from the Migration DDL reference section (line 260): `device_capabilities`

2. **Write file header** — first two lines of the file:
   ```sql
   -- anvil:seed_table device_capabilities
   -- anvil:seed_strategy replace_all
   ```

3. **Generate INSERT statements** — for each of the 126 rows, emit:
   ```sql
   INSERT OR REPLACE INTO device_capabilities (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn) VALUES (0x10DE, 0x2684, 'NVIDIA GeForce RTX 4090', '8.9', 1, 1, 1, 1, 0, 0, 1);
   ```

4. **Verify by inspection** — confirm:
   - File begins with both `-- anvil:` directive comments
   - Exactly 126 INSERT statements (77 NVIDIA + 49 AMD)
   - Column order matches DDL: `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`
   - Boolean values are `0` or `1` (not `Y`/`N`)
   - Hex vendor/device IDs use `0x` prefix
   - String literals are single-quoted

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/seeds/devices.sql` | Seed file with 126 INSERT OR REPLACE statements for device_capabilities table |

## Tests

None. This task creates a static SQL data file only — no Rust code, no test runner involved. The acceptance criterion is syntactic validity by inspection, not compilation or execution.

## CI Impact

No CI changes required. A `.sql` seed file does not affect any CI gate commands (format, clippy, tests, cross-checks). The file is data-only and has no effect on the build pipeline.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Copying rows with transcription errors from SUPPORTED_DEVICES_DB.md | Read each row directly from the source document; verify count matches expected (77 NVIDIA + 49 AMD = 126) |
| Using wrong table name (`devices` vs `device_capabilities`) | Task explicitly instructs to verify against migration DDL; migration creates `device_capabilities` — use that name |
| Column order mismatch causing silent data misalignment | Follow the canonical order from 004_device_capabilities.sql: vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn |
| Missing trailing semicolons on INSERT statements | Each statement ends with `;` — verify during inspection |

## Acceptance Criteria

- [ ] `backend/seeds/devices.sql` exists and is non-empty
- [ ] File begins with `-- anvil:seed_table device_capabilities` followed by `-- anvil:seed_strategy replace_all`
- [ ] Contains exactly 126 INSERT OR REPLACE statements (77 NVIDIA + 49 AMD)
- [ ] Column order matches DDL: vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn
- [ ] Boolean values are `0` or `1` (Y→1, N→0)
- [ ] vendor_id and device_id use hex literals with `0x` prefix
- [ ] model_name and arch are single-quoted string literals
- [ ] Rows are verbatim from SUPPORTED_DEVICES_DB.md — no generated, inferred, or re-ordered entries
