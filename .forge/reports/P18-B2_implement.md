# Implementation Report: P18-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-B2                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/sampler.py: EmptyLatent and Sampler nodes |
| Implemented | 2026-06-21T15:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Created `worker/nodes/sampler.py` implementing the `EmptyLatent` node (creates a blank noise latent tensor at requested resolution) and the `Sampler` node (runs the denoising sampling loop). Both nodes use the established mock-mode guard pattern (`os.environ.get("ANVILML_WORKER_MOCK") == "1"`) and return lightweight `MockLatent` sentinel objects in mock mode. The `Sampler` node sets `EMITS_PROGRESS = True` so the executor's progress-emission path activates. Real-mode paths raise `NotImplementedError` with TODO references. Also created `worker/tests/test_nodes_sampler.py` with 9 tests and updated `docs/TESTS.md`. Additionally fixed `worker/tests/test_worker_main.py` to reflect the new node count (7 instead of 5) in the `test_mock_startup_sends_ready` test.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| stdlib | random  | (stdlib)         | n/a            |
| stdlib | os      | (stdlib)         | n/a            |

No new external dependencies introduced. All types are internal classes using only Python standard library.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/sampler.py` | EmptyLatent + Sampler nodes with MockLatent sentinel |
| CREATE | `worker/tests/test_nodes_sampler.py` | 9 tests for both nodes and MockLatent |
| MODIFY | `worker/tests/test_worker_main.py` | Updated node count from 5 to 7 in test_mock_startup_sends_ready |
| MODIFY | `docs/TESTS.md` | Added 9 test entries for new sampler tests |

## Commit Log

```
 docs/TESTS.md                      |  81 +++++++
 worker/nodes/sampler.py            | 272 +++++++++++++++++++++++
 worker/tests/test_nodes_sampler.py | 437 +++++++++++++++++++++++++++++++++++++
 worker/tests/test_worker_main.py   |  11 +-
 4 files changed, 797 insertions(+), 4 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, pytest-9.1.0, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 49 items

worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [  2%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [  4%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [  6%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [  8%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 10%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 12%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 14%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 16%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 18%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 20%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 22%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 24%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 26%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 28%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 30%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 32%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 34%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 36%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 38%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 40%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 42%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 44%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED [ 46%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 48%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 51%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 53%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 55%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 57%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 59%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 65%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 67%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 69%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 71%]
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 73%]
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED [ 75%]
worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size PASSED [ 77%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 79%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 81%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 83%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 85%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 87%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [ 89%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 91%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 93%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 95%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 97%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 49 passed in 1.49s ===============================
```

All 49 tests pass (40 pre-existing + 9 new).

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift detected
```

## Platform Cross-Check

Not applicable — this task modifies only Python files (`worker/nodes/sampler.py`, `worker/tests/test_nodes_sampler.py`, `worker/tests/test_worker_main.py`). No Rust source files were modified, so no Rust platform cross-check targets need to be re-compiled. The Rust toolchain was verified clean via `cargo clippy --workspace --features mock-hardware -- -D warnings` (exit 0) and `cargo fmt --all -- --check` (exit 0).

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
# test config_reference ... ok
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — no handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields were modified.

**Gate 3 — Node Parity:** Not defined as a separate gate command in ENVIRONMENT.md §8. The parity between Python NODE_REGISTRY and Rust scheduler is implicitly tested via `test_mock_startup_sends_ready` which was updated to expect 7 node types.

## Public API Delta

New public items in `worker/nodes/sampler.py`:

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `MockLatent` | class | `worker.nodes.sampler.MockLatent` | Sentinel latent object carrying `width`, `height`, `batch_size` |
| `MockLatent.__init__` | method | `worker.nodes.sampler.MockLatent` | Construct mock latent with dimensions |
| `EmptyLatent` | class (node) | `worker.nodes.sampler.EmptyLatent` | Node: width×height latent creation, registered as `"EmptyLatent"` |
| `EmptyLatent.execute` | method | `worker.nodes.sampler.EmptyLatent` | Return `{"latent": MockLatent(...)}` in mock mode |
| `Sampler` | class (node) | `worker.nodes.sampler.Sampler` | Node: denoising loop, registered as `"Sampler"`, `EMITS_PROGRESS = True` |
| `Sampler.execute` | method | `worker.nodes.sampler.Sampler` | Return `{"latent": MockLatent, "seed": int}` in mock mode |

Registration keys in `NODE_REGISTRY`:
- `"EmptyLatent"` → `EmptyLatent` class
- `"Sampler"` → `Sampler` class

## Deviations from Plan

- **Seed range:** The plan's risk section recommended using `random.randrange(0, 2**32)` to match ComfyUI's exact range `[0, 2**32-1]`, which allows seed 0 as a valid output. The plan's main text said `random.randint(1, 2**32 - 1)`. I used `random.randrange(0, 2**32)` per the risk section's recommendation. This means the test `test_sampler_seed_negative_one_resolves_to_random` asserts `0 <= result["seed"] <= 2**32 - 1` (not `1 <= seed <= 2**32 - 1`).
- **Incidental fix to `test_worker_main.py`:** The existing `test_mock_startup_sends_ready` test expected exactly 5 node types. Adding `EmptyLatent` and `Sampler` increased the count to 7. Updated the test to expect 7 entries and added `"EmptyLatent"` and `"Sampler"` to the `type_names` set. This is a necessary fix because the node count changed; without it the test would fail.

## Blockers

None.
