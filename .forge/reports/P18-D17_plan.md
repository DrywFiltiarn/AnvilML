# Plan Report: P18-D17

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D17                                     |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/sampler.py: EmptyLatent gains optional model input and real noise tensor path |
| Depends on  | P18-D16, P18-D3b                            |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add an optional `model:MODEL` input slot to `EmptyLatent` and implement its real-mode code path: dispatch to the loaded model's architecture module, read `num_channels_latents` from `model.in_channels`, call `compute_latent_shape()` (architecture-specific formula), and return a `torch.randn` noise tensor. In mock mode the node behaves exactly as before — the new slot is simply ignored. Existing mock tests continue to pass unchanged.

## Scope

### In Scope
- Add `SlotSpec("model", "MODEL", optional=True)` as the 4th slot in `EmptyLatent.INPUT_SLOTS` (after `batch_size`).
- Update `EmptyLatent.execute()` real path: if `model` is provided, dispatch via `arch.get_module(model)`, read `num_channels_latents` from `model.in_channels`, call `mod.compute_latent_shape(batch_size, height, width, num_channels_latents)`, and return `torch.randn(shape, dtype=torch.float32, device=ctx.device)`.
- If `model` is absent in real mode, raise `ValueError("EmptyLatent real path requires a model input")`.
- Add inline comment explaining why shape computation is delegated to the arch module.
- Update `test_emptylatent_metadata_attributes` to expect 4 INPUT_SLOTS instead of 3.
- Update the docstring on `EmptyLatent.execute()` to remove the `NotImplementedError` mention and describe the real path.

### Out of Scope
None. `defers_to (from JSON): absent` — this task must implement its full scope.

## Existing Codebase Assessment

**What already exists:** `EmptyLatent` is defined in `worker/nodes/sampler.py` with 3 INPUT_SLOTS (`width`, `height`, `batch_size`) and a mock code path that returns `MockLatent(width, height, batch_size)`. The real path raises `NotImplementedError`. The `arch` package at `worker/nodes/arch/__init__.py` re-exports `can_handle()` and `get_module()` from `arch/diffusion/__init__.py`. `arch/diffusion/zit.py` provides `compute_latent_shape(batch_size, height, width, num_channels_latents)` which returns a `(batch_size, num_channels_latents, h, w)` tuple. `NodeContext` (from `base.py`) provides `ctx.device` (a string like `"cuda:0"` or `"cpu"`) and `ctx.pipeline_cache`.

**Established patterns:** The mock-vs-real split uses `os.environ.get("ANVILML_WORKER_MOCK") == "1"` as a runtime check. Real-mode heavy imports (`torch`, `diffusers`) are lazy — inside the non-mock branch. Error handling uses `ValueError` (e.g., `loader.py` raises `ValueError(f"unsupported clip_type: {clip_type!r}")`). The `@register` decorator validates all six metadata attributes at import time. Tests use `importlib.reload()` to re-register nodes against a cleared `NODE_REGISTRY`.

**Gap between design doc and source:** The design doc §10.3 already reflects the updated EmptyLatent row with `model: Model?` as an optional input — no design doc change is needed. However, `NodeError` is referenced in the design doc and task context but does not exist in the codebase; the established pattern uses `ValueError` instead (confirmed by P18-D6's implementation report noting the same discrepancy).

## Resolved Dependencies

None. No new external dependencies are introduced. The task uses `torch` which is already a dependency of the Python worker (`worker/requirements/base.txt`), imported lazily inside the real-mode path.

## Approach

1. **Add the optional `model` input slot to `EmptyLatent.INPUT_SLOTS`.**
   In `worker/nodes/sampler.py`, change `INPUT_SLOTS` from a 3-element list to a 4-element list by appending `SlotSpec("model", "MODEL", optional=True)` as the 4th slot (after `batch_size`). This makes the slot optional so existing job graphs without it still pass registration.

2. **Update `EmptyLatent.execute()` docstring.**
   Replace the `NotImplementedError` mention in the docstring `Raises:` section with a description of the real path (dispatch to arch module, compute shape, return noise tensor).

3. **Implement the real-mode code path in `execute()`.**
   After the mock-mode `if` block and before the `raise NotImplementedError`, insert the real path:
   - Read `model = inputs.get("model")`.
   - Check if `model` is `None` — if so, raise `ValueError("EmptyLatent real path requires a model input")`.
   - Dispatch: `mod = arch.get_module(model)`. If `mod` is `None`, raise `ValueError(f"EmptyLatent: unsupported model architecture for {model}")`.
   - Read `num_channels_latents = model.in_channels` (this attribute is set by `LoadModel`'s real path in P18-D4/P18-D13).
   - Call `shape = mod.compute_latent_shape(batch_size, height, width, num_channels_latents)`.
   - Import `torch` lazily (inside the real-mode branch) and return `{"latent": torch.randn(shape, dtype=torch.float32, device=ctx.device)}`.
   - Add an inline comment explaining why shape computation is delegated to the arch module (architecture-specific formula; Flux 2 Klein uses a structurally different packing scheme).

4. **Update the test for EmptyLatent metadata.**
   In `worker/tests/test_nodes_sampler.py`, update `test_emptylatent_metadata_attributes` to expect `len(EmptyLatent.INPUT_SLOTS) == 4` and add an assertion for the 4th slot (`model`, `MODEL`, optional=True).

5. **Verify mock-mode tests still pass.**
   The mock code path is unchanged — it reads `width`, `height`, `batch_size` and returns `MockLatent`. The new `model` slot is optional and ignored in mock mode. All existing mock tests continue to pass.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `EmptyLatent.INPUT_SLOTS` | class attribute (list) | `worker.nodes.sampler.EmptyLatent.INPUT_SLOTS` | Extended from 3 to 4 slots; 4th slot is `SlotSpec("model", "MODEL", optional=True)` |
| `EmptyLatent.execute()` | method | `worker.nodes.sampler.EmptyLatent.execute` | Real path now dispatches to arch module; docstring updated |

No new `pub` items or new exported functions. The change is additive to an existing class attribute.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/sampler.py` | Add `model` slot to `EmptyLatent.INPUT_SLOTS`; implement real path in `execute()` |
| MODIFY | `worker/tests/test_nodes_sampler.py` | Update `test_emptylatent_metadata_attributes` for 4 slots |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_metadata_attributes` | EmptyLatent has 4 INPUT_SLOTS including the new `model` slot | `ANVILML_WORKER_MOCK=1` (conftest), NODE_REGISTRY cleared | None (class-level inspection) | 4 slots: width(INT, req), height(INT, req), batch_size(INT, opt), model(MODEL, opt) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_execute_returns_mock_latent` (existing) | Mock mode still returns MockLatent with correct dimensions | `ANVILML_WORKER_MOCK=1` | `width=512, height=512, batch_size=4` | `MockLatent(512, 512, 4)` | Same command as above exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_default_batch_size` (existing) | Default batch_size=1 in mock mode | `ANVILML_WORKER_MOCK=1` | `width=512, height=512` (no batch_size) | `MockLatent(512, 512, 1)` | Same command as above exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_registered_in_registry` (existing) | EmptyLatent is registered in NODE_REGISTRY | `ANVILML_WORKER_MOCK=1`, NODE_REGISTRY cleared | None (import) | `"EmptyLatent" in NODE_REGISTRY` | Same command as above exits 0 |

## CI Impact

No CI changes required. The modified files are Python source and test files, which are covered by the existing `worker-linux` and `worker-windows` CI jobs (`py_compile` + `pytest worker/tests/ -v`). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `torch.randn(..., device=ctx.device)` call uses whatever device string is in `NodeContext` (e.g., `"cuda:0"`, `"cpu"`), which is already handled by the existing worker infrastructure. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NodeError` referenced in task context does not exist in the codebase; using `ValueError` instead. A future task or the design doc may expect `NodeError`. | High | Medium | Use `ValueError` to match the existing error pattern in `loader.py` (P18-D6 already did this). Document the discrepancy in the plan. If a `NodeError` type is defined later, it can be adopted in a follow-up task without breaking this one. |
| The real path imports `torch` lazily — if `torch` is not installed on the target system, the lazy import will fail with `ModuleNotFoundError` rather than a clean error. | Low | Low | This is the established pattern used by all other nodes (sampler.py, zit.py, loader.py). The worker's preflight checks (ENVIRONMENT.md §5) already verify `import torch` succeeds before dispatching jobs. |
| Adding the 4th slot changes the `test_emptylatent_metadata_attributes` assertion from 3 to 4 slots — if the test is not updated, it will fail. | Low | Medium | The test update is part of this task's scope (step 4 in Approach). The acceptance command will catch any omission. |
| `model.in_channels` attribute may not exist on the model object if `LoadModel`'s real path (P18-D4/P18-D13) is not yet deployed. | Medium | High | This task's prereqs include P18-D4 (LoadModel real path). The task JSON confirms this dependency. If the attribute is missing, the code will raise `AttributeError` — which is a clear signal that a prerequisite task is missing. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/sampler.py worker/tests/test_nodes_sampler.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 (all existing mock tests pass, including updated metadata test)
- [ ] `grep -c 'SlotSpec("model", "MODEL", optional=True)' worker/nodes/sampler.py` outputs `1`
- [ ] `grep -c 'arch.get_module(model)' worker/nodes/sampler.py` outputs `>= 1`
- [ ] `grep -c 'compute_latent_shape' worker/nodes/sampler.py` outputs `>= 1`
- [ ] `grep -c 'torch.randn' worker/nodes/sampler.py` outputs `>= 1`
- [ ] `grep -c 'requires a model input' worker/nodes/sampler.py` outputs `>= 1`
