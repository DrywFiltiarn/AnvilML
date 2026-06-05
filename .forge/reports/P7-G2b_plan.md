# Plan Report: P7-G2b

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-G2b                                        |
| Phase       | 007 — WebSocket Event Stream                |
| Description | seed_loader — execution engine for replace_all and merge strategies |
| Depends on  | P7-G2a                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-06-05T17:35:00Z                         |
| Attempt     | 1                                             |

## Objective

Implement the actual SQL execution engine in `crates/anvilml-registry/src/seed_loader.rs`, replacing the stub `execute_seed` function (P7-G2a) with real transactional logic for both `replace_all` and `merge` seed strategies. Both strategies execute within a single SQLite transaction, update `seed_history` on success, and roll back entirely on any error.

## Scope

### In Scope
- Implement `execute_seed(pool, table, body_bytes, strategy)` with real SQL execution:
  - **replace_all**: `BEGIN; DELETE FROM <table>; <each INSERT from body>; UPDATE seed_history ...; COMMIT`
  - **merge**: `BEGIN; <each INSERT OR REPLACE from body>; UPDATE seed_history ...; COMMIT`
- Parse file body by stripping `-- anvil:` header lines and splitting remaining SQL on `;` into individual statements
- Execute each non-empty statement via `sqlx::query` within a single transaction
- Roll back transaction on any statement error (no `seed_history` update)
- Integration tests in `tests/seed_loader.rs` (≥5 total including P7-G2a tests):
  - `sha256_skip_does_not_execute` — unchanged hash skips execution entirely
  - `replace_all_replaces_table_content` — pre-existing rows are deleted, new ones inserted
  - `merge_preserves_unreferenced_rows` — pre-existing rows not in seed file remain
  - `changed_sha256_reruns_seed` — modified content triggers re-execution
  - `missing_seed_table_directive_returns_error` — missing directive returns `SeedMissingDirective` error

### Out of Scope
- Adding new dependencies (sha2, hex already present from P7-G2a)
- Modifying the seed_history upsert logic (already implemented in P7-G2a)
- Header parsing changes (already implemented in P7-G2a)
- CLI integration or config changes (P7-G3 territory)
- Modifying `devices.sql` seed file content

## Approach

1. **Parse body from raw bytes.** After header directives are parsed and SHA256 computed, the remaining content after the last `-- anvil:` line is the SQL body. Strip leading/trailing whitespace.

2. **Split on semicolons.** Split body by `;` into individual statement strings. Trim each. Skip empty strings.

3. **Execute in single transaction.** For each strategy:
   - `replace_all`: execute `DELETE FROM <table>` first, then each non-empty statement from the body (these are INSERT statements), finally the `seed_history` update (which is done after `execute_seed` returns).
   - `merge`: execute each non-empty statement from the body directly (INSERT OR REPLACE).
   - The `seed_history` UPDATE remains in the caller (`run`) to keep transaction scope clear: BEGIN → seed statements → COMMIT.

4. **Transaction management.** Use `sqlx::query` for each statement. On any error, propagate it up — sqlx will handle rollback on transaction drop. No explicit ROLLBACK needed; Drop impl of `Transaction` handles it.

5. **Add tests** to `tests/seed_loader.rs` using `open_in_memory()` and temporary seed files via `tempfile::TempDir`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/seed_loader.rs` | Replace stub `execute_seed` with real transactional execution for both strategies |
| Modify | `crates/anvilml-registry/tests/seed_loader.rs` | Add ≥4 new integration tests (sha256_skip_does_not_execute, replace_all_replaces_table_content, merge_preserves_unreferenced_rows, changed_sha256_reruns_seed) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/seed_loader.rs` | `sha256_skip_does_not_execute` | When file hash matches stored value, execute_seed is never called (no new rows appear in target table beyond what existed before the call) |
| `tests/seed_loader.rs` | `replace_all_replaces_table_content` | Pre-existing rows in the target table are deleted by DELETE FROM; only rows from the seed file remain after execution |
| `tests/seed_loader.rs` | `merge_preserves_unreferenced_rows` | Rows inserted before merge execution that are not in the seed file remain untouched after the merge completes |
| `tests/seed_loader.rs` | `changed_sha256_reruns_seed` | After modifying a seed file's content (changing its hash), the next run re-executes and the target table reflects the new content |
| `tests/seed_loader.rs` | `missing_seed_table_directive_returns_error` | A SQL file missing the `-- anvil:seed_table` directive causes `run()` to return `Err(AnvilError::SeedMissingDirective)` without touching the database |

## CI Impact

No CI workflow changes required. This task only modifies source code and tests within `anvilml-registry`. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace -- -D warnings`, platform cross-checks) will cover this change automatically.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Transaction rollback behavior in sqlx — if a statement fails mid-transaction, the entire transaction must roll back and no seed_history row should be written | Use `sqlx::query` inside the transaction; on error the transaction Drop impl rolls back automatically. The `seed_history` upsert is in the caller after `execute_seed` returns Ok, so it only runs on success. |
| Statement splitting on `;` could produce empty or whitespace-only statements that cause SQL errors | Trim each split result and skip any string that is empty or contains only whitespace |
| Comment lines within SQL body (e.g., `-- NVIDIA Devices`) could be misinterpreted as header directives if parsing logic is shared | Body extraction happens after header parsing — the body is everything after the last `-- anvil:` line, so internal comments are just part of the SQL text and handled by SQLite's own parser |
| The seed file contains `INSERT OR REPLACE` statements but `replace_all` strategy should use plain `INSERT` (after DELETE) | The body contains `INSERT OR REPLACE` — for `replace_all`, these work correctly after `DELETE FROM` (the `OR REPLACE` is harmless). For `merge`, they are the intended behavior. No transformation needed. |
| Test isolation — multiple tests share an in-memory database | Each test creates its own `open_in_memory()` pool, so databases are fully isolated. No shared state between tests. |

## Acceptance Criteria

- [ ] `execute_seed` implements both `replace_all` and `merge` strategies with single-transaction execution
- [ ] `cargo test -p anvilml-registry -- seed` exits 0 with ≥5 tests passing
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] Platform cross-check: `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` exits 0
