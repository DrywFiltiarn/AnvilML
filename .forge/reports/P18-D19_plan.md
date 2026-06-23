# Plan Report: P18-D19

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D19                                           |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/sampler.py: Sampler real dispatch to arch.get_module().sample() |
| Depends on  | P18-D18c                                          |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T14:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace `Sampler.execute()`'s stubbed real-path `NotImplementedError` with actual architecture-module dispatch: resolve the matching arch module via `arch.get_module(model)`, raise `ValueError("unsupported model architecture")` if none matches, build a 2-argument `emit_progress(step, total)` callable that wraps `ctx.emit` into a `Progress` event dict, call `mod.sample()` with all required arguments, and return `{"latent": result[0], "seed": result[1]}`. Existing mock-mode tests in `worker/tests/test_nodes_sampler.py` must continue to pass unchanged.

## Scope

### In Scope
- Modify `worker/nodes/sampler.py`: replace `Sampler.execute()`'s real-path stub with `arch.get_module(model)` dispatch to `mod.sample()`.
- Build the `emit_progress(step, total)` wrapper callable that converts 2-arg calls into `ctx.emit({"_type": "Progress", "job_id": ..., "step": ..., "total_steps": ..., "preview_b64": None})`.
- Add inline comment at the dispatch site explaining the architecture dispatch contract.
- Fix the existing TODO comment `TODO(P18-C1)` which is misattributed (P18-C1 is `pipeline_cache.py`).
- All existing mock tests in `worker/tests/test_nodes_sampler.py` continue to pass.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. No functionality is deferred.

## Existing Codebase Assessment

The `Sampler` node in `worker/nodes/sampler.py` already has a fully implemented mock path (lines 282–298) that returns `MockLatent` and the resolved seed. The real path (lines 300–309) is a stub raising `NotImplementedError` with a misattributed `TODO(P18-C1)` comment. The `EmptyLatent` node in the same file already has a complete real path (lines 142–184) that demonstrates the established pattern: call `arch.get_module(model)`, check for `None`, dispatch to the module, and handle errors with `ValueError`.

The `arch.get_module()` function (in `worker/nodes/arch/diffusion/__init__.py`) iterates loaded diffusion arch modules, calls their `can_handle(model_obj)`, and returns the first matching module or `None`. The `arch` re-export shim at `worker/nodes/arch/__init__.py` imports `can_handle` and `get_module` from the `diffusion` subpackage.

The `Sampler.sample()` function in `worker/nodes/arch/diffusion/zit.py` (lines 212–358) has a complete signature: `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None) -> tuple[Any, int]`. It already handles mock mode and real mode (assembling `ZImagePipeline`, invoking it, returning `(result[0], seed)`). The `_make_callback()` adapter bridges `diffusers`' 4-arg `callback_on_step_end` to the 2-arg `emit_progress(step, total)` shape.

`ctx.emit` is set to `send_event` from `worker.ipc` in `worker_main.py` (line 243). The `send_event` function accepts a dict and serialises it via msgpack. The `Progress` event format is shown in `executor.py` (lines 222–228): `{"_type": "Progress", "job_id": ctx.job_id, "step": step, "total_steps": total, "preview_b64": None}`.

There is no `NodeError` class defined in the Python worker codebase. The `EmptyLatent` node uses `ValueError` for unsupported architecture errors (line 159–161), which is the pattern to follow here.

The `ctx.cancel_flag` is a `list[bool]` (see `worker_main.py` line 238: `_cancel_flag[0] = False`), not a `threading.Event`. However, the `sample()` function in `zit.py` calls `cancel_flag.is_set()` (line 112), which expects a `threading.Event`. This is a known mismatch — the task context says to "confirm exact ctx.emit Progress-event call signature via worker/nodes/base.py and worker_main.py at ACT time". At ACT time, the plan must note that `cancel_flag` is a `list[bool]` in the current codebase, while `sample()` expects a `threading.Event`. The ACT agent should confirm whether this needs a wrapper or if `cancel_flag` has been updated by a prerequisite task (P18-D18c or P903-A2).

## Resolved Dependencies

None. This task introduces no new Python packages or external crates. It only modifies existing code paths within the `worker/nodes/sampler.py` module, calling into already-imported modules (`worker.nodes.arch`, `worker.nodes.base`).

## Approach

1. **Replace the TODO comment and `NotImplementedError` stub** in `Sampler.execute()` (lines 300–309). Remove the misattributed `TODO(P18-C1)` comment (P18-C1 is `pipeline_cache.py`, never touched `sampler.py`) and the `raise NotImplementedError(...)` call.

2. **Build the `emit_progress(step, total)` wrapper callable.** Create a local function inside `execute()` that accepts `(step, total)` and calls `self.ctx.emit()` with a `Progress` event dict:
   ```python
   def emit_progress(step: int, total: int) -> None:
       self.ctx.emit({
           "_type": "Progress",
           "job_id": self.ctx.job_id,
           "step": step,
           "total_steps": total,
           "preview_b64": None,
       })
   ```
   This matches the `Progress` event shape used by `executor.py` (lines 222–228) and is the format `send_event` in `worker.ipc` expects.

3. **Dispatch to the architecture module.** After the mock-mode guard, call:
   ```python
   mod = arch.get_module(model)
   ```
   If `mod is None`, raise `ValueError("unsupported model architecture")` — this matches the `EmptyLatent` pattern (line 159–161) and the ANVILML_DESIGN.md §10.4 contract.

4. **Call `mod.sample()` with all required arguments.** Pass:
   - `model`, `conditioning`, `latent`, `steps`, `cfg`, `seed` — from the inputs
   - `ctx.device` — from the node context
   - `ctx.cancel_flag` — from the node context (a `list[bool]`; the `sample()` function in `zit.py` calls `cancel_flag.is_set()` which expects a `threading.Event` — this is a known API shape mismatch that the ACT agent must confirm at session start)
   - `emit_progress` — the wrapper built in step 2
   - `vae=None` — keyword-only default (the task context does not pass VAE; the `sample()` function has `vae=None` as default)
   - `pipeline_cache=self.ctx.pipeline_cache` — keyword-only argument from the node context

   ```python
   result = mod.sample(
       model, conditioning, latent, steps, cfg, seed,
       ctx.device, ctx.cancel_flag, emit_progress,
       pipeline_cache=self.ctx.pipeline_cache,
   )
   ```

5. **Return the result.** Extract `result[0]` as the latent and `result[1]` as the seed:
   ```python
   return {"latent": result[0], "seed": result[1]}
   ```

6. **Add inline comment at the dispatch site.** A single `#` comment explaining that `arch.get_module()` scans loaded arch modules and returns the matching one, or `None` if no module claims the model architecture — in which case `ValueError` is raised per the dispatch contract.

7. **Fix the docstring.** Update the `Sampler.execute()` docstring's `Raises` section: remove the reference to `NotImplementedError` and the P18-C1 stub note; replace with `ValueError` for unsupported architecture.

## Public API Surface

No new public items are introduced. The only change is to the existing private implementation of `Sampler.execute()` (an `@abstractmethod` implementation from `BaseNode`). No `pub`/`def` signatures change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/sampler.py` | Replace `Sampler.execute()`'s real-path stub with `arch.get_module()` dispatch to `mod.sample()`; fix misattributed TODO; update docstring |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_sampler.py` | All existing tests | Mock-mode execution continues to work — `test_sampler_execute_returns_mock_latent_and_seed`, `test_sampler_seed_negative_one_resolves_to_random`, `test_sampler_registered_in_registry`, `test_sampler_emits_progress_flag`, `test_sampler_metadata_attributes` — all must pass unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 |

No new tests are added. The task only modifies the real-path code (which is unreachable in mock mode), so existing mock tests serve as the acceptance gate. The mock tests verify that the mock path is untouched and the node metadata remains correct.

## CI Impact

No CI changes required. The task modifies only `worker/nodes/sampler.py` (a Python source file) and does not add new test files, new CI configuration, or change any build/lint/formatter settings. The existing `worker-linux` and `worker-windows` CI jobs pick up the modified file via `py_compile` and `pytest worker/tests/`.

## Platform Considerations

None identified. The change is a pure Python logic modification — no platform-specific code paths, no file I/O, no socket operations. The `ctx.device` string is passed through unchanged. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ctx.cancel_flag` is a `list[bool]` (see `worker_main.py` line 238) but `sample()` in `zit.py` calls `cancel_flag.is_set()` (line 112), which expects a `threading.Event`. This API shape mismatch means the real path will raise `AttributeError` at runtime. | High | High | Confirm at ACT time whether P18-D18c or a prerequisite task updated `cancel_flag` to a `threading.Event`. If not, the ACT agent must either: (a) wrap `ctx.cancel_flag` in a `threading.Event` before passing it, or (b) pass `ctx.cancel_flag` as-is and let the real-mode path handle the mismatch. Document the resolution in the plan's Deviations section. |
| The `sample()` function has keyword-only parameters `vae=None` and `pipeline_cache=None`. The task context does not explicitly mention passing `pipeline_cache`, but the `sample()` implementation calls `pipeline_cache.get_or_load()` which requires it. Omitting it will cause `TypeError: get_or_load() missing 1 required positional argument`. | Medium | High | Pass `pipeline_cache=self.ctx.pipeline_cache` as a keyword argument. The `self.ctx` attribute is available on every node instance (set in `BaseNode.__init__`). Confirm at ACT time that `self.ctx.pipeline_cache` is a `PipelineCache` instance (P903-A2 guarantees this). |
| Importing `arch` inside the real path branch vs. at module top level. The module already imports `from worker.nodes import arch` at the top level (line 28), so no additional imports are needed. | Low | Low | No action needed — the import already exists. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/sampler.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 (all existing tests pass)
- [ ] `grep -n "NotImplementedError" worker/nodes/sampler.py` returns no matches (the stub is fully removed)
- [ ] `grep -n "TODO(P18-C1)" worker/nodes/sampler.py` returns no matches (the misattributed TODO is removed)
- [ ] `grep -n "arch.get_module" worker/nodes/sampler.py` returns exactly 1 match (the dispatch call)
