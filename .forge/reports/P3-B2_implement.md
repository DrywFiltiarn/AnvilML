# Implementation Report: P3-B2

| Field         | Value                                          |
|---------------|------------------------------------------------|
| Task ID       | P3-B2                                          |
| Phase         | 003 — Core Domain Types                        |
| Description   | anvilml.toml drift guard test (committed toml key-set == ServerConfig) |
| Project       | anvilml                                        |
| Implemented at| 2026-06-01T15:30:00Z                           |
| Attempt       | 1                                              |

## Summary

Created a drift guard integration test that ensures the committed `anvilml.toml` file stays structurally in sync with the `ServerConfig` Rust struct from `anvilml-core`. The implementation fixed the `[frontend]` section in `anvilml.toml` from an inline table format (`mode = { path = "./bloomery" }`) to the nested table format that serde produces when serializing `FrontendMode::Local { path: PathBuf }` (`[frontend.mode.Local]`). The TOML sections were also restructured so that top-level scalar keys (`num_threads`, `num_interop_threads`, `worker_log_dir`) appear before the `[[model_dirs]]` array declaration, preventing TOML parsing from absorbing them into the last array element. A recursive key-set comparison function was written to collect all keys from both the committed TOML and `ServerConfig::default()` serialization, comparing them while treating arrays as opaque leaf values.

## Files Changed

| Action   | Path                             | Description                                              |
|----------|----------------------------------|----------------------------------------------------------|
| MODIFY   | anvilml.toml                     | Fixed `[frontend]` section to nested table format; restructured sections so scalar keys precede `[[model_dirs]]` array |
| MODIFY   | backend/Cargo.toml               | Added `toml = "0.8"` as a dev-dependency                 |
| CREATE   | backend/tests/config_reference.rs| Integration test with recursive TOML key-set comparison  |

## Test Results

**Full workspace test suite (cargo test --workspace --features mock-hardware):**
```
running 68 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test error::tests::from_io_error ... ok
test config_load::tests::env_overrides_toml ... ok
test error::tests::send_sync ... ok
test config_load::tests::missing_toml_fallback ... ok
test config::tests::test_toml_roundtrip ... ok
... (56 more anvilml-core tests all passed)

test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests (anvilml-server)
test tests::health_returns_200 ... ok
test tests::env_returns_200_with_stub_report ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 8 tests (backend binary)
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test (config_reference integration test)
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Config drift gate (cargo test -p backend --features mock-hardware --test config_reference):**
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Windows cross-check (cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware):**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s
```

**Clippy lint (cargo clippy --workspace --features mock-hardware -- -D warnings):**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
```

## CI Changes

No CI changes made.
The test runs as part of the existing `cargo test -p backend` gate in CI.

## Commit Log

```
A  .forge/reports/P3-B2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  anvilml.toml
M  backend/Cargo.toml
A  backend/tests/config_reference.rs
```

## Acceptance Criteria — Verification

| Criterion                                          | Status | Evidence                                       |
|----------------------------------------------------|--------|------------------------------------------------|
| Fix `[frontend]` section format in `anvilml.toml`  | PASS   | Changed inline table to `[frontend.mode.Local]` nested table matching serde output |
| Add `toml = "0.8"` dev-dependency                  | PASS   | Added `[dev-dependencies]` section to `backend/Cargo.toml` |
| Create `backend/tests/config_reference.rs`         | PASS   | Integration test with recursive key-set comparison |
| Recursive key-set comparison logic                 | PASS   | `collect_keys()` function handles nested tables, treats arrays as opaque |
| Ignore `[[model_dirs]]` array contents             | PASS   | Arrays are not recursed into during key collection |
| Ignore commented `[hardware_override]` section     | PASS   | Commented sections are not parsed by TOML library |
| `cargo fmt --all` passes                           | PASS   | Formatted in-place, no formatting violations    |
| `cargo clippy --workspace --features mock-hardware -D warnings` passes | PASS | Zero warnings                                     |
| Windows cross-check (`x86_64-pc-windows-gnu`)      | PASS   | Clean check output                                |
| Full workspace test suite passes                   | PASS   | 79 tests passed, 0 failures                      |
| Config drift gate test passes                      | PASS   | `test_toml_key_set_matches_default ... ok`       |
