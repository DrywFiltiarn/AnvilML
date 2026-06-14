# Implementation Report: P900-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P900-A2                            |
| Phase         | 900 — Retrofit                     |
| Description   | anvilml-core: add #[serial] to env-var-mutating config_load tests |
| Implemented   | 2026-06-14T18:57:14Z               |
| Status        | COMPLETE                           |

## Summary

Added `serial_test` dev-dependency to `anvilml-core` and annotated three env-var-mutating tests
(`test_env_var_beats_toml`, `test_cli_override_beats_env`, `test_nested_env_var`) with the
`#[serial]` proc-macro attribute to serialise their execution and eliminate the race window
on process-global `std::env`. The `test_missing_file_uses_defaults` test was intentionally
left unannotated as it only calls `remove_var` without `set_var`. The `anvilml-core` crate
version was bumped from 0.1.5 to 0.1.6. All 28 workspace tests pass, format gates clean,
and all four platform cross-checks pass.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| crate  | serial_test | 3.5.0            | crates.io (plan: 3.1, resolved by cargo to latest compatible ^3.1) |
| crate  | serial_test_derive | 3.5.0   | crates.io (transitive) |

Note: The rust-docs MCP server was unavailable (dependency import error). The plan specified
`serial_test = "3.1"` which is a semver-compatible constraint `^3.1`. Cargo resolved to
`3.5.0` (latest compatible). This satisfies the version floor rule — 3.5.0 >= 3.1.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-core/Cargo.toml | Added `serial_test = "3.1"` to `[dev-dependencies]`; bumped version 0.1.5 → 0.1.6 |
| Modify | crates/anvilml-core/tests/config_load_tests.rs | Added `use serial_test::serial;` import; added `#[serial]` to 3 tests |
| Modify | docs/TESTS.md | Added `#[serial]` justification note to 3 test entries |
| Modify | Cargo.lock | Updated by cargo — added serial_test 3.5.0 and serial_test_derive 3.5.0 |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                   |  6 +++---
 .forge/state/state.json                        | 13 ++++++------
 Cargo.lock                                     | 28 +++++++++++++++++++++++++-
 crates/anvilml-core/Cargo.toml                 |  3 ++-
 crates/anvilml-core/tests/config_load_tests.rs |  4 ++++
 docs/TESTS.md                                  |  3 +++
 6 files changed, 46 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-261cc91878857941)

running 4 tests
test test_missing_file_uses_defaults ... ok
test test_cli_override_beats_env ... ok
test test_env_var_beats_toml ... ok
test test_nested_env_var ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace: 28 tests passed, 0 failed across all crates.

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows:
Checking anvilml-core v0.1.6
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s

# 3. Real-hardware Linux:
Checking anvilml-hardware v0.1.0
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s

# 4. Real-hardware Windows:
Checking anvilml-hardware v0.1.0
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s
```

All four cross-checks exited 0.

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node types.
No `config_reference` test exists in this codebase; no Gate 1/2/3 triggers apply.

## Public API Delta

```
git diff HEAD -- crates/anvilml-core/tests/config_load_tests.rs crates/anvilml-core/Cargo.toml | grep '^+.*pub ' | head -40
```
(no output)

No new pub items introduced. The only changes are test-level annotations and a dev-dependency.

## Deviations from Plan

None. All plan steps were implemented exactly as specified:
- `serial_test = "3.1"` added to `[dev-dependencies]` (cargo resolved to 3.5.0 at build time)
- `use serial_test::serial;` import added
- `#[serial]` added to `test_env_var_beats_toml`, `test_cli_override_beats_env`, `test_nested_env_var`
- `test_missing_file_uses_defaults` left unannotated as directed
- Version bumped 0.1.5 → 0.1.6
- `docs/TESTS.md` entries updated with serial annotation notes

## Blockers

None.
