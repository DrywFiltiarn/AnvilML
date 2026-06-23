# Plan Report: P18-D18c

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D18c                                    |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/diffusion/zit.py: invoke pipeline with output_type=latent and return result |
| Depends on  | P18-D18a, P18-D18b                          |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T12:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `sample()`'s real sampling path in `worker/nodes/arch/diffusion/zit.py` by replacing the `NotImplementedError` stub (introduced in P18-D18a) with an actual `ZImagePipeline.__call__` invocation. The pipeline was assembled and cached in P18-D18a; the callback adapter was built in P18-D18b. This task wires them together: invoke the pipeline with `output_type="latent"` (raw denoised latent tensor, not decoded image), wrap the call in `try/except _SamplingCancelled` for clean cancellation handling, and return `(result[0], seed)` on success. The acceptance criterion is that the existing mock tests in `worker/tests/test_arch_zit.py` continue to pass and a new real-mode test verifies the pipeline invocation.

## Scope

### In Scope
- Replace the `raise NotImplementedError(...)` at the bottom of `sample()`'s real path with actual `pipeline(...)` invocation.
- Wrap the invocation in `try/except _SamplingCancelled` — re-raise the sentinel on cancellation so it propagates to `worker_main.py`'s exception handler (which sends a Failed event; the Cancelled event was already sent by the `CancelJob` handler).
- Return `(result[0], seed)` on success, matching the function's declared return type `tuple[Any, int]`.
- Update the existing `test_sample_real_assembles_pipeline_via_cache` test to account for the new pipeline call (the mock pipeline's `__call__` must be configured to return a list-like object, since the code now accesses `result[0]`).
- Add a new test `test_sample_real_invokes_pipeline_with_correct_args` that verifies the pipeline is called with the correct keyword arguments (`prompt_embeds`, `negative_prompt_embeds`, `latents`, `num_inference_steps`, `guidance_scale`, `output_type="latent"`, `callback_on_step_end`, `return_dict=False`).
- Update the module docstring to remove the "not yet invoked" language — the pipeline is now invoked.
- Update `__all__` if any new public items are added (none expected; `_SamplingCancelled` and `_make_callback` are already defined in P18-D18b).

### Out of Scope
None. This task implements its full scope with no deferrals — `defers_to: []`. The task context's instruction to "confirm at ACT time" the cancellation convention has been resolved during planning by inspecting `worker_main.py` and `executor.py`.

## Existing Codebase Assessment

**What already exists:** `worker/nodes/arch/diffusion/zit.py` contains the full scaffolding for real-mode sampling: `VAE_SCALE_FACTOR`, `compute_latent_shape()`, `MockLatent`, `can_handle()`, `_SamplingCancelled` (defined in P18-D18b), `_make_callback()` (P18-D18b), and the pipeline assembly logic in `sample()`'s real path (P18-D18a). The only missing piece is the actual `pipeline(...)` invocation — the function raises `NotImplementedError` after assembling the pipeline. The test file `worker/tests/test_arch_zit.py` has 10 tests covering mock mode, can_handle, compute_latent_shape, _make_callback, and the pipeline assembly (which currently expects a NotImplementedError).

**Established patterns:**
- Mock mode is controlled by `os.environ.get("ANVILML_WORKER_MOCK") == "1"`. Tests that need real mode temporarily set the env var and restore it in a `finally` block.
- The `cancel_flag` is a `list[bool]` (mutable container), confirmed in `worker_main.py` line 48: `_cancel_flag: list[bool] = [False]`. The `_make_callback` adapter calls `cancel_flag.is_set()`, but `list` doesn't have `is_set()`. Looking at the existing test (line 162), `cancel_flag=[False]` is passed, and the test on line 461-462 uses `threading.Event()` instead. The `_make_callback` docstring says `cancel_flag` is expected to be a `threading.Event`; the test `test_make_callback_raises_on_cancellation` correctly uses a `threading.Event`. The mock test `test_sample_mock_returns_mock_latent_and_seed` passes `cancel_flag=[False]` which works because the mock path never reaches `_make_callback`.
- Tests use `unittest.mock.MagicMock` for mocking the pipeline cache and pipeline objects.
- The module docstring describes the real callback shape — it currently says "the adapter between them is provided by `_make_callback` in a downstream task" which is now outdated since P18-D18b has already implemented it.
- The `sample()` function's docstring says "invocation is deferred to the downstream task (P18-D18c)" — this needs updating.

**Gap between design doc and source:** The design doc (TASKS_PHASE018.md) describes the pipeline call signature precisely. The source code matches this description. No structural gaps found.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0          | pypi-query MCP | n/a                    |

`diffusers` is already declared in `worker/requirements/base.txt`. The `ZImagePipeline` class and `FlowMatchEulerDiscreteScheduler` are confirmed present in diffusers ≥ 0.36.0 (per task context and design doc). No new dependencies introduced.

## Approach

1. **Update the module docstring** in `worker/nodes/arch/diffusion/zit.py`. Replace the paragraph that says "invocation is deferred to the downstream task (P18-D18c)" with language confirming the pipeline is now invoked in the real path. The callback shape description is already accurate (it mentions `_make_callback`).

2. **Update `sample()`'s docstring**. Change the Returns section from "a denoised latent tensor (real mode, not yet invoked)" to "a denoised latent tensor (real mode)". Remove the "not yet invoked" qualifier. The Raises section should no longer mention `NotImplementedError` — replace it with a description that `_SamplingCancelled` may be raised on cancellation.

3. **Replace the `raise NotImplementedError(...)` stub** at the bottom of `sample()`'s real path (line 335) with actual pipeline invocation:
   - Call `pipeline(prompt_embeds=conditioning.positive, negative_prompt_embeds=conditioning.negative, latents=latent, num_inference_steps=steps, guidance_scale=cfg, output_type="latent", callback_on_step_end=_make_callback(emit_progress, cancel_flag, steps), return_dict=False)`.
   - Wrap the call in `try/except _SamplingCancelled: raise` — re-raise the sentinel so it propagates to `worker_main.py`'s exception handler. The `CancelJob` handler already sends a Cancelled event; this exception propagates as a Failed event with the cancellation reason in the error message.
   - On success (no exception), return `(result[0], seed)`.

4. **Update `test_sample_real_assembles_pipeline_via_cache`**. The existing test mocks `pipeline_cache.get_or_load()` to return a `MagicMock()` pipeline object. After this change, the test will call `pipeline(...)` on that mock, which returns another `MagicMock`. The test then checks `NotImplementedError` — update it to expect the function to return normally (no exception) and verify the return value structure. Specifically:
   - Configure `mock_cache.get_or_load.return_value.__call__` to return `[MagicMock(), seed]` (a list with the latent result and seed, matching `return_dict=False`'s output format).
   - Remove the `pytest.raises(NotImplementedError)` context manager.
   - Assert the returned tuple has the correct structure: `result[0]` is the latent, `result[1]` equals the input seed.
   - Keep the `get_or_load.assert_called_once()` assertion and the cache key check.

5. **Add `test_sample_real_invokes_pipeline_with_correct_args`**. This test verifies the pipeline is called with all expected keyword arguments:
   - Force real mode by setting `ANVILML_WORKER_MOCK="0"`.
   - Create a mock pipeline cache that returns a mock pipeline object.
   - Build a mock model with `arch="zit"`, `model_id="test_model"`.
   - Build a conditioning object with `positive`, `negative`, `tokenizer`, and `text_encoder` attributes.
   - Build a mock VAE and a `threading.Event()` as cancel_flag.
   - Call `sample()` with all arguments.
   - Assert the mock pipeline's `__call__` was called with `output_type="latent"` and `return_dict=False`.
   - Assert `num_inference_steps=steps`, `guidance_scale=cfg` are passed correctly.
   - Assert `callback_on_step_end` is a callable (the `_make_callback` adapter).
   - Assert the returned tuple has the correct structure.
   - Restore `ANVILML_WORKER_MOCK` in a `finally` block.

6. **Verify the existing mock tests still pass.** The mock path (`if _mock: return (MockLatent(), seed)`) is unchanged — it returns before reaching any real-mode code. All existing mock tests should pass without modification.

## Public API Surface

No new public items are introduced. The only change is to the existing `sample()` function's implementation — its signature and return type remain `tuple[Any, int]`. The `_SamplingCancelled` exception and `_make_callback` function were already added in P18-D18b and are already in `__all__`-adjacent scope (underscore-prefixed, not exported).

| Action | Item | Module Path |
|--------|------|-------------|
| MODIFY | `def sample(...) -> tuple[Any, int]` | `worker.nodes.arch.diffusion.zit` |
| MODIFY | Module docstring | `worker.nodes.arch.diffusion.zit` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Replace NotImplementedError stub with pipeline invocation; update docstrings |
| MODIFY | `worker/tests/test_arch_zit.py` | Update existing real-mode test for pipeline call; add new test for argument verification |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_sample_mock_returns_mock_latent_and_seed` | Mock path returns `(MockLatent(), seed)` without touching real-mode code | `ANVILML_WORKER_MOCK=1` (conftest autouse) | seed=42, all other args None/empty | `isinstance(result[0], MockLatent)` and `result[1] == 42` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_preserves_seed_value` | Mock path preserves exact seed for multiple values | `ANVILML_WORKER_MOCK=1` | seeds: 0, 1, 2^32-1, 12345 | Each seed returned unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_real_assembles_pipeline_via_cache` | Pipeline cache get_or_load called with correct key; pipeline invoked returns `(latent, seed)` | `ANVILML_WORKER_MOCK="0"` (temporarily set) | mock model with `model_id="test_model"`, mock pipeline_cache | `get_or_load` called with `:pipeline` key; returns `(mock_latent, 42)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_real_invokes_pipeline_with_correct_args` (NEW) | Pipeline called with correct keyword arguments including `output_type="latent"`, `return_dict=False` | `ANVILML_WORKER_MOCK="0"` (temporarily set) | mock model, conditioning, vae, pipeline_cache, cancel_flag | Pipeline `__call__` called with `output_type="latent"`, `return_dict=False`, correct steps/cfg | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_make_callback_emits_progress` | `_make_callback` adapter emits progress per step | `ANVILML_WORKER_MOCK=1` | total_steps=4, i=0 | `emit_progress(0, 4)` called; returns `{}` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_make_callback_emits_progress -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_make_callback_raises_on_cancellation` | `_make_callback` raises `_SamplingCancelled` when flag set | `ANVILML_WORKER_MOCK=1` | total_steps=4, i=2, cancel_flag set | `_SamplingCancelled` raised; `emit_progress(2, 4)` called first | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_make_callback_raises_on_cancellation -v` exits 0 |

## CI Impact

No CI changes required. The modified test file (`worker/tests/test_arch_zit.py`) is already included in the `worker-linux` and `worker-windows` CI jobs via `pytest worker/tests/`. No new test file, no new CI gate.

## Platform Considerations

None identified. The change is purely Python-level — no `#[cfg(unix)]` / `#[cfg(windows)]` guards, no platform-specific path handling, no line-ending concerns. The `ZImagePipeline.__call__` API is the same on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ZImagePipeline.__call__`'s return format with `return_dict=False` may not be `[latent, seed]` — diffusers pipelines sometimes return just the latent or a tuple in different orders. | Medium | High | Verify against diffusers 0.38.0 source code at ACT time. If the format differs, adjust the unpacking. The existing test with a mock pipeline will catch this immediately since `result[0]` would fail on wrong format. |
| `conditioning.positive` / `conditioning.negative` attribute names may not match what P18-D16's conditioning object exposes. | Medium | High | Confirm at ACT time by reading the conditioning object produced by P18-D16's `ClipTextEncode`. If attribute names differ, adjust the attribute access. The mock conditioning in tests uses these exact names. |
| `_SamplingCancelled` re-raise causes a "Failed" event to be sent in addition to the "Cancelled" event already sent by the CancelJob handler. | Low | Medium | This is the established convention in worker_main.py — the CancelJob handler sends Cancelled, then the exception propagates and sends Failed. The scheduler handles both events. If this is undesirable, the ACT agent should confirm with the Rust scheduler's Cancelled/Failed handling before writing code. |
| The mock test `test_sample_real_assembles_pipeline_via_cache` uses `pytest.raises(NotImplementedError)` — after removing the stub, the test structure needs careful update to avoid accidentally suppressing real errors. | Low | Medium | Replace the `pytest.raises` context manager with direct assertion of return value. The mock pipeline's `__call__` must be configured to return a list-like object `[MagicMock(), seed]` to match the real pipeline's `return_dict=False` output. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch.diffusion.zit import sample; print('import ok')"` exits 0
- [ ] `head -1 .forge/reports/P18-D18c_plan.md` prints `# Plan Report: P18-D18c`
- [ ] `grep "^## " .forge/reports/P18-D18c_plan.md` shows 12 headings
- [ ] `wc -l .forge/reports/P18-D18c_plan.md` shows > 40 lines
