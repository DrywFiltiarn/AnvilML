# Implementation Report: P2-A5

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P2-A5                                             |
| Phase         | 002 — Core Domain Types: Config & Errors          |
| Description   | anvilml-core: config_load env var + CLI flag layers |
| Implemented   | 2026-06-26T22:55:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Extended `config_load::load()` in `crates/anvilml-core/src/config_load.rs` to implement layers 3–4 of the four-layer config precedence chain: scan `ANVILML_*` environment variables with `__` nested-field convention, and accept an optional `CliOverrides` struct applied last as the highest-precedence layer. Added `pub struct CliOverrides { host, port }`, extended the `load()` signature, implemented `apply_env_vars()` and `apply_cli_overrides()` helpers, re-exported `CliOverrides` from `lib.rs`, added 7 new tests (13 total in file), and bumped `anvilml-core` patch version 0.1.4 → 0.1.5. Added `serial_test` dev-dependency for env var test isolation.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source       |
|--------|------------|------------------|--------------|
| crate  | serial_test| 3.5.0            | cargo search |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config_load.rs` | Add `CliOverrides` struct, extend `load()` signature, implement `apply_env_vars()` and `apply_cli_overrides()`, restructure TOML reading into `if path.exists()` block so env vars and CLI overrides always apply |
| Modify | `crates/anvilml-core/src/lib.rs` | Add `pub use config_load::CliOverrides;` re-export |
| Modify | `crates/anvilml-core/tests/config_load_tests.rs` | Add 7 new tests for env var and CLI override layers, update all existing tests for new `load()` signature, add `#[serial]` attribute and env var cleanup helpers |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5, add `serial_test = "3.5.0"` dev-dependency |
| Modify | `docs/TESTS.md` | Add 7 new test entries |

## Commit Log

```
 .forge/reports/P2-A5_plan.md                   | 202 ++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  11 +-
 Cargo.lock                                     |  28 ++-
 crates/anvilml-core/Cargo.toml                 |   3 +-
 crates/anvilml-core/src/config_load.rs         | 237 +++++++++++++-----
 crates/anvilml-core/src/lib.rs                 |   1 +
 crates/anvilml-core/tests/config_load_tests.rs | 317 +++++++++++++++++++++++--
 docs/TESTS.md                                  |  84 +++++++
 9 files changed, 802 insertions(+), 87 deletions(-)
```

## Test Results

```
running 13 tests
test test_cli_override_beats_env_var ... ok
test test_env_var_overrides_default_no_toml ... ok
test test_env_var_overrides_toml_value ... ok
test test_env_var_port_override ... ok
test test_load_default_path_resolves_anvilml_toml ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_load_nested_struct_partial_override ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok
test test_nested_env_var_gpu_selection ... ok
test test_num_threads_env_var ... ok
test test_unset_env_vars_leave_prior_layer_value ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all 57 tests passed (0 failed).

## Format Gate

```
Format pass 2 OK
```

## Platform Cross-Check

```
Check 1 (mock-hardware Linux):   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
Check 2 (mock-hardware Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.63s
Check 3 (real-hardware Linux):   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
Check 4 (real-hardware Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
```

All four checks exited 0.

## Project Gates

Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` — 0 tests, 0 failures.
Gate 2 (OpenAPI Drift): Not required — `api/openapi.json` does not exist.

## Public API Delta

```
+pub struct CliOverrides {
+    pub host: Option<String>,
+    pub port: Option<u16>,
+pub fn load(
+pub use config_load::CliOverrides;
```

New `pub` items:
- `struct CliOverrides` in `anvilml_core::CliOverrides` — CLI flag overrides struct
- `fn load` in `anvilml_core::config_load::load` — extended signature with `cli_overrides: Option<CliOverrides>` parameter
- `pub use config_load::CliOverrides` re-export in `anvilml_core`

## Deviations from Plan

1. **Restructured `load()` TOML reading**: The original plan had `load()` return early with defaults when the TOML file doesn't exist, which would skip env var and CLI override layers. I restructured the code so TOML reading is inside an `if path.exists()` block, ensuring env vars and CLI overrides are always applied regardless of whether a TOML file exists. This is a behavior improvement that makes the env var and CLI override tests work correctly.

2. **Added `serial_test` dev-dependency**: The plan stated "No new external dependencies are needed", but `#[serial]` annotation for env var test isolation requires the `serial_test` crate (version 3.5.0). This was added to `Cargo.toml` dev-dependencies.

3. **Added `clear_anvilml_env_vars()` and `restore_env_vars()` helper functions**: These were added to the test file to ensure proper env var isolation across tests, since all tests run in the same process with shared `std::env`.

4. **Rust 1.96 `unsafe` for `set_var`/`remove_var`**: Rust 1.96 made `std::env::set_var` and `std::env::remove_var` unsafe. All env var mutations in tests are wrapped in `unsafe` blocks.

## Blockers

None.
