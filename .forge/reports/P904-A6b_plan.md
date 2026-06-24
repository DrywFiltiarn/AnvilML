# Plan Report: P904-A6b

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A6b                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py: remove vestigial vae parameter from sample()/loader_fn |
| Depends on  | P904-A6                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T00:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Remove the vestigial `vae: Any = None` parameter from `sample()` in `worker/nodes/arch/diffusion/zit.py` and the corresponding `vae=vae` line in `loader_fn`'s `ZImagePipeline(...)` construction. This parameter was never used by design — `Sampler` must never receive or forward a VAE component; `VaeDecode` (P18-D20) is its sole owner. The task also updates two test call sites in `test_arch_zit.py` and their corresponding catalogue entries in `docs/TESTS.md` to remove the now-invalid `vae=` kwargs.

## Scope

### In Scope
- Remove `vae: Any = None` from `sample()`'s parameter list in `worker/nodes/arch/diffusion/zit.py`
- Remove the `vae=vae` line from `loader_fn`'s `ZImagePipeline(...)` construction in the same file
- Remove `vae=None` keyword argument from `test_sample_real_assembles_pipeline_via_cache` call site in `worker/tests/test_arch_zit.py`
- Remove `vae=mock_vae` keyword argument from `test_sample_real_invokes_pipeline_with_correct_args` call site in `worker/tests/test_arch_zit.py`
- Update the corresponding `docs/TESTS.md` catalogue entries to remove references to the `vae` parameter

### Out of Scope
None. This task has `defers_to: []` (absent from JSON) and must implement its full scope. No stubs, no deferred functionality.

## Existing Codebase Assessment

The `sample()` function in `zit.py` (line 212) currently accepts `vae: Any = None` as a named parameter (line 223), positioned between `emit_progress` and the keyword-only separator `*`. Inside the real-mode path, `loader_fn` (line 302) constructs a `ZImagePipeline` with `vae=vae` (line 323). The `Sampler` node in `sampler.py` does **not** pass `vae` to `mod.sample()` — it passes `model, conditioning, clip, latent, steps, cfg, seed, self.ctx.device, self.ctx.cancel_flag, emit_progress, pipeline_cache=self.ctx.pipeline_cache` (lines 335-338). This confirms `vae` is vestigial: it is accepted by `sample()` but never forwarded by its only caller.

The `diffusers` 0.38.0 `ZImagePipeline.__init__` tolerates `vae=None` via `register_modules`, and `__call__` never dereferences `self.vae` when `output_type="latent"` — the VAE is only used at final decode (~line 583), unreachable from `Sampler`. This was confirmed by the task context and verified via MCP (diffusers 0.38.0 is the current stable version on PyPI).

Established patterns:
- Python docstrings use Google style with Args/Returns/Raises sections.
- Mock-mode tests override `ANVILML_WORKER_MOCK` temporarily with capture-and-restore in try/finally blocks.
- Test catalogue entries in `docs/TESTS.md` follow a structured format with File, Context, Tests, Inputs, Expected output, and Acceptance command fields.
- The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for every test; real-mode tests override it.

No gap between design doc and current source: `ANVILML_DESIGN.md §10.4` was pre-updated by a human to reflect the correct contract (no `vae` on `sample()`), and this task's code change brings the implementation into line with the doc.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0        | pypi-query MCP | n/a                    |

The task context specified diffusers 0.38.0; the MCP result confirms 0.38.0 is the current latest stable version on PyPI. No version override needed. The `ZImagePipeline.__init__` signature and `register_modules` tolerance of `None` were confirmed against the published package metadata.

## Approach

1. **Read the current source to confirm exact line positions.** Open `worker/nodes/arch/diffusion/zit.py` and locate:
   - Line 223: `vae: Any = None,` in `sample()`'s signature — remove this line entirely.
   - Line 323: `vae=vae,` in `ZImagePipeline(...)` call — remove this line entirely.

2. **Remove `vae` from `sample()`'s signature.** Edit `worker/nodes/arch/diffusion/zit.py`:
   - Delete the line `vae: Any = None,` (currently between `emit_progress: Callable[[int, int], None] | None = None,` and `*,`).
   - The keyword-only separator `*,` remains on its line. The parameter immediately before it (`emit_progress`) now becomes the last positional parameter.
   - Update the docstring's `Args:` section: remove the `vae:` bullet (currently between `emit_progress:` and `pipeline_cache:`). The `vae` bullet reads: "vae: The VAE component used by the pipeline. Passed by the calling node; ``None`` in mock-mode tests."

3. **Remove `vae=vae` from `loader_fn`'s `ZImagePipeline(...)` call.** Edit `worker/nodes/arch/diffusion/zit.py`:
   - Delete the line `vae=vae,` (currently between `scheduler=scheduler,` and `text_encoder=text_encoder,`).
   - The remaining keyword arguments (`scheduler`, `text_encoder`, `tokenizer`, `transformer`) stay in their current order.

4. **Update `test_sample_real_assembles_pipeline_via_cache` call site.** Edit `worker/tests/test_arch_zit.py`:
   - Remove the line `vae=None,` from the `sample(...)` call (currently between `emit_progress=lambda step, total: None,` and `pipeline_cache=mock_cache,`).

5. **Update `test_sample_real_invokes_pipeline_with_correct_args` call site.** Edit `worker/tests/test_arch_zit.py`:
   - Remove the line `vae=mock_vae,` from the `sample(...)` call (currently between `emit_progress=lambda step, total: None,` and `pipeline_cache=mock_cache,`).
   - Remove the `mock_vae = MagicMock()` line (line 358) since it is no longer used — it was only created to pass to `sample()`.

6. **Update `docs/TESTS.md` catalogue entries.** Edit `docs/TESTS.md`:
   - For `test_sample_real_assembles_pipeline_via_cache` (line ~3433): Remove the `vae` reference from the Inputs line. The current Inputs line reads: `model_id="test_model"`, `steps=4`, `seed=42`, `cfg=7.0`, `device="cpu"`, `clip=mock_clip`. No `vae` is listed here already — but the Context line (line 3436) does not mention `vae` either. The entry is already clean of `vae` references. No change needed if no `vae` text is present.
   - For `test_sample_real_invokes_pipeline_with_correct_args` (line ~3442): The Context line (3445) mentions "mock VAE" — update to remove this reference. The Inputs line (3447) does not list `vae` — but the Context mentions "mock VAE" in the test description. Update the Context to remove the mention of mock VAE. The Inputs line mentions `clip=mock_clip` but not `vae`.

7. **Run Python syntax check** to confirm no syntax errors were introduced:
   ```bash
   worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py worker/tests/test_arch_zit.py
   ```

8. **Run the affected test module** to confirm all tests pass:
   ```bash
   ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
   ```

## Public API Surface

| Change | Path | Before | After |
|--------|------|--------|-------|
| Modify | `worker/nodes/arch/diffusion/zit.py:sample()` | `def sample(model, conditioning, clip=None, latent=None, steps=4, cfg=7.0, seed=42, device="cpu", cancel_flag=None, emit_progress=None, vae=None, *, pipeline_cache=None)` | `def sample(model, conditioning, clip=None, latent=None, steps=4, cfg=7.0, seed=42, device="cpu", cancel_flag=None, emit_progress=None, *, pipeline_cache=None)` |

No new public items are introduced. No `pub` items exist in Python code (no Rust crate affected).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Remove `vae: Any = None` from `sample()` signature and `vae=vae` from `ZImagePipeline(...)` call; update docstring |
| Modify | `worker/tests/test_arch_zit.py` | Remove `vae=None` and `vae=mock_vae` kwargs from two `sample()` call sites; remove unused `mock_vae` variable |
| Modify | `docs/TESTS.md` | Update catalogue entries for `test_sample_real_assembles_pipeline_via_cache` and `test_sample_real_invokes_pipeline_with_correct_args` to remove `vae` references |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_sample_mock_returns_mock_latent_and_seed` | `sample()` returns `(MockLatent(), seed)` in mock mode — unchanged by this task, verifies no regression to core mock path | `ANVILML_WORKER_MOCK=1` (autouse) | `sample(model=None, conditioning=None, ...)` | `MockLatent` instance and seed == 42 | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_preserves_seed_value` | `sample()` returns the exact seed passed in mock mode — unchanged by this task | `ANVILML_WORKER_MOCK=1` (autouse) | Multiple seed values (0, 1, 2**32-1, 12345) | Each seed returned unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_real_assembles_pipeline_via_cache` | `sample()` calls `pipeline_cache.get_or_load()` with correct key in real mode — updated to remove `vae=None` kwarg | `ANVILML_WORKER_MOCK="0"` (temporarily set), mock pipeline cache | `model_id="test_model"`, `steps=4`, `seed=42`, `clip=mock_clip` (no `vae`) | `get_or_load` called with `:pipeline` key; returns `(latent_result, 42)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_real_invokes_pipeline_with_correct_args` | `sample()` invokes pipeline with all expected kwargs — updated to remove `vae=mock_vae` kwarg and unused `mock_vae` variable | `ANVILML_WORKER_MOCK="0"` (temporarily set), mock pipeline cache | `model_id="test_model"`, `steps=8`, `cfg=7.5`, `seed=99`, `clip=mock_clip` (no `vae`) | Pipeline called with `output_type="latent"`, `return_dict=False`, correct steps/cfg; returns `(latent_result, 99)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_no_torch_import` | Module imports cleanly without torch in mock mode — unchanged by this task | `ANVILML_WORKER_MOCK=1` (autouse) | Fresh import after removing torch from sys.modules | `torch` absent from `sys.modules` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import -v` exits 0 |

## CI Impact

No CI changes required. This task modifies only Python source files within the existing test module (`test_arch_zit.py`) and its catalogue in `docs/TESTS.md`. No new test files, no new CI gates, no Rust changes. The existing `cargo test --workspace --features mock-hardware` and `ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v` gates continue to pick up this module unchanged.

## Platform Considerations

None identified. This task is a pure Python parameter removal with no platform-specific code paths, no `# cfg` guards, and no path-separator or line-ending handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `mock_vae = MagicMock()` on line 358 of `test_arch_zit.py` is referenced elsewhere in the test function beyond the `sample()` call — removing it could cause an `UnboundLocalError` or leave the variable unused but harmless | Low | Low | Inspect the full test function body before editing; if `mock_vae` is used elsewhere (e.g., in an assertion), keep the variable but don't pass it to `sample()`. If it's unused, remove it. |
| The `docs/TESTS.md` catalogue entries for these two tests contain prose references to "mock VAE" in the Context field that must be removed — missing a reference could leave the documentation inconsistent with the code | Low | Low | After editing the source files, search the TESTS.md entries for "vae" (case-insensitive) and remove any remaining references. Verify with `grep -in "vae" docs/TESTS.md | grep -i "sample_real"` before staging. |
| The `vae` parameter removal changes `sample()`'s positional argument count — any external caller not in this repo (e.g., the manual real-path harness `01_loaders.py`) would break at runtime | Medium | Low | The harness is external to this repo (listed as P904-A14's scope). This task only affects committed code. The harness update is handled separately by P904-A14. |
| `ZImagePipeline(vae=None)` tolerance in diffusers 0.38.0 may not hold in a future version bump — removing the parameter now is correct, but if someone re-adds it later they must verify the new diffusers version still tolerates `None` | Low | Low | The task context already documented this finding; future task authors who touch `sample()`'s signature should re-verify against the pinned diffusers version. No action needed in this task. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (all tests pass after removing `vae=` kwargs)
- [ ] `python3 -c "import inspect,os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import sample; s=inspect.signature(sample); assert 'vae' not in s.parameters"` exits 0
- [ ] `grep -n "vae=" worker/tests/test_arch_zit.py` returns zero matches (no test call site passes `vae` anymore)
- [ ] `grep -n "SlotSpec(\"vae\"" worker/nodes/sampler.py` returns zero matches (Sampler.INPUT_SLOTS must not gain a vae slot)
- [ ] `grep -in "mock.vae\|mock_vae" docs/TESTS.md | grep -i "test_sample_real_invokes_pipeline_with_correct_args\|test_sample_real_assembles_pipeline_via_cache"` returns zero matches (TESTS.md entries updated to remove vae references)
