# Implementation Report: P2-A4

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P2-A4                                       |
| Phase          | 002 — Config & Graceful Shutdown            |
| Description    | anvilml: tracing subscriber init (plain/json, ANVILML_LOG env filter) |
| Project        | anvilml                                     |
| Implemented at | 2026-06-01T11:00:00Z                        |
| Attempt        | 1                                           |

## Summary

Added structured logging to the AnvilML backend by introducing `tracing` (v0.1) and `tracing-subscriber` (v0.3 with `env-filter` and `json` features). The subscriber is initialised at the top of `main.rs` before any server logic, sourcing the log-level filter from the `ANVILML_LOG` environment variable (falling back to `RUST_LOG`, then `info`). The `--log-format plain|json` CLI flag routes to plain-text or JSON output via a `Box<dyn Layer>` abstraction that unifies the otherwise incompatible `FmtSubscriber` types. The existing `println!` startup message was replaced with `tracing::info!`.

## Files Changed

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | backend/Cargo.toml                | Added `tracing = "0.1"` and `tracing-subscriber` with `env-filter`, `json` features |
| MODIFY   | backend/src/main.rs               | Added subscriber init with env-filter resolution, plain/json routing via `Box<dyn Layer>`, replaced `println!` with `tracing::info!` |
| MODIFY   | Cargo.lock                        | Updated lockfile with new tracing dependency tree    |

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-d32e554d1688c331)

running 9 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_default_server_config ... ok
test config_load::tests::env_nested_field ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_overrides_toml ... ok
test config_load::tests::missing_toml_fallback ... ok
test config_load::tests::override_beats_env ... ok
test config::tests::test_toml_roundtrip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-f8ca89b4fde12841)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-36ad9a87003ef179)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-23860774db875e7e)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-2ec6b5c9f58ca99a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-fa6a3c2248a3fdd0)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-2f94c353a3da873c)

running 1 test
test tests::health_returns_200 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-d53adc0af7639869)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml-3d839d169d634581)

running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_hardware

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_scheduler

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_worker

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P2-A4_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  backend/Cargo.toml
M  backend/src/main.rs
```

## Acceptance Criteria — Verification

| Criterion                                      | Status | Evidence                                              |
|------------------------------------------------|--------|-------------------------------------------------------|
| Add `tracing` v0.1 to `[dependencies]`         | PASS   | `grep tracing backend/Cargo.toml` shows dependency    |
| Add `tracing-subscriber` v0.3 with features    | PASS   | Features `env-filter`, `json` present in Cargo.toml   |
| Subscriber init before config/server logic     | PASS   | Init code placed after `cli::parse()`, before `load_config()` |
| Log filter: ANVILML_LOG → RUST_LOG → info      | PASS   | Code uses `.var("ANVILML_LOG").or_else(var "RUST_LOG").unwrap_or("info")` |
| plain format → fmt()                           | PASS   | Plain branch calls `fmt().finish().with(filter)`      |
| json format → fmt().json()                     | PASS   | Json branch calls `fmt().json().finish().with(filter)`|
| Replace println! with tracing::info!           | PASS   | Line 53 uses `tracing::info!("Listening on ...")`     |
| cargo fmt --all passes                          | PASS   | Ran successfully, no output                           |
| cargo clippy --workspace -D warnings            | PASS   | Zero warnings, finished successfully                  |
| cargo check x86_64-pc-windows-gnu              | PASS   | Cross-check completed with zero errors                |
| cargo test --workspace all tests pass           | PASS   | 18 tests passed, 0 failed                             |
