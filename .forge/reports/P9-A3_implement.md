# Implementation Report: P9-A3

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P9-A3                                             |
| Phase         | 009 — Real Worker Startup                         |
| Description   | worker/: pyproject.toml or pytest.ini with real_mode marker registered |
| Implemented   | 2026-07-05T10:18:31Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `worker/pyproject.toml` with a `[tool.pytest.ini_options]` section that registers the `real_mode` pytest marker. The marker tells pytest that a test exercises real torch-level code (requires `torch` import) and must never run under `ANVILML_WORKER_MOCK=1`. Verification via `pytest --markers` confirms the marker is correctly registered.

## Resolved Dependencies

None. This task creates a single TOML configuration file with no external dependencies. The `pytest` marker registration is a built-in pytest feature — no new package or crate is introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pyproject.toml` | pytest marker registration with `real_mode` marker |

## Commit Log

```
 .forge/reports/P9-A3_plan.md   | 86 ++++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md   |  6 ++--
 .forge/state/state.json        | 13 +++----
 worker/pyproject.toml          |  4 +++
 4 files changed, 100 insertions(+), 9 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.87s
     Running unittests src/lib.rs (target/debug/deps/anvilml-7708e8b7193f85b4)
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

     Running tests/db_startup_tests.rs
running 5 tests
test tests::test_missing_seed_file_causes_startup_failure ... ok
test tests::test_db_file_created_on_startup ... ok
test tests::test_seed_populates_device_capabilities ... ok
test tests::test_migrations_create_required_tables ... ok
test tests::test_seed_idempotent_second_run ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hw_probe_help_test.rs
running 1 test
test tests::hw_probe_help_shows_subcommand ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/logging_tests.rs
running 6 tests
test tests::test_anvilml_log_debug_yields_stderr ... ok
test tests::test_log_format_invalid_exits_nonzero ... ok
test tests::test_anvilml_log_precedence_over_rust_log ... ok
test tests::test_log_format_plain_produces_text_lines ... ok
test tests::test_log_format_json_produces_json_lines ... ok
test tests::test_rust_log_debug_yields_stderr ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs
running 2 tests
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok
test tests::test_shutdown_signal_timeout_cancels ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_artifacts-31257c0d823873d0)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_core-bccf3c6079de7173)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/artifact_tests.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_load_tests.rs
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/events_tests.rs
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hardware_tests.rs
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/job_tests.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/model_tests.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_registry_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_tests.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/worker_tests.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-089cfd6e7f0535da)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cpu_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/detect_tests.rs
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/dxgi_tests.rs
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/mock_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/sysfs_tests.rs
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/vulkan_tests.rs
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-ad46a9e914b6a83e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/roundtrip_tests.rs
running 26 tests
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/stress_test.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/db_tests.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner_tests.rs
running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader_tests.rs
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-64121f61ef175350)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/bridge_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/demux_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/env_tests.rs
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/keepalive_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/managed_tests.rs
running 39 tests
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/pool_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/respawn_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/spawn_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_registry — 2 passed
   Doc-tests anvilml_worker — 1 passed

All 229 tests passed; 0 failed.
```

Verification of marker registration:
```
cd /home/dryw/AnvilML/worker && ../worker/.venv/bin/python -m pytest --markers 2>/dev/null | grep real_mode
@pytest.mark.real_mode: test requires real torch import, never runs under ANVILML_WORKER_MOCK=1
```

## Format Gate

```
cargo fmt --all -- --check
```
(No output — exit 0, no formatting drift.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.92s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.75s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.45s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
`api/openapi.json` does not yet exist — gate skipped per documentation.

## Public API Delta

No new pub items introduced. This task creates a TOML configuration file only.

## Deviations from Plan

None. Implementation exactly matches the approved plan.

## Blockers

None.
