# Plan Report: P21-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-C2                                      |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/sampler.py: Sampler real branch dispatches to arch module |
| Depends on  | P21-C1, P21-B2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-07-14T18:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `Sampler` node's `execute()` method by replacing the `NotImplementedError` stub (introduced in P21-C1) with a real branch that dispatches to `arch.diffusion.get_module(model.arch).sample()`, passing the model, conditioning, latent, steps, cfg, and seed through to `zit.py`'s already-implemented `sample()` function. The real branch returns a denoised latent tensor and resolved seed. Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers on `execute()` are updated to name collectible, passing tests from this same task.

## Scope

### In Scope
- Replace the real branch body of `Sampler.execute()` in `worker/nodes/sampler.py`: dispatch to `arch.diffusion.get_module(model.arch).sample(...)` and return `(denoised_latent, resolved_seed)`.
- Update the `REAL_PATH_VERIFIED` marker comment on `execute()` to name the new real-mode test (not the old "raises NotImplementedError" stub).
- Update the `MOCK_PATH_VERIFIED` marker comment to confirm it still names the existing mock-mode test.
- Add real-mode tests in `worker/tests/test_nodes_sampler.py` exercising the Sampler's real branch against the ZiT fixture checkpoint loaded by P20 (P21-B2's `sample()` already exists).
- Update the docstring on `execute()` to reflect the completed real branch.
- Update module-level docstring to remove the "deferred to P21-C2" reference.

### Out of Scope
None. The `defers_to` field is empty (`[]` from JSON). All functionality described in the task context is implemented here. No stubs, no `NotImplementedError`, no deferred work.

## Existing Codebase Assessment

**What exists:** `worker/nodes/sampler.py` contains the `Sampler` node class (created in P21-C1) with a working mock branch and a real branch that raises `NotImplementedError`. The mock branch returns a sentinel dict `{"mock": True, "shape": ...}` with a deterministic seed resolution (`-1` → `0`). The `@register` decorator already registers `Sampler` in `NODE_REGISTRY`. The existing test file `test_nodes_sampler.py` has 5 tests: 3 mock-mode tests (class attributes, mock returns, mock seed) and 1 real-mode test (raises NotImplementedError) plus 1 subprocess registry test.

**Established patterns:**
- `LoadModel` (the only other arch-dispatching node) imports `get_module` from `worker.nodes.arch.diffusion` inside the real branch, calls `get_module("zit")`, and wraps the result in `pipeline_cache.get_or_load()`. This same pattern applies to `Sampler`, except the model is already loaded (passed as an input) so no caching is needed.
- `NodeContext` has `.mock`, `.caps`, `.device`, `.emit`, `.pipeline_cache`, `.cancel_flag`, and `.job_id`. The mock branch checks `ctx.mock` at the top of `execute()`.
- Real-mode tests use `worker/tests/fixtures/zit_tiny.safetensors` (the ZiT fixture from P20). Tests load the model via `zit.load()`, run `zit.sample()`, and assert output shape/seed.
- Dual-mode parity markers are placed as `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` comments immediately above the function definition.

**Gap between design doc and current source:** The design doc (§10.4) specifies that `Sampler` calls `diffusion.get_module(model.arch).sample(...)`. The current `sampler.py` has a stub real branch. `zit.py`'s `sample()` signature is `sample(model, model_id, conditioning, latent, steps, cfg, seed) -> (tensor, int)`. The model's `.arch` attribute is `"zit"` (set in `ZiTModel.__init__`). The `model_id` for the Sampler node's real branch should be derived from the model object — since the model is already loaded (not a path string), we use the model's own identity. Looking at how `LoadModel` passes `model_id`, the Sampler node receives the model object directly, so `model_id` can be derived from the model's identity or a stable identifier. The most practical approach: since the model is a `ZiTModel` instance with `.arch = "zit"`, and the `sample()` function needs a `model_id` for cache keying, we use a string representation of the model (e.g., `str(id(model))` or `"sampler_model"` as a per-job unique key). Actually, looking more carefully at the `sample()` function — it caches pipelines under `f"{model_id}:pipeline"`. For the Sampler node, each execution is a single job step, so we can use `f"job_{ctx.job_id}"` as the model_id to keep cache keys scoped to the current job.

## Resolved Dependencies

None. This task modifies only Python source files within the project. No new external packages or crates are introduced. The `arch.diffusion.get_module()` and `zit.sample()` functions are already implemented in prior tasks (P20-B2 and P21-B2 respectively). All imports are from project-internal modules (`worker.nodes.arch.diffusion`, `worker.nodes.base`) or the Python standard library (`threading`).

## Approach

**Step 1 — Modify `worker/nodes/sampler.py`:**

Replace the real branch of `Sampler.execute()` (lines 86–93) with the following logic:

```python
else:
    # Real branch: dispatch to the registered diffusion arch module.
    # model.arch is "zit" (set by ZiTModel.__init__), which routes
    # get_module() to zit.py. The sample() function handles pipeline
    # assembly, denoising, and seed resolution internally.
    from worker.nodes.arch.diffusion import get_module

    module = get_module(inputs["model"].arch)
    if module is None:
        raise RuntimeError(
            f"no diffusion arch module registered for "
            f"'{inputs['model'].arch}'; cannot sample"
        )

    denoised_latent, resolved_seed = module.sample(
        inputs["model"],
        f"job_{ctx.job_id}",
        inputs["conditioning"],
        inputs["latent"],
        inputs["steps"],
        inputs["cfg"],
        inputs["seed"],
    )

    return {"latent": denoised_latent, "seed": resolved_seed}
```

Key decisions:
- `model_id` for the cache key is `f"job_{ctx.job_id}"` — each job gets its own pipeline cache namespace, preventing cross-job cache pollution. This is consistent with the per-job execution model.
- The import of `get_module` is inside the real branch (same pattern as `LoadModel`) to avoid importing torch/diffusers in mock-mode tests.
- The `model` input is the raw `ZiTModel` (a `torch.nn.Module`) in real mode — not a dict. We access `.arch` directly.
- `conditioning` is passed through as-is (may be `None` for unconditional generation).
- `latent` is passed through as-is — `sample()` clones it internally.

Update the `REAL_PATH_VERIFIED` marker to:
```python
# REAL_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture
```

Update the `MOCK_PATH_VERIFIED` marker to (confirming it still names the existing test):
```python
# MOCK_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape
```

Update the module docstring to remove "deferred to P21-C2" language.

Update the `execute()` docstring to remove the `NotImplementedError` raise section and describe the real branch's return value.

**Step 2 — Add real-mode tests in `worker/tests/test_nodes_sampler.py`:**

Add the following tests (each with `@pytest.mark.real_mode` decorator):

1. `test_sampler_real_denoises_zit_fixture` — The canonical real-mode test. Loads the ZiT fixture via `zit.load()`, calls `Sampler.execute()` with the loaded model, conditioning (None), a noise latent tensor, steps=20, cfg=7.5, seed=42. Asserts the returned latent is a `torch.Tensor` with the same shape as the input, and the returned seed equals 42.

2. `test_sampler_real_seed_minus_one_resolves` — Calls `Sampler.execute()` with seed=-1. Asserts the returned seed is a non-negative integer in `[0, 2**63)`. This verifies the real branch correctly delegates seed resolution to `zit.sample()`.

3. `test_sampler_real_explicit_seed_unchanged` — Calls with seed=42. Asserts returned seed == 42.

4. `test_sampler_real_multiple_steps` — Calls with steps=10. Asserts output shape matches input shape.

5. `test_sampler_real_cfg_one_is_conditional_only` — Calls with cfg=1.0. Asserts output is a tensor (not an error). This is a basic smoke test for the CFG path.

6. `test_sampler_real_latent_shape_preserved` — Calls with a latent of shape (1, 4, 8, 8). Asserts the output tensor has shape (1, 4, 8, 8).

Each test follows the pattern established in `test_arch_zit.py::test_sample_denoising_real_zit_fixture`: load fixture → call sample → assert shape and seed → clean up pipeline cache.

## Public API Surface

No new public items are introduced. The task modifies an existing public method (`Sampler.execute()`) in-place. The signature remains:

```python
def execute(self, ctx: NodeContext, **inputs) -> dict:
```

The return value changes from `{"latent": {"mock": True, ...}, "seed": int}` (mock) or `NotImplementedError` (real stub) to `{"latent": torch.Tensor, "seed": int}` (real).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/sampler.py` | Replace real branch stub with arch module dispatch; update markers and docstrings |
| Modify | `worker/tests/test_nodes_sampler.py` | Add ≥5 real-mode tests exercising the Sampler's real branch |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_nodes_sampler.py` | `test_sampler_real_denoises_zit_fixture (real)` | End-to-end: load ZiT fixture, execute Sampler with seed=42, assert denoised latent shape and seed | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_real_seed_minus_one_resolves (real)` | seed=-1 resolves to non-negative int in [0, 2^63) | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_real_explicit_seed_unchanged (real)` | seed=42 passes through unchanged | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_real_multiple_steps (real)` | steps=10 produces correct output shape | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_multiple_steps -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_real_cfg_one_conditional_only (real)` | cfg=1.0 (no guidance) works correctly | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_cfg_one_conditional_only -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_real_latent_shape_preserved (real)` | Output tensor shape matches input latent shape | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved -v -m real_mode` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_mock_returns_expected_shape` | Existing mock test still passes (marker confirmation) | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape -v` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_mock_seed_zero` | Existing mock seed test still passes | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_seed_zero -v` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_class_attributes` | Existing class attr test still passes | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_class_attributes -v` exits 0 |
| `test_nodes_sampler.py` | `test_sampler_in_registry` | Existing subprocess registry test still passes | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_in_registry -v` exits 0 |

Total: ≥10 tests in file (≥5 real-mode + 4 existing mock + 1 registry).

## CI Impact

The `worker-linux-mock` and `worker-windows-mock` CI jobs run `pytest worker/tests -v -m "not real_mode"` — the new real-mode tests are gated behind `@pytest.mark.real_mode` and will not be collected by mock-mode CI. The `worker-linux-real` and `worker-windows-real` CI jobs run `pytest worker/tests -v -m real_mode` (mock unset) — these will collect and run all 6 new real-mode tests plus the existing real-mode test (which now exercises the real branch instead of raising NotImplementedError). No CI workflow files are modified.

## Platform Considerations

None identified. The real branch dispatches to `zit.py`'s `sample()` which runs entirely on the target device (CPU in CI/agent VM). No `#[cfg(...)]` guards or platform-specific code paths are introduced. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `inputs["model"]` is a dict in real mode (mock data leaked) | Low | High | The test constructs `NodeContext(mock=False)` and passes a real `ZiTModel` object. In production, the executor passes the raw model from `LoadModel`'s output. Add an assert in the test that `inputs["model"]` is a `torch.nn.Module` instance. |
| `get_module(model.arch)` returns `None` if no module is registered | Low | High | Follow the `LoadModel` pattern: check for `None` and raise `RuntimeError` with a descriptive message. The zit module is imported in `diffusion/__init__.py` (P20-B2), so this should never happen in normal operation. |
| `sample()`'s pipeline cache grows unbounded across jobs | Medium | Medium | Use `f"job_{ctx.job_id}"` as the model_id so each job gets a unique cache namespace. The `PipelineCache` LRU eviction (max 4 entries) will eventually evict old job pipelines. Document this in a code comment. |
| Test fixture `zit_tiny.safetensors` doesn't load correctly on CPU | Low | High | The fixture was already validated by P20's real-mode tests (`test_sample_denoising_real_zit_fixture`). Use the exact same load pattern from that test. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_nodes_sampler.py -v -m real_mode` exits 0 (≥6 real-mode tests)
- [ ] `python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 (≥8 total tests in file)
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/sampler.py` returns a test name that resolves: `worker/.venv/bin/python -m pytest --collect-only "<named test>" -q` exits 0
- [ ] `grep "MOCK_PATH_VERIFIED:" worker/nodes/sampler.py` returns a test name that resolves: `worker/.venv/bin/python -m pytest --collect-only "<named test>" -q` exits 0
- [ ] `grep -n "NotImplementedError" worker/nodes/sampler.py` returns no matches (no stubs remain)
- [ ] `grep -n "deferred to P21-C2" worker/nodes/sampler.py` returns no matches (no deferral references remain)
