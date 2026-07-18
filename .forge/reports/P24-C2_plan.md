# Plan Report: P24-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P24-C2                                      |
| Phase       | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/loader.py: EmptyLatent real branch via compute_latent_shape |
| Depends on  | P24-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-18T22:57:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `EmptyLatent`'s real branch in `worker/nodes/loader.py` by replacing the `NotImplementedError` stub with actual logic that dispatches to `arch.diffusion.get_module(model.arch).compute_latent_shape(width, height, batch_size)` and allocates a zero-filled latent tensor. In real mode, the `model` input is required — its absence raises a clear `ValueError`. This closes the scope deferred by P24-C1, with both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers updated to point at passing tests.

## Scope

### In Scope
- Replace the `NotImplementedError` stub in `EmptyLatent.execute()`'s real branch (`else:` block) with actual dispatch logic:
  - Check `inputs.get("model")` is present; raise `ValueError` with a clear message if absent.
  - Import `arch.diffusion.get_module` and dispatch to `get_module(model["arch"]).compute_latent_shape(width, height, batch_size)`.
  - Allocate a `torch.zeros(latent_shape)` tensor on `ctx.device`.
  - Return `{"latent": tensor}`.
- Update the `REAL_PATH_VERIFIED` marker on `EmptyLatent.execute()` to point at the new real-mode test.
- Update the `MOCK_PATH_VERIFIED` marker on `EmptyLatent.execute()` to point at the existing mock-mode test (already correct).
- Add >=5 new tests in `worker/tests/test_nodes_loader.py` covering:
  - Real mode without model input raises `ValueError`.
  - Real mode with loaded model produces latent matching `compute_latent_shape()`.
  - Real mode with different dimensions (e.g. 128×128) produces correct shape.
  - Real mode with non-default batch_size scales correctly.
  - Real mode with zero dimensions produces zero latent dims.

### Out of Scope
None. This task's `defers_to` field is empty (`[]`), so no scope may be deferred. The mock branch (already implemented in P24-C1) is not modified.

## Existing Codebase Assessment

The codebase has a complete `EmptyLatent` class in `worker/nodes/loader.py` (lines 296–391) with:
- A working mock branch that returns `{"mock": True, "shape": (batch_size, 4, height//8, width//8)}` — no torch import, matching every other node's mock branch pattern.
- A stubbed real branch (`raise NotImplementedError("... deferred to P24-C2")`).
- Existing markers: `REAL_PATH_VERIFIED` points at `test_empty_latent_real_raises_not_implemented` (the stub test), and `MOCK_PATH_VERIFIED` points at `test_empty_latent_mock_returns_placeholder_shape` (correct).

The architecture-specific `compute_latent_shape(width, height, batch_size)` function exists in `worker/nodes/arch/diffusion/zit.py` (lines 403–437). It reads from module-level globals `MODEL_PATCH_SIZE` and `MODEL_LATENT_CHANNELS` that are set by `load()`. The function uses ceiling division for non-multiple-of-patch-size dimensions and returns `(batch_size, MODEL_LATENT_CHANNELS, latent_height, latent_width)`. It is thoroughly tested in `test_arch_zit.py`.

The existing test file `worker/tests/test_nodes_loader.py` has three mock-mode EmptyLatent tests (lines 508–595) and one real-mode stub test (lines 598–618). The `_make_ctx` helper creates minimal `NodeContext` instances. Real-mode tests use `PipelineCache()` and fixture paths.

Established patterns to follow:
- Mock branch: sentinel dict with no torch import (already correct, no changes needed).
- Real branch: import the dispatch function inside the `else:` block (same pattern as LoadModel/LoadVae/LoadClip), check for missing inputs, raise clear errors, dispatch to arch module, return dict with output slot.
- Error style: `ValueError` for missing required input (not `KeyError` — more descriptive for user-facing errors).
- Test style: real-mode tests use `@pytest.mark.real_mode`, fixture paths via `Path(__file__).parent / "fixtures" / ...`, and verify the output tensor's shape and device.

No gap between design doc and source: `compute_latent_shape()` exists and works correctly; the only missing piece is the dispatcher call in `EmptyLatent.execute()`.

## Resolved Dependencies

None. No new external dependencies are introduced. The task uses only existing packages: `torch` (for `torch.zeros`), `worker.nodes.arch.diffusion.get_module` (existing dispatch function), and `worker.nodes.arch.diffusion.zit.compute_latent_shape` (existing function).

| Type   | Name                          | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------------------------|-----------------|----------------|------------------------|
| python | torch (existing)              | N/A             | n/a            | n/a                    |

## Approach

1. **Implement the real branch in `EmptyLatent.execute()`** in `worker/nodes/loader.py`:
   - Replace the `else:` block (currently `raise NotImplementedError(...)`) with:
     ```python
     # Real branch: model input is required — per §10.3's "required in real mode" note,
     # EmptyLatent dispatches to the loaded model's arch module for architecture-specific
     # latent shape computation. The mock branch ignores model entirely.
     model = inputs.get("model")
     if model is None:
         # model is required in real mode to determine the architecture-specific
         # latent shape formula. Without it, we cannot dispatch to the correct
         # arch module's compute_latent_shape().
         raise ValueError(
             "EmptyLatent requires a 'model' input in real mode; "
             "provide the output of LoadModel to this node."
         )

     # Dispatch to the model's arch module for compute_latent_shape().
     # model["arch"] is the architecture string (e.g. "zit") set by the
     # loader node's load() function. get_module() returns the matching
     # arch module (currently zit), whose compute_latent_shape() reads
     # module-level globals set by load() to produce the correct shape.
     from worker.nodes.arch.diffusion import get_module

     module = get_module(model["arch"])
     if module is None:
         # Defensive guard — if model["arch"] is "zit" (the only registered
         # arch), get_module("zit") should always return zit. This guard
         # catches unexpected arch strings from corrupted or future models.
         raise RuntimeError(
             f"no diffusion arch module registered for '{model['arch']}'; "
             f"cannot compute latent shape"
         )

     width = inputs["width"]
     height = inputs["height"]
     batch_size = inputs.get("batch_size", 1)

     # compute_latent_shape() reads module-level globals (MODEL_PATCH_SIZE,
     # MODEL_LATENT_CHANNELS) that were set by load() on this same module.
     # It returns (batch_size, latent_channels, latent_height, latent_width).
     latent_shape = module.compute_latent_shape(width, height, batch_size)

     # Allocate a zero-filled latent tensor on the worker's device.
     # torch.zeros produces a tensor with the exact shape returned by
     # compute_latent_shape(), ready to be passed to the Sampler's denoising loop.
     # The model's arch module's compute_latent_shape() already accounts for
     # the correct patch_size and latent_channels for this architecture.
     import torch

     latent = torch.zeros(latent_shape, device=ctx.device)
     return {"latent": latent}
     ```
   - Remove the old `# defers_to: P24-C2` comment from the stub (no longer needed — the scope is now implemented).
   - Add a `REAL_PATH_VERIFIED` marker pointing at the new real-mode test (see step 3).

2. **Update the `REAL_PATH_VERIFIED` marker** on `EmptyLatent.execute()`:
   - Change from `worker/tests/test_nodes_loader.py::test_empty_latent_real_raises_not_implemented` (the stub test) to `worker/tests/test_nodes_loader.py::test_empty_latent_real_produces_latent_with_loaded_model` (the new real-mode test).

3. **Update the `MOCK_PATH_VERIFIED` marker** on `EmptyLatent.execute()`:
   - Keep pointing at `worker/tests/test_nodes_loader.py::test_empty_latent_mock_returns_placeholder_shape` (already correct, no change needed).

4. **Add new tests in `worker/tests/test_nodes_loader.py`** (after the existing EmptyLatent tests, after line 618):
   - `test_empty_latent_real_raises_value_error_without_model`: Real mode without model input raises `ValueError` with a clear message mentioning "model".
   - `test_empty_latent_real_produces_latent_with_loaded_model`: Load the ZiT fixture, call `EmptyLatent.execute()` with `model` output, verify the returned latent tensor's shape matches `compute_latent_shape()`'s output (use the fixture's actual patch_size).
   - `test_empty_latent_real_different_dimensions`: Real mode with 128×128 dimensions produces correct shape `(1, 4, 32, 32)` (after load sets patch_size=4, 128/4=32).
   - `test_empty_latent_real_batch_size_scaling`: Real mode with `batch_size=3` produces shape `(3, 4, 8, 8)` for 64×64 dimensions.
   - `test_empty_latent_real_zero_dimensions`: Real mode with `width=0` or `height=0` produces zero latent dims.

5. **Update the `REAL_PATH_VERIFIED` marker on the old stub test** `test_empty_latent_real_raises_not_implemented`: Remove this test entirely (it tested the stub, which is now replaced). The new real-mode tests provide the REAL_PATH_VERIFIED coverage.

## Public API Surface

No new public Python items are introduced. The only change is to an existing method signature:

| Item | Before | After |
|------|--------|-------|
| `worker.nodes.loader.EmptyLatent.execute(self, ctx: NodeContext, **inputs) -> dict` | Raises `NotImplementedError` in real mode | Returns `{"latent": torch.Tensor}` in real mode; still raises `ValueError` if model is absent |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Replace EmptyLatent's real branch stub with dispatch to `arch.diffusion.get_module().compute_latent_shape()` and `torch.zeros()` allocation; update markers |
| Modify | `worker/tests/test_nodes_loader.py` | Add >=5 new real-mode tests; remove the old `test_empty_latent_real_raises_not_implemented` stub test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_raises_value_error_without_model` (real) | Real mode without model input raises `ValueError` with clear message | `mock=False` context | `width=64, height=64` | `ValueError` raised | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k "test_empty_latent_real_raises_value_error_without_model"` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_produces_latent_with_loaded_model` (real) | Real mode with loaded ZiT model produces latent matching `compute_latent_shape()` output | ZiT fixture loaded, `mock=False` | `width=64, height=64, model=<loaded_model>` | `torch.Tensor` with shape `(1, 4, 8, 8)` on CPU | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k "test_empty_latent_real_produces_latent_with_loaded_model"` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_different_dimensions` (real) | Real mode with 128×128 dimensions produces correct shape | ZiT fixture loaded, `mock=False` | `width=128, height=128` | `torch.Tensor` with shape `(1, 4, 32, 32)` | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k "test_empty_latent_real_different_dimensions"` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_batch_size_scaling` (real) | Real mode with `batch_size=3` scales correctly | ZiT fixture loaded, `mock=False` | `width=64, height=64, batch_size=3` | `torch.Tensor` with shape `(3, 4, 8, 8)` | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k "test_empty_latent_real_batch_size_scaling"` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_zero_dimensions` (real) | Real mode with zero width/height produces zero latent dims | ZiT fixture loaded, `mock=False` | `width=0, height=32` | `torch.Tensor` with shape `(1, 4, 0, 4)` | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k "test_empty_latent_real_zero_dimensions"` exits 0 |

## CI Impact

The `worker-linux-real` and `worker-windows-real` CI jobs pick up the new `@pytest.mark.real_mode` tests automatically. No CI configuration changes are needed — the `real_mode` marker is already registered in `pyproject.toml` / `pytest.ini`. The existing `worker-linux-mock` and `worker-windows-mock` jobs are unaffected since the new tests are `real_mode`-only.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. `torch.zeros(device=ctx.device)` works on all supported platforms (CPU, CUDA, ROCm). No `#[cfg(unix)]` / `#[cfg(windows)]` guards are needed — this is pure Python worker code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `model["arch"]` may not exist on the sentinel dict returned by mock-mode loader nodes (if a test accidentally passes a mock sentinel as `model`). | Low | Medium | The real branch only runs when `ctx.mock=False`, so mock-mode sentinels are never passed. Add a defensive check: `if "arch" not in model: raise ValueError(...)`. |
| `compute_latent_shape()` reads module-level globals that are set by `load()`. If `load()` hasn't been called, it uses defaults (patch_size=2, latent_channels=4) which may not match the actual checkpoint. | Low | Medium | This is the documented design — `load()` must be called first. The test calls `LoadModel` (which calls `load()`) before `EmptyLatent`, so the globals are always set. Document this in the test's docstring. |
| The new real-mode tests require torch to be importable at test collection time (they're `real_mode`-marked, so they're excluded from mock-mode CI). | Low | Low | The `real_mode` marker ensures these tests only run in the `worker-*-real` CI jobs where torch is installed. No action needed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v -m "not real_mode"` exits 0 (no regressions to mock tests)
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/loader.py | grep -q "test_empty_latent_real_produces_latent_with_loaded_model"` — marker points to a real test
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/loader.py | grep -q "test_empty_latent_mock_returns_placeholder_shape"` — marker still points to the mock test
- [ ] `grep -c "NotImplementedError" worker/nodes/loader.py` returns 0 — no stub remains
