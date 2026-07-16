# Implementation Report: P23-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P23-A1                          |
| Phase         | 23 — ZiT VAE Arch Module        |
| Description   | worker/tests/fixtures/: ZiT VAE fixture safetensors builder |
| Implemented   | 2026-07-16T18:45:00+0200        |
| Status        | COMPLETE                          |

## Summary

Created `worker/tests/fixtures/build_zit_vae_fixture.py`, a builder script that generates two tiny synthetic `.safetensors` checkpoint files for the ZiT VAE architecture: (1) `zit_vae_tiny.safetensors` with structurally valid ZiT-VAE-shaped tensor keys and `arch: "zit_vae"` metadata in the header, and (2) `zit_vae_tiny_no_metadata.safetensors` with non-recognizable `xyz_` key prefixes and no `arch` metadata, exercising the mandatory metadata-fallback regression path. Both files load successfully via `safetensors.safe_open` and have a combined size of 170,936 bytes — well under the 10 MB budget.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | safetensors | 0.8.0            | pypi-query MCP |
| python | torch       | 2.12.1+cpu       | project venv   |

`safetensors==0.8.0` matches the project's `worker/requirements/base.txt`. The `save_file` function from `safetensors.torch` is the standard API.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_zit_vae_fixture.py` | Builder script for ZiT VAE fixture checkpoints |
| CREATE | `worker/tests/fixtures/zit_vae_tiny.safetensors` | ZiT-VAE-shaped fixture with `arch: "zit_vae"` metadata (85,504 bytes) |
| CREATE | `worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors` | Metadata-fallback regression fixture (85,432 bytes) |

## Commit Log

```
 .forge/reports/P23-A1_plan.md                      | 202 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 worker/tests/fixtures/build_zit_vae_fixture.py     | 133 ++++++++++++++
 worker/tests/fixtures/zit_vae_tiny.safetensors     | Bin 0 -> 85504 bytes
 .../fixtures/zit_vae_tiny_no_metadata.safetensors  | Bin 0 -> 85432 bytes
 6 files changed, 345 insertions(+), 9 deletions(-)
```

## Test Results

### Rust tests (cargo test --workspace --features mock-hardware)

```
running 296 tests total — 0 failures

anvilml: 1 passed (cli_help_test) + 1 passed (config_reference) + 5 passed (db_startup_tests) + 1 passed (hw_probe_help_test) + 6 passed (logging_tests) + 2 passed (shutdown_tests) + 2 passed (startup_scan_tests)
anvilml_artifacts: 9 passed (store_tests)
anvilml_core: 1 passed (config_load_tests) + 3 passed (artifact_tests) + 13 passed (config_load_tests) + 13 passed (config_tests) + 17 passed (error_tests) + 10 passed (events_tests) + 9 passed (hardware_tests) + 4 passed (job_tests) + 4 passed (model_tests) + 5 passed (node_registry_tests) + 4 passed (node_tests) + 4 passed (worker_tests)
anvilml_hardware: 6 passed (cpu_tests) + 15 passed (detect_tests) + 6 passed (mock_tests) + 7 passed (sysfs_tests) + 8 passed (vulkan_tests)
anvilml_ipc: 7 passed (error_tests) + 26 passed (roundtrip_tests) + 1 passed (stress_test)
anvilml_registry: 4 passed (db_tests) + 5 passed (device_store_tests) + 9 passed (job_store_tests) + 20 passed (scanner_tests) + 8 passed (seed_loader_tests) + 5 passed (store_tests)
anvilml_scheduler: 35 passed (dag_tests) + 25 passed (event_loop_tests) + 6 passed (ledger_tests) + 10 passed (queue_tests) + 38 passed (scheduler_tests)
anvilml_server: 8 passed (artifacts_tests) + 2 passed (cors_tests) + 8 passed (handler_tests) + 1 passed (health_tests) + 25 passed (jobs_tests) + 6 passed (models_tests) + 5 passed (nodes_tests) + 12 passed (state_tests) + 5 passed (stats_tick_tests) + 7 passed (system_tests) + 7 passed (workers_tests)
anvilml_worker: 4 passed (pool) + 5 passed (bridge_tests) + 10 passed (demux_tests) + 7 passed (env_tests) + 5 passed (keepalive_tests) + 43 passed (managed_tests) + 5 passed (pool_tests) + 1 passed (real_startup_tests) + 6 passed (respawn_tests) + 6 passed (spawn_tests)

Doc-tests: 4 passed (anvilml-registry, anvilml-worker)
```

### Python mock-mode tests

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 192 items / 75 deselected / 117 selected
worker/tests/ - 117 passed, 75 deselected in 11.80s
```

### Python real-mode tests

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 192 items / 117 deselected / 75 selected
worker/tests/ - 75 passed, 117 deselected in 17.51s
```

## Format Gate

```
cargo fmt --all -- --check
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (already verified in cargo check --workspace --features mock-hardware above)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.50s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.69s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.61s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.82s
```

All four platform cross-checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync

```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out
```

### Gate 2 — OpenAPI Drift

Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity

Not triggered — this task does not add, remove, or rename a node type in `worker/nodes/`, nor does it modify `crates/anvilml-core/src/node_registry.rs`.

### Gate 4 — Mock/Real Parity Markers

Not triggered — this task creates a builder script (not a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`).

## Public API Delta

```
(grep of git diff HEAD for new pub items in modified files):
(no output — no new pub items introduced)
```

No new `pub` items introduced. This task creates a builder script (not a library module) — the `build()` function is a module-level function invoked via `if __name__ == "__main__"`.

## Deviations from Plan

None. Implementation followed the approved plan exactly.

## Blockers

None.

