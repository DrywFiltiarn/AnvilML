# Implementation Report: P6-A8

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P6-A8                           |
| Phase         | 006 — Model Registry & Artifacts|
| Description   | database/seeds/devices.sql: one-time conversion from SUPPORTED_DEVICES_DB.md |
| Implemented   | 2026-06-29T22:45:00Z            |
| Status        | COMPLETE                          |

## Summary

Created `database/seeds/devices.sql` — a one-time SQL seed file containing 353 INSERT statements (292 NVIDIA + 61 AMD) that populate the `device_capabilities` table with PCI-ID-based capability hints for all supported GPUs. Each INSERT is preceded by a traceability comment naming the source vendor heading and device. The file was generated via a throwaway Python conversion script from `docs/SUPPORTED_DEVICES_DB.md`, validated by loading into an in-memory SQLite instance, and confirmed to return exactly 353 rows. Also fixed a pre-existing one-line bug in `crates/anvilml-registry/tests/db_tests.rs` (missing return expression due to trailing semicolon).

## Resolved Dependencies

None. This task creates a SQL seed file only — no Rust crates, Python packages, or external dependencies are introduced.

| Type   | Name    | Version resolved | Source        |
|--------|---------|------------------|---------------|
| (none) | —       | —                | —             |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/seeds/devices.sql` | One-time SQL seed: 353 INSERT statements for device_capabilities table (292 NVIDIA + 61 AMD), with traceability comments |
| CREATE | `database/seeds/` (directory) | Parent directory for seed files |
| MODIFY | `crates/anvilml-registry/tests/db_tests.rs` | Pre-existing fix: removed trailing semicolon on line 118 that caused return type mismatch (NamedTempFile → ()) |

## Commit Log

```
 crates/anvilml-registry/tests/db_tests.rs |    2 +-
 database/seeds/devices.sql                | 1417 +++++++++++++++++++++++++++++
 2 files changed, 1418 insertions(+), 1 deletion(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

Running unittests src/lib.rs (target/debug/deps/anvilml-a59c5f5d3bbaec87)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/cli_help_test.rs
running 1 test
test tests::cli_help_shows_all_flags ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/config_reference.rs
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/db_tests.rs (anvilml-registry)
running 4 tests
test test_migrations_create_tables ... ok
test test_pool_creation_succeeds ... ok
test test_wal_mode_enabled ... ok
test test_migrations_idempotent ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/seed_loader_tests.rs (anvilml-registry)
running 8 tests
test test_already_applied_unseen_seed_returns_false ... ok
test test_seed_log_created_on_first_use ... ok
test test_already_applied_hash_mismatch_returns_false ... ok
test test_already_applied_hash_match_returns_true ... ok
test test_run_malformed_sql_returns_err_no_partial_state ... ok
test test_run_first_time_applies_and_records ... ok
test test_run_skips_when_already_applied ... ok
test test_run_reapplies_on_changed_content ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

[... all other crate tests passed — 0 failures across the full workspace ...]

all doctests in 0.70s; merged doctests compilation took 0.69s
test result: ok. all passed; 0 failed; 0 ignored; 0 measured
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit code: 0 (no formatting drift)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.93s
Exit: 0

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.67s
Exit: 0

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.93s
Exit: 0

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.07s
Exit: 0
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Exit: 0
```

## Public API Delta

No new pub items introduced. This task creates a SQL seed file — no Rust `pub` items, no Python functions, no API surface changes.

## Deviations from Plan

- **Pre-existing bug fix**: `crates/anvilml-registry/tests/db_tests.rs` line 118 had a trailing semicolon that caused `create_temp_db()` to return `()` instead of `NamedTempFile`, producing a compile error (`E0308: mismatched types`). This was a pre-existing defect from a prior task in the same phase. Per `FORGE_AGENT_RULES.md §9.3`, the most minimal fix was applied (removed the semicolon) and is documented here. This fix is necessary for the test suite to compile and pass.

- **SQL load command**: The plan specified `sqlite3 :memory: < database/migrations/001_initial.sql database/seeds/devices.sql` for validation, but this syntax does not work with `sqlite3` (it treats the second file as a command, not input). The correct approach used was `cat database/migrations/001_initial.sql database/seeds/devices.sql | sqlite3 :memory:` which concatenates both files and pipes them into a single in-memory SQLite session.

- **Conversion method**: The plan stated "A throwaway script (Python or shell) may be used to automate the row-by-row conversion." A Python script was written, executed, and then discarded (deleted from `/tmp/`). The conversion was verified: 292 NVIDIA rows + 61 AMD rows = 353 total INSERT statements, matching the source Markdown exactly.

## Blockers

None.
