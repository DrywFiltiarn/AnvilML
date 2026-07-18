# Implementation Report: P24-C2

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P24-C2                                        |
| Phase       | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/loader.py: EmptyLatent real branch via compute_latent_shape |
| Implemented | 2026-07-19T01:20:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Replaced the `NotImplementedError` stub in `EmptyLatent.execute()`'s real branch with actual dispatch logic. The real branch now checks for a `model` input (raises `ValueError` if absent), dispatches to the loaded model's arch module via `get_module(model.arch).compute_latent_shape(width, height, batch_size)`, and allocates a zero-filled `torch.Tensor` on the worker's device. Five new real-mode tests were added and one old stub test was removed. All markers updated.

## Resolved Dependencies

| Type   | Name | Version resolved | Source |
|--------|------|------------------|--------|
| (none) | —    | —                | —      |

No new dependencies introduced. The task uses only existing packages: `torch` (for `torch.zeros`), `worker.nodes.arch.diffusion.get_module` (existing dispatch function), and `worker.nodes.arch.diffusion.zit.compute_latent_shape` (existing function).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Replace EmptyLatent's real branch stub with dispatch to `arch.diffusion.get_module().compute_latent_shape()` and `torch.zeros()` allocation; update markers; use `getattr(model, "arch")` instead of `model["arch"]` (ZiTModel is a torch.nn.Module, not a dict) |
| Modify | `worker/tests/test_nodes_loader.py` | Remove old stub test; add 5 new real-mode tests; fix expected shapes to match fixture's actual patch_size=4 |
| Modify | `docs/TESTS.md` | Add entries for the 5 new real-mode tests |

## Commit Log

```
 .forge/reports/P24-C2_plan.md     | 184 ++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 docs/TESTS.md                     |  10 ++
 worker/nodes/loader.py            |  89 +++++++++++++---
 worker/tests/test_nodes_loader.py | 206 ++++++++++++++++++++++++++++++++++++--
 6 files changed, 475 insertions(+), 33 deletions(-)
```

## Test Results

```
# Mock-mode (142 passed)
$ ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
===================== 142 passed, 125 deselected in 6.59s ======================

# Real-mode (125 passed)
$ worker/.venv/bin/python -m pytest worker/tests/ -v -m real_mode
===================== 125 passed, 142 deselected in 20.98s =====================
```

## Format Gate

```
$ cargo fmt --all -- --check
(no output — clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (already passed during cargo test)
# 2. Mock-hardware Windows
Checking anvilml v0.1.18 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.87s

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.18s

# 4. Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.43s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
$ cargo test -p anvilml --features mock-hardware -- config_reference_matches_defaults
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

### Gate 4 — Mock/Real Parity Markers
```
# Check 1: All markers name collectible tests — all passed
# Check 2: No files lack REAL_PATH_VERIFIED or MOCK_PATH_VERIFIED markers — clean
```

## Public API Delta

```
$ git diff HEAD -- worker/nodes/loader.py | grep '^+.*def \|^[+-].*class '
+            model = inputs.get("model")
+            if model is None:
+                raise ValueError(...)
+            from worker.nodes.arch.diffusion import get_module
+            module = get_module(arch)
+            if module is None:
+                raise RuntimeError(...)
+            width = inputs["width"]
+            height = inputs["height"]
+            batch_size = inputs.get("batch_size", 1)
+            latent_shape = module.compute_latent_shape(width, height, batch_size)
+            import torch
+            latent = torch.zeros(latent_shape, device=ctx.device)
+            return {"latent": latent}
```

No new public items introduced. The only change is to an existing method signature:
- `worker.nodes.loader.EmptyLatent.execute()` — still returns `{"latent": ...}`; the return type changed from "raises NotImplementedError" to "returns dict with torch.Tensor"

## Deviations from Plan

1. **`model["arch"]` → `getattr(model, "arch")`**: The plan specified `model["arch"]` (dict subscript), but the model returned by `LoadModel.execute()` in real mode is a `ZiTModel` object (a `torch.nn.Module`), not a dict. The mock branch returns `{"mock": True, "model_id": ...}` (a dict), which is why the plan's dict-style access was written. In real mode, the model is a real `ZiTModel` instance with an `.arch` attribute. Used `getattr(model, "arch", None)` instead, which is both safer (returns None on missing attribute) and consistent with how the model carries its architecture identifier.

2. **Test expected shapes corrected**: The plan specified expected shapes of `(1, 4, 8, 8)` for 64×64 and `(3, 4, 8, 8)` for batch_size=3 with 64×64. However, the `zit_tiny.safetensors` fixture's `load()` sets `MODEL_PATCH_SIZE=4` (not 8), so `compute_latent_shape(64, 64, 1)` returns `(1, 4, 16, 16)` (64/4=16, not 64/8=8). The test expectations were corrected to `(1, 4, 16, 16)` and `(3, 4, 16, 16)`. Similarly, `width=0, height=32` produces `(1, 4, 0, 8)` (0/4=0, 32/4=8), not `(1, 4, 0, 4)` as the plan stated.

3. **Removed `test_empty_latent_real_raises_not_implemented`**: The plan said to update the marker on this test, but the correct action was to remove it entirely since it tested the stub (which is now replaced). The new real-mode tests provide the REAL_PATH_VERIFIED coverage.

## Blockers

None.
