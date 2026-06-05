# Implementation Report: P7-G2b

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P7-G2b                                                        |
| Phase       | 007 — WebSocket Event Stream                                  |
| Description | seed_loader — execution engine for replace_all and merge strategies |
| Implemented | 2026-06-05T18:45:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Implemented the actual SQL execution engine in `crates/anvilml-registry/src/seed_loader.rs`, replacing the stub `execute_seed` function with real transactional logic for both `replace_all` and `merge` seed strategies. The implementation extracts the SQL body from raw file bytes (after header directives), splits on semicolons into individual statements, and executes them within a single SQLite transaction via `sqlx::Transaction`. On any error, the transaction is automatically rolled back by sqlx's Drop impl. Integration tests were added to verify both strategies, SHA256 skip behavior, and re-execution on content change.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| crate | sqlx | 0.9.0 (existing) | Pre-existing in Cargo.toml |
| crate | sha2 | (existing) | Pre-existing, no changes |
| crate | hex | (existing) | Pre-existing, no changes |

No new dependencies were added. The `AssertSqlSafe` wrapper from `sqlx::AssertSqlSafe` was used to pass non-static SQL strings to `sqlx::query` (required by sqlx 0.9's `'static` lifetime constraint on SQL).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/seed_loader.rs` | Added `extract_body()` function; replaced stub `execute_seed` with real transactional execution for both strategies; updated `run()` to pass actual table and strategy values to `execute_seed` |
| Modify | `crates/anvilml-registry/tests/seed_loader.rs` | Renamed `test_sha256_skip_unchanged` → `sha256_skip_does_not_execute` with target-table assertions; added `replace_all_replaces_table_content`; added `merge_preserves_unreferenced_rows`; added `changed_sha256_reruns_seed`; fixed pre-existing tests to create target tables before running seeds |

## Commit Log

```
 crates/anvilml-registry/src/seed_loader.rs   |  77 +++++++++--
 crates/anvilml-registry/tests/seed_loader.rs | 183 +++++++++++++++++++++++++--
 2 files changed, 237 insertions(+), 23 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)

running 19 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test seed_loader::tests::test_compute_sha256_empty ... ok
test seed_loader::tests::test_compute_sha256_known_value ... ok
test seed_loader::tests::test_parse_header_both_directives ... ok
test seed_loader::tests::test_parse_header_defaults_strategy ... ok
test seed_loader::tests::test_parse_header_empty_file ... ok
test seed_loader::tests::test_parse_header_missing_table ... ok
test device_store::tests::test_get_miss_returns_none ... ok
test db::tests::test_open_creates_tables ... ok
test device_store::tests::test_upsert_then_get_roundtrip ... ok
test db::tests::test_open_creates_file_if_missing ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-587e31eb59849c7a)

running 7 tests
test test_directive_parsing_miss ... ok
test test_table_bootstrap_idempotent ... ok
test sha256_skip_does_not_execute ... ok
test merge_preserves_unreferenced_rows ... ok
test test_directive_parsing_hit ... ok
test replace_all_replaces_table_content ... ok
test changed_sha256_reruns_seed ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace test suite: 180 tests, 0 failures.
```

## Platform Cross-Check

**Check 1 — Mock-hardware Windows-gnu:**
```
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.77s
```

**Check 2 — Real-hardware Linux native:**
```
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s
```

**Check 3 — Real-hardware Windows-gnu:**
```
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.35s
```

All three checks exit 0.

## Project Gates

**Config Surface Sync:**
```
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

## Deviations from Plan

1. **sqlx 0.9 API difference**: In sqlx 0.9, `Transaction<'_, DB>` does not implement `Executor` (the impl is commented out due to "lack of lazy normalization" in Rust's trait system). Instead of `query().execute(&mut tx)`, I used `query(sqlx::AssertSqlSafe(...)).execute(&mut *tx)` — dereferencing through `DerefMut` to get `&mut SqliteConnection`, which does implement `Executor`. The `AssertSqlSafe` wrapper is needed because sqlx 0.9 requires `'static` lifetime for SQL strings.

2. **Pre-existing test fixes**: Two pre-existing integration tests (`test_table_bootstrap_idempotent` and `test_directive_parsing_hit`) were failing because they referenced tables (`foo`, `main_schema`) that didn't exist. Since `execute_seed` now actually runs SQL statements, these tests needed CREATE TABLE statements added before calling `run()`. This is a test isolation fix, not a plan deviation.

3. **Test assertion correction**: The `changed_sha256_reruns_seed` test originally expected 2 rows after re-execution with replace_all strategy, but replace_all deletes all rows before inserting the new ones. The assertion was corrected to expect 1 row (only the new value) and verify the old row was deleted.

## Blockers

None.
