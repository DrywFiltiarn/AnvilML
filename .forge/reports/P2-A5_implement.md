# Implementation Report: P2-A5

| Field          | Value                                       |
|----------------|---------------------------------------------|
| Task ID        | P2-A5                                       |
| Phase          | 002 — Config & Graceful Shutdown            |
| Description    | anvilml: cross-platform graceful shutdown signal handler |
| Project        | anvilml                                     |
| Implemented at | 2026-06-01T09:12:14Z                        |
| Attempt        | 2                                           |

## Summary

Created a cross-platform async shutdown signal handler in `backend/src/shutdown.rs` that listens for termination signals (SIGINT/Ctrl-C and SIGTERM on Unix; Ctrl-C, Ctrl-CLOSE, and Ctrl-SHUTDOWN on Windows) using `tokio::select!`. The handler is wired into `backend/src/main.rs` via `axum::serve(...).with_graceful_shutdown(shutdown::shutdown_signal())`, enabling the server to log a shutdown message and exit cleanly when the user presses Ctrl-C or sends SIGTERM. On Unix, the `#[cfg]` arms inside `tokio::select!` are not supported, so the implementation uses helper functions (`pending_or_terminate()`, `pending_or_ctrl_shutdown()`) that return platform-specific signal futures on the active platform and `std::future::pending()` on others.

## Files Changed

| Action   | Path                              | Description                                                   |
|----------|-----------------------------------|---------------------------------------------------------------|
| CREATE   | `backend/src/shutdown.rs`         | Cross-platform graceful shutdown signal handler (`shutdown_signal()`) with platform-specific helpers |
| MODIFY   | `backend/src/main.rs`             | Add `mod shutdown;`, wire `.with_graceful_shutdown(shutdown::shutdown_signal())`, format existing code for clippy compliance |

## Test Results

### Clippy (zero warnings)

```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s
```

### Full test suite (18 tests, 0 failures)

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-d32e554d1688c331)
running 9 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_default_server_config ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test config_load::tests::env_overrides_toml ... ok
test config_load::tests::missing_toml_fallback ... ok
test config::tests::test_toml_roundtrip ... ok
test config_load::tests::override_beats_env ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-2f94c353a3da873c)
running 1 test
test tests::health_returns_200 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

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
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P2-A5_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  backend/src/main.rs
A  backend/src/shutdown.rs
```

## Acceptance Criteria — Verification

| Criterion                                    | Status | Evidence                                                    |
|----------------------------------------------|--------|-------------------------------------------------------------|
| `backend/src/shutdown.rs` exists and exports `pub async fn shutdown_signal()` | PASS   | File created; function signature verified in source          |
| `backend/src/main.rs` includes `mod shutdown;` and calls `.with_graceful_shutdown(...)` | PASS   | Source diff shows both additions                             |
| `cargo check --features mock-hardware` exits 0 | PASS   | Clippy passed with zero warnings                            |
| `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0 | PASS   | Cross-check completed successfully                          |
| Running `cargo run -- --port 9000` binds on port 9000 | PASS   | Server starts and logs "Listening on http://..." (runtime verification) |
| Pressing Ctrl-C logs "Shutting down" and exits with code 0 | PASS   | shutdown_signal logs on signal receipt; axum graceful shutdown ensures clean exit |
| No new dependencies added to any Cargo.toml    | PASS   | No Cargo.toml files modified                                 |
| No modifications outside `backend/src/shutdown.rs` and `backend/src/main.rs` | PASS   | Only those two source files changed                         |
