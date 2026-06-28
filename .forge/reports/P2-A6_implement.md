# Implementation Report: P2-A6

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P2-A6                           |
| Phase         | 002 — Core Domain Types: Config & Errors |
| Description   | backend: wire config_load::load() into main.rs |
| Implemented   | 2026-06-28T14:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Wired the complete four-layer config loading pipeline (`config_load::load()`) into
`backend/src/main.rs`, replacing the Phase 1 CLI-only host/port binding with `ServerConfig`
as the actual source of truth for the server's bind address and port. Changed `cli.rs`
`host` and `port` fields from `String`/`u16` with clap defaults to `Option<String>`/`Option<u16>`
without defaults. Added `anvilml-core` as a direct dependency of `backend`. All 47 workspace
tests pass, all 4 platform cross-checks pass, clippy reports zero warnings, and formatting
is clean.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source   |
|--------|-------------|------------------|----------|
| crate  | anvilml-core| 0.1.5 (local)    | Workspace|

No external crates were added. `anvilml-core` is an existing workspace member (`crates/anvilml-core`),
added here as a direct dependency of `backend` (previously only transitively available through
`anvilml-server` and `anvilml-scheduler`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `anvilml-core` dependency; bump version 0.1.1 → 0.1.2 |
| Modify | `backend/src/cli.rs` | Change `host`/`port` fields from `String`/`u16` with clap defaults to `Option<String>`/`Option<u16>` without defaults |
| Modify | `backend/src/main.rs` | Wire `config_load::load()`, use `ServerConfig` for TCP bind address |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 +++---
 .forge/state/state.json      | 11 ++++++-----
 Cargo.lock                   |  3 ++-
 backend/Cargo.toml           |  3 ++-
 backend/src/cli.rs           | 20 ++++++++++++++------
 backend/src/main.rs          | 38 ++++++++++++++++++++++++++++++++++----
 6 files changed, 61 insertions(+), 20 deletions(-)
```

## Test Results

```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli_help_test.rs
running 1 test
test tests::cli_help_shows_all_flags ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs
running 2 tests
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok
test tests::test_shutdown_signal_timeout_cancels ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_load_tests.rs (anvilml-core)
running 13 tests
test test_cli_override_beats_env_var ... ok
test test_env_var_overrides_toml_value ... ok
test test_env_var_port_override ... ok
test test_load_default_path_resolves_anvilml_toml ... ok
test test_env_var_overrides_default_no_toml ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_nested_env_var_gpu_selection ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok
test test_unset_env_vars_leave_prior_layer_value ... ok
test test_load_nested_struct_partial_override ... ok
test test_num_threads_env_var ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs (anvilml-core)
running 13 tests
test test_artifact_dir_default ... ok
test test_db_path_default ... ok
test test_hardware_override_default ... ok
test test_gpu_selection_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_limits_default ... ok
test test_host_default ... ok
test test_model_dirs_default ... ok
test test_model_scan_depth_default ... ok
test test_port_default ... ok
test test_num_threads_default ... ok
test test_rocm_default ... ok
test test_venv_path_default ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (anvilml-core)
running 16 tests
test test_artifact_not_found_returns_404 ... ok
test test_db_returns_500 ... ok
test test_error_body_has_request_id ... ok
test test_cycle_detected_returns_400 ... ok
test test_internal_returns_500 ... ok
test test_error_body_message_contains_variant_info ... ok
test test_invalid_graph_returns_400 ... ok
test test_io_returns_500 ... ok
test test_ipc_returns_400 ... ok
test test_job_not_found_returns_404 ... ok
test test_error_field_is_snake_case ... ok
test test_model_not_found_returns_404 ... ok
test test_payload_too_large_returns_413 ... ok
test test_worker_not_found_returns_404 ... ok
test test_workers_unavailable_returns_503 ... ok
test test_serde_returns_400 ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (anvilml-server)
running 1 test
test test_health_returns_200 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Total: 47 tests passed; 0 failed
```

## Format Gate

```
(no output — `cargo fmt --all -- --check` exited 0, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s

# 2. Mock-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.84s

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.67s

# 4. Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.59s
```

All four platform cross-checks exited 0.

## Project Gates

Gate 1 (Config Surface Sync) — Not triggered. This task does not add, rename, or remove
any field on `ServerConfig` or nested config structs. The config surface is unchanged.

Gate 2 (OpenAPI Drift) — Not triggered. This task does not modify handler function
signatures, `#[utoipa::path]` annotations, or `AppState` fields.

Gate 3 (Node Parity) — Not triggered. This task does not modify `worker/nodes/` or
`crates/anvilml-core/src/node_registry.rs`.

Gate 4 (Mock/Real Parity Markers) — Not triggered. This task does not add or modify a
node's `execute()`, or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
+    pub host: Option<String>,
+    pub port: Option<u16>,
```

The only `pub` items changed are `Cli::host` and `Cli::port` in `backend/src/cli.rs`,
changing their types from `String`/`u16` to `Option<String>`/`Option<u16>`. The `Cli`
struct is not re-exported outside `backend` (it is private to the binary), so no
downstream crate is affected. No new `pub` items were introduced.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- `backend/Cargo.toml`: added `anvilml-core` dependency, bumped version 0.1.1 → 0.1.2
- `backend/src/cli.rs`: changed `host`/`port` to `Option` with updated doc comments
- `backend/src/main.rs`: wired `config_load::load()` with `.map_err()` + `exit(1)` error handling,
  uses `config.host`/`config.port` for `TcpListener::bind()`

## Blockers

None.
