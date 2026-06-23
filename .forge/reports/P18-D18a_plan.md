# Plan Report: P18-D18a

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D18a                                    |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/diffusion/zit.py: assemble ZImagePipeline from cached components |
| Depends on  | P18-D17                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace `sample()`'s `NotImplementedError` real path with pipeline assembly code that constructs a `diffusers.ZImagePipeline` from cached transformer, VAE, text encoder, tokenizer, and scheduler components via `pipeline_cache.get_or_load()`, stores it as a local variable, and does NOT invoke it (that is deferred to P18-D18c). Also fix the module docstring's incorrect description of the real callback shape and add a test asserting `get_or_load` is called with the pipeline cache key.

## Scope

### In Scope
- Replace the `NotImplementedError` in `sample()`'s real mode path with pipeline assembly logic:
  - Construct a `loader_fn` closure that builds `diffusers.ZImagePipeline(scheduler, vae, text_encoder, tokenizer, transformer)` from the `model`, `conditioning`, and `vae` arguments passed to `sample()`.
  - Call `self.ctx.pipeline_cache.get_or_load(f"{model_id}:pipeline", dtype, loader_fn)` and store the result in a local variable `pipeline`.
  - Do NOT call `pipeline(...)` — leave the invocation for P18-D18c.
  - Add `# defers_to: P18-D18c -- pipeline assembled, not yet invoked` comment at the stub site per FORGE_AGENT_RULES.md §9.7.
- Fix the module docstring which currently wrongly describes `emit_progress` as the real callback shape (it is actually `callback_on_step_end(self, i, t, callback_kwargs)` per `diffusers` source).
- Add a test in `test_arch_zit.py` asserting `get_or_load` is called with the pipeline cache key `"test_model:pipeline"`.

### Out of Scope
- Pipeline invocation — P18-D18c handles calling `pipeline(...)` with `output_type="latent"` and cancellation handling.
- `_make_callback()` adapter — P18-D18b handles bridging the 4-argument `callback_on_step_end` to the 2-argument `emit_progress` interface.
- Real-mode `sample()` execution path — the assembled pipeline is stored but never called.

## Existing Codebase Assessment

**What already exists:** The module `worker/nodes/arch/diffusion/zit.py` already has `can_handle()`, `compute_latent_shape()`, `MockLatent`, and `VAE_SCALE_FACTOR`. The `sample()` function has a mock path that returns `(MockLatent(), seed)` and a real path that raises `NotImplementedError`. The module docstring incorrectly describes the real callback as `emit_progress(step, total)` — the actual `diffusers` API uses `callback_on_step_end(self, i, t, callback_kwargs)`.

**Established patterns:** The mock mode check uses `os.environ.get("ANVILML_WORKER_MOCK") == "1"` at runtime (not module-level import guard). All heavy imports (`torch`, `diffusers`, `safetensors`) are lazy — inside the non-mock code path. The `PipelineCache.get_or_load()` API takes `(model_id, dtype, loader_fn)` where `loader_fn` is a zero-argument callable. The `model` object carries `.arch` and `.in_channels` attributes (from `RealModel` or `MockModel`). The `conditioning` object carries `.positive` and `.negative` attributes (from `Conditioning` or `MockConditioning`). The `vae` argument to `sample()` is a VAE component.

**Gap between design doc and current source:** The module docstring says the real callback is `emit_progress(step, total)` — this is wrong. The actual `diffusers.ZImagePipeline.__call__` signature uses `callback_on_step_end(self, i, t, callback_kwargs)`. This gap is being fixed by this task.

## Resolved Dependencies

| Type   | Name                  | Version verified | MCP source          | Feature flags confirmed |
|--------|-----------------------|-----------------|---------------------|------------------------|
| python | diffusers             | 0.38.0          | pypi-query MCP      | n/a                    |
| python | transformers          | 5.12+ (project req) | pypi-query MCP  | n/a                    |

Verified via `pypi-query MCP`: `diffusers` latest version is 0.38.0. The project's `worker/requirements/base.txt` requires `diffusers>=0.38.0`. `ZImagePipeline` constructor confirmed from source at `src/diffusers/pipelines/z_image/pipeline_z_image.py`: `__init__(self, scheduler: FlowMatchEulerDiscreteScheduler, vae: AutoencoderKL, text_encoder: PreTrainedModel, tokenizer: AutoTokenizer, transformer: ZImageTransformer2DModel)`. `FlowMatchEulerDiscreteScheduler` confirmed from `src/diffusers/schedulers/scheduling_flow_match_euler_discrete.py`.

## Approach

1. **Fix the module docstring** in `worker/nodes/arch/diffusion/zit.py`:
   - Change the sentence describing `sample()`'s real path callback from "calls `emit_progress(step, total)`" to the correct description: "calls `callback_on_step_end(self, i, t, callback_kwargs)` via the pipeline's `callback_on_step_end` hook" — note that the actual invocation (bridged by P18-D18b) is deferred, but the docstring must describe the real diffusers API shape, not the wrong one.

2. **Replace the `NotImplementedError` in `sample()`'s real mode path** with pipeline assembly:
   - Inside the real mode block (after `_mock = False`), add lazy imports for `diffusers.ZImagePipeline` and `diffusers.FlowMatchEulerDiscreteScheduler` — these must be lazy (inside the non-mock path) to preserve mock-mode import isolation.
   - Extract `model_id` from `model` — use `getattr(model, "model_id", str(model))` as a fallback since `MockModel` doesn't carry `model_id` but real `RealModel` will (P18-D4). For the test, use a mock model with `model_id` attribute.
   - Construct the `loader_fn` closure:
     ```python
     def loader_fn():
         # Pull components from model, conditioning, and vae arguments
         transformer = getattr(model, "_transformer", None)
         if transformer is None:
             # Fallback: the model object itself is the transformer
             transformer = model
         tokenizer = getattr(conditioning, "tokenizer", None)
         text_encoder = getattr(conditioning, "text_encoder", None)
         vae = vae  # passed as argument to sample()
         scheduler = FlowMatchEulerDiscreteScheduler()
         return ZImagePipeline(
             scheduler=scheduler,
             vae=vae,
             text_encoder=text_encoder,
             tokenizer=tokenizer,
             transformer=transformer,
         )
     ```
   - Call `self.ctx.pipeline_cache.get_or_load(f"{model_id}:pipeline", dtype, loader_fn)` and store in local variable `pipeline`.
   - Add the defers_to comment: `# defers_to: P18-D18c -- pipeline assembled, not yet invoked`
   - The `dtype` parameter for `get_or_load` — use `"fp8"` to match the convention used by `LoadModel` (which calls `get_or_load(model_id, "fp8", ...)`).
   - Do NOT call `pipeline(...)` — leave that for P18-D18c.

3. **Add a test** in `worker/tests/test_arch_zit.py`:
   - `test_sample_real_assembles_pipeline_via_cache`: Mock `self.ctx.pipeline_cache` (or use a mock model that carries a `ctx` with a mocked cache). Call `sample()` in real mode (with `ANVILML_WORKER_MOCK=0`). Assert that `get_or_load` was called with the pipeline cache key containing `:pipeline`.

## Public API Surface

No new public items are introduced. The task modifies existing public items:
- `sample()` function signature unchanged: `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]`
- Module docstring updated (not a signature change)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Replace NotImplementedError with pipeline assembly; fix module docstring |
| MODIFY | `worker/tests/test_arch_zit.py` | Add test asserting get_or_load called with pipeline cache key |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_sample_real_assembles_pipeline_via_cache` | In real mode, `sample()` calls `pipeline_cache.get_or_load()` with a key containing `:pipeline` | `ANVILML_WORKER_MOCK` temporarily set to `"0"`; `pipeline_cache` mock configured | Mock model with `model_id="test_model"`, mock conditioning, mock vae, `steps=4`, `seed=42` | `get_or_load.assert_called_once()` with key `"test_model:pipeline"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |

## CI Impact

No CI changes required. The test file already exists and is picked up by the existing `worker-linux` and `worker-windows` CI jobs. Adding one test function does not change CI behaviour — the existing test runner picks it up automatically.

## Platform Considerations

None identified. The platform cross-check in ENVIRONMENT.md §7 is sufficient. This task only modifies Python code that runs in the worker subprocess, which is already tested under both Linux and Windows CI runners.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `MockModel` does not carry a `model_id` attribute, so the test's mock model needs one added | Low | Low | The test constructs its own mock model object with a `model_id` attribute — `MockModel` is just the production mock. The test uses `type("Model", (), {"arch": "zit", "model_id": "test_model"})()` which is independent of `MockModel`. |
| `self.ctx` is not accessible from `sample()` since it's a module-level function, not a method | Medium | High | **This is a real issue.** `sample()` is a standalone function, not a method. It receives `model`, `conditioning`, `latent`, etc. as arguments but NOT `ctx`. The `pipeline_cache` is accessed via `self.ctx` in node `execute()` methods. For `sample()` to access the cache, either: (a) pass `pipeline_cache` as an argument to `sample()`, or (b) the `model` or `conditioning` object carries a reference to the cache. **The plan must be updated** — `sample()` cannot call `self.ctx.pipeline_cache.get_or_load()` directly. The correct approach is to have the Sampler node (P18-D19) pass the pipeline cache into `sample()`. Since this task only assembles the pipeline (doesn't invoke it), and the pipeline cache access happens inside `sample()`, the plan needs to either add a `pipeline_cache` parameter to `sample()` or use a different mechanism. |

**Risk resolution:** The `sample()` function currently has no access to `pipeline_cache`. Looking at the existing codebase pattern in `loader.py`, node `execute()` methods access `self.ctx.pipeline_cache`. The `sample()` function is called by the `Sampler` node. The correct approach for this task is to pass `pipeline_cache` as an additional argument to `sample()`, or to have the `model` object carry a reference to the cache. Since modifying the function signature would affect P18-D18b and P18-D18c (which also call `sample()`), and the task description says "confirm exact field names on the model object from P18-D4/P18-D6 at ACT time", the safest approach is to add `pipeline_cache` as a new argument to `sample()` with a default of `None` (for backward compatibility with mock tests), then use it in the real path. This is a non-breaking change since all existing callers (mock tests) pass positional args and the default handles the case.

Revised approach step 2:
- Add `pipeline_cache` as an optional keyword argument to `sample()`: `def sample(..., pipeline_cache: Any = None)` — this keeps backward compatibility with existing callers.
- Inside the real mode path, use `pipeline_cache.get_or_load(...)` when `pipeline_cache` is not `None`, or fall back to a no-op if `None` (for test isolation).

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_zit.py` exits 0
