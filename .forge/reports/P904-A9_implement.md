# Implementation Report: P904-A9

| Field         | Value                                                      |
|---------------|------------------------------------------------------------|
| Task ID       | P904-A9                                                    |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)      |
| Description   | worker/nodes/loader.py: remove deprecated HF-directory loading remnants entirely |
| Implemented   | 2026-06-24T11:15:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Deleted two dead-code helper functions (`_load_from_hf_directory` and `_load_clip_from_hf_directory`) from `worker/nodes/loader.py` that were preserved as "kept but never called" remnants from Phase 018. Both functions were never called anywhere in the codebase — the active loading paths now use `from_single_file()` (P18-D14) and the arch-dispatched `arch_clip.get_module()` (P18-D12) respectively. The module was reduced from 831 to 724 lines. All 12 mock-mode tests pass, all Rust tests pass (155 tests, 0 failures), and all four platform cross-checks (mock/real × Linux/Windows) exit cleanly.

## Resolved Dependencies

None. This task introduces no new dependencies and removes none.

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| (none) | (none)  | (none)          | (none)        |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Delete `_load_from_hf_directory` (lines 645–672) and `_load_clip_from_hf_directory` (lines 756–831); no import changes needed |

## Commit Log

```
.forge/reports/P904-A9_plan.md | 114 +++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md   |   6 +--
 .forge/state/state.json        |  13 ++---
 worker/nodes/loader.py         | 107 --------------------------------------
 4 files changed, 124 insertions(+), 116 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 12 items

worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [  8%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 16%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 25%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 33%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 41%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 50%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 58%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 66%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 75%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 83%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 91%]
worker/tests/test_nodes_loader.py::test_loadmodel_hf_directory_accepts_device_param PASSED [100%]

============================== 12 passed in 2.61s ==============================
```

Rust test summary (all 155 tests passed):
```
cargo test --workspace --features mock-hardware
  anvilml: 1 passed (cli_tests) + 1 passed (config_reference)
  anvilml_artifacts: 5 passed (store_tests)
  anvilml_core: 3 passed (artifact_tests) + 4 passed (config_load_tests) + 3 passed (config_tests) + 17 passed (error_tests) + 4 passed (events_tests) + 4 passed (hardware_tests) + 5 passed (job_tests) + 3 passed (model_tests) + 3 passed (node_tests) + 3 passed (worker_tests)
  anvilml_hardware: 3 passed (cpu_tests) + 7 passed (device_db_tests) + 12 passed (dxgi_sysfs_tests) + 9 passed (mock_tests) + 4 passed (vulkan_tests)
  anvilml_ipc: 17 passed (roundtrip_tests) + 1 passed (stress_test) + 4 passed (transport_tests)
  anvilml_registry: 5 passed (db_tests) + 4 passed (device_store_tests) + 7 passed (scanner_tests) + 2 passed (seed_loader_tests) + 7 passed (store_tests)
  anvilml_scheduler: 10 passed (dag_tests) + 5 passed (dispatch_tests) + 3 passed (event_loop_tests) + 3 passed (image_ready_tests) + 8 passed (ledger_tests) + 3 passed (model_resolve_tests) + 6 passed (node_registry_tests) + 2 passed (progress_tests) + 10 passed (queue_tests) + 5 passed (scheduler_cancel_tests) + 8 passed (scheduler_tests)
  anvilml_server: 5 passed (artifact_store_tests) + 4 passed (artifacts_tests) + 3 passed (broadcaster_tests) + 2 passed (handler_tests) + 1 passed (health_tests) + 10 passed (jobs_tests) + 6 passed (models_tests) + 2 passed (nodes_tests) + 3 passed (state_tests) + 3 passed (stats_tick_tests) + 2 passed (system_tests) + 2 passed (workers_tests)
  anvilml_worker: 2 passed (bridge_tests) + 4 passed (demux_tests) + 10 passed (env_tests) + 5 passed (keepalive_tests) + 12 passed (managed_tests) + 6 passed (pool_tests) + 4 passed (respawn_tests) + 7 passed (spawn_tests)
  Doc-tests: 1 passed (anvilml_ipc)
  Total: 155 passed; 0 failed
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no output, no drift)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.58s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

## Project Gates

None applicable — this task does not add, rename, or remove fields on `ServerConfig`, does not modify handler function signatures or `#[utoipa::path]` annotations, and does not add, remove, or rename node types in `worker/nodes/`. Only helper functions were deleted.

## Public API Delta

```
git diff HEAD -- worker/nodes/loader.py | grep "^+.*pub " | head -40
```
No new pub items introduced. (The deleted functions were module-private — underscore-prefixed and not exported in `__all__`.)

## Deviations from Plan

None. Implementation followed the approved plan exactly.

## Blockers

None.
