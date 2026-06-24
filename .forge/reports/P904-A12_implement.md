# Implementation Report: P904-A12

| Field         | Value                                                           |
|---------------|-----------------------------------------------------------------|
| Task ID       | P904-A12                                                        |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)           |
| Description   | worker/nodes/loader.py: rewrite LoadModel/LoadVae/LoadClip's loader functions as thin per-arch wrappers |
| Implemented   | 2026-06-24T12:30:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Refactored `worker/nodes/loader.py` so that all three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`) dispatch loading through the architecture registry system instead of calling diffusers classes directly. `_load_model_from_hf_directory` was renamed to `_load_model_from_safetensors` and its `ZImageTransformer2DModel.from_single_file()` call was replaced with `arch.diffusion.get_module_by_name(detected_arch).load_transformer(model_id)`. `LoadVae.execute()`'s inline `loader_fn` closure was extracted into a named `_load_vae_from_safetensors(model_id, arch, device)` function that dispatches through `arch.diffusion.get_module_by_name(arch).load_vae(model_id)`. `LoadClip.execute()`'s dispatch was extracted into a named `_load_clip_from_safetensors(model_id, clip_type, device)` wrapper. The `arch.diffusion` module gained a new `get_module_by_name(arch)` function (used by all three wrappers) that constructs a shim object with `arch` and delegates to the existing `get_module()` dispatcher. All 95 Python tests pass; mock-mode paths remain unchanged.

## Resolved Dependencies

None. This task introduces no new dependencies — it reuses existing `arch.diffusion.get_module_by_name()` (implemented as part of this task's scope since `defers_to` is empty) and existing `load_transformer()`/`load_vae()` functions from zit.py.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Added `get_module_by_name(arch: str)` function and exported it in `__all__` |
| MODIFY | `worker/nodes/loader.py` | Renamed `_load_model_from_hf_directory` → `_load_model_from_safetensors`; replaced `ZImageTransformer2DModel.from_single_file()` with arch dispatch; extracted inline `loader_fn` → `_load_vae_from_safetensors`; extracted inline dispatch → `_load_clip_from_safetensors`; updated all three `execute()` call sites; added `arch_diffusion` import |
| MODIFY | `worker/tests/test_nodes_loader.py` | Renamed `test_loadmodel_hf_directory_accepts_device_param` → `test_loadmodel_safetensors_accepts_device_param`; updated import and assertion strings |
| MODIFY | `docs/TESTS.md` | Added entry for `test_loadmodel_safetensors_accepts_device_param` |

## Commit Log

```
 .forge/reports/P904-A12_plan.md         | 148 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 +-
 docs/TESTS.md                           |   9 ++
 worker/nodes/arch/diffusion/__init__.py |  27 +++-
 worker/nodes/loader.py                  | 215 ++++++++++++++++++++------------
 worker/tests/test_nodes_loader.py       |  10 +-
 7 files changed, 330 insertions(+), 98 deletions(-)
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
worker/tests/test_nodes_loader.py::test_loadmodel_safetensors_accepts_device_param PASSED [100%]

============================== 12 passed in 2.03s ==============================
```

Full worker test suite: 95 passed in 17.84s.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0 with no drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
=== CHECK 1 PASS ===

# Check 2: Mock-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
=== CHECK 2 PASS ===

# Check 3: Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
=== CHECK 3 PASS ===

# Check 4: Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
=== CHECK 4 PASS ===
```

All four checks passed.

## Project Gates

- **Gate 1 (Config Surface Sync):** Not applicable — task does not modify config fields.
- **Gate 2 (OpenAPI Drift):** Not applicable — task does not modify handler signatures or `#[utoipa::path]` annotations.
- **Gate 3 (Node Parity):** Not applicable — `worker/tests/test_parity.py` does not yet exist in the repository.

## Public API Delta

```
(No new pub items — grep returned empty)
```

All changes are to private module-level functions (prefixed with `_`) and internal method call sites. The `__all__` list in `loader.py` is unchanged. The `worker/nodes/arch/diffusion/__init__.py` adds `get_module_by_name` to `__all__` — this is a new public function but it is an internal dispatch helper (not a node type or user-facing API), consistent with the existing `get_module` and `can_handle` exports.

## Deviations from Plan

- **`get_module_by_name` implementation:** The plan states `get_module_by_name` is implemented in P904-A13, but P904-A12's `defers_to` is empty and the plan requires using it. Since empty `defers_to` forbids deferring in-scope functionality (FORGE_AGENT_RULES §4.7a/§9.7a), I implemented `get_module_by_name` directly in this task. The implementation constructs a shim object with `arch=arch` and delegates to the existing `get_module()` dispatcher — this matches the plan's stated intent exactly.

## Blockers

None.
