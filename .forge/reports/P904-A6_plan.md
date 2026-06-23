# Plan Report: P904-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A6                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py + sampler.py: loader_fn reads tokenizer/text_encoder off the wrong object |
| Depends on  | P18-D18a, P18-D19, P904-A5                  |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T20:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix `zit.py`'s `loader_fn` which reads `.tokenizer` and `.text_encoder` off the `conditioning` object (a `Conditioning` instance that only has `.positive`/`.negative`) instead of off the `clip` object (a `RealClip` instance that actually carries those attributes). Add a `clip` parameter to `sample()`'s signature, wire it through `Sampler.execute()`, and update the two affected real-mode tests to pass a mock `clip` object.

## Scope

### In Scope
- `worker/nodes/arch/diffusion/zit.py`: add `clip: Any = None` to `sample()`'s signature (after `conditioning`, before `latent`); change `loader_fn` to read `tokenizer` and `text_encoder` from `clip` instead of `conditioning`
- `worker/nodes/sampler.py`: add `SlotSpec("clip", "CLIP")` to `Sampler.INPUT_SLOTS`; pass `clip=inputs.get("clip")` through to `mod.sample(...)` in `Sampler.execute()`
- `worker/tests/test_arch_zit.py`: update `test_sample_real_assembles_pipeline_via_cache` and `test_sample_real_invokes_pipeline_with_correct_args` to pass a mock `clip` object with `.tokenizer`/`.text_encoder` attributes; remove the `.tokenizer`/`.text_encoder` attributes from the mock conditioning objects (they are no longer read)
- `docs/TESTS.md`: update catalogue entries for both tests to reflect the new `clip` argument and the corrected conditioning mock

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. The vestigial `vae` parameter removal is handled by the dependent task P904-A6b. The `ANVILML_DESIGN.md` is human-authored only and explicitly out of scope.

## Existing Codebase Assessment

The `sample()` function in `zit.py` (line 212) currently accepts 11 parameters: `model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None`. Its inner `loader_fn` closure (line 298) reads `tokenizer` and `text_encoder` from the `conditioning` object via `getattr(conditioning, "tokenizer", None)` and `getattr(conditioning, "text_encoder", None)` — lines 309–310. This is a wiring defect: `Conditioning` (defined in `encoder.py`, line 33) only ever carries `.positive` and `.negative` attributes, never `.tokenizer` or `.text_encoder`.

The `clip` object that actually carries `.tokenizer` and `.text_encoder` is `RealClip` (defined in `loader.py`, line 121), produced by `LoadClip.execute()` and passed to `ClipTextEncode`. The `ANVILML_DESIGN.md §10.4` (line 1118) already documents the correct signature: `def sample(model, conditioning, clip, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]` — the design doc was pre-updated by a human ahead of this phase.

`Sampler.execute()` (line 236 in `sampler.py`) currently calls `mod.sample()` with 9 positional arguments followed by `pipeline_cache=` as keyword-only. It does not read or pass a `clip` input at all, and `Sampler.INPUT_SLOTS` does not include a `CLIP` slot. The three mock-mode test calls omit `clip` and rely on its default `None`, which is fine because the mock path returns before reaching `loader_fn`.

## Resolved Dependencies

None. This task only modifies existing Python source files and does not introduce any new dependencies.

## Approach

1. **Add `clip` parameter to `sample()`'s signature in `zit.py`.**
   - In `worker/nodes/arch/diffusion/zit.py`, add `clip: Any = None` as the third positional parameter, immediately after `conditioning` and before `latent`. This shifts `latent` from position 3 to position 4 in the parameter list. The `vae` parameter stays where it is (after `emit_progress`, before the `*`).
   - Updated signature: `def sample(model, conditioning, clip, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None) -> tuple[Any, int]:`
   - Rationale: The design doc (§10.4, line 1118) specifies this exact parameter order. Placing `clip` after `conditioning` reflects the logical dependency: the sampler needs the clip object to resolve tokenizer/text_encoder before it needs the latent (which is the input noise).

2. **Fix `loader_fn` to read from `clip` instead of `conditioning`.**
   - In `zit.py`'s `loader_fn` (line 298), replace:
     ```python
     tokenizer = getattr(conditioning, "tokenizer", None)
     text_encoder = getattr(conditioning, "text_encoder", None)
     ```
     with:
     ```python
     tokenizer = getattr(clip, "tokenizer", None) if clip else None
     text_encoder = getattr(clip, "text_encoder", None) if clip else None
     ```
   - Rationale: `clip` may be `None` in mock-mode tests (which call `sample()` without a clip argument). The conditional `if clip` guard prevents `getattr` on `None`. In real mode, `clip` will always be a `RealClip` instance with both attributes.
   - Update the comment on lines 306–308 to reflect the corrected source object: "Pull tokenizer and text_encoder from clip (RealClip). The clip object is produced by LoadClip and carries these attributes."

3. **Add `clip` to `Sampler.INPUT_SLOTS` in `sampler.py`.**
   - In `worker/nodes/sampler.py`, add `SlotSpec("clip", "CLIP")` to `Sampler.INPUT_SLOTS` after the `"conditioning"` slot (line 224) and before the `"latent"` slot (line 225).
   - Updated `INPUT_SLOTS`: `[SlotSpec("model", "MODEL"), SlotSpec("conditioning", "CONDITIONING"), SlotSpec("clip", "CLIP"), SlotSpec("latent", "LATENT"), SlotSpec("steps", "INT"), SlotSpec("cfg", "FLOAT"), SlotSpec("seed", "INT")]`
   - Also update the docstring for `Sampler` (line 208) to include `clip` (CLIP, required) in the INPUT_SLOTS description.

4. **Pass `clip` through in `Sampler.execute()`.**
   - In `sampler.py`'s `execute()` method, add `clip = inputs.get("clip")` alongside the other input reads (after `latent = inputs.get("latent")`).
   - Update the `mod.sample()` call (line 329–333) to include `clip` as the third positional argument:
     ```python
     result = mod.sample(
         model, conditioning, clip, latent, steps, cfg, seed,
         self.ctx.device, self.ctx.cancel_flag, emit_progress,
         pipeline_cache=self.ctx.pipeline_cache,
     )
     ```
   - Rationale: The positional argument order must match `sample()`'s new signature (step 1). `clip` is the third positional parameter, `latent` is now the fourth.

5. **Update `test_sample_real_assembles_pipeline_via_cache` in `test_arch_zit.py`.**
   - After constructing the mock conditioning object (line 245–250), add a mock clip object:
     ```python
     mock_clip = type("RealClip", (), {
         "tokenizer": MagicMock(),
         "text_encoder": MagicMock(),
     })()
     ```
   - Remove `"tokenizer": None` and `"text_encoder": None` from the conditioning object's attributes (they are no longer read by `loader_fn`).
   - Add `clip=mock_clip` to the `sample()` call (line 253–265).
   - The conditioning mock now only needs `"positive": None, "negative": None`.

6. **Update `test_sample_real_invokes_pipeline_with_correct_args` in `test_arch_zit.py`.**
   - After constructing the mock conditioning object (line 337–342), add a mock clip object:
     ```python
     mock_clip = type("RealClip", (), {
         "tokenizer": MagicMock(),
         "text_encoder": MagicMock(),
     })()
     ```
   - Remove `"tokenizer": MagicMock()` and `"text_encoder": MagicMock()` from the conditioning object's attributes.
   - Add `clip=mock_clip` to the `sample()` call (line 348–360).
   - The conditioning mock now only needs `"positive": MagicMock(), "negative": MagicMock()`.

7. **Update `docs/TESTS.md` catalogue entries.**
   - For `test_sample_real_assembles_pipeline_via_cache` (line 3433): Update the Context section to note that a separate mock `clip` object with `.tokenizer`/`.text_encoder` attributes is passed to `sample()`. Update the Inputs section to include `clip=mock_clip`.
   - For `test_sample_real_invokes_pipeline_with_correct_args` (line 3442): Same updates — note the separate mock `clip` object and add it to the Inputs.

## Public API Surface

| Item | Before | After | Module Path |
|------|--------|-------|-------------|
| `sample()` signature | `def sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None)` | `def sample(model, conditioning, clip, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None)` | `worker.nodes.arch.diffusion.zit` |
| `Sampler.INPUT_SLOTS` | 7 slots (model, conditioning, latent, steps, cfg, seed) | 8 slots (model, conditioning, **clip**, latent, steps, cfg, seed) | `worker.nodes.sampler` |
| `Sampler.execute()` call to `mod.sample()` | 9 positional args + `pipeline_cache=` | 10 positional args (inserted `clip` as 3rd) + `pipeline_cache=` | `worker.nodes.sampler` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `clip` parameter to `sample()` signature; fix `loader_fn` to read from `clip` |
| MODIFY | `worker/nodes/sampler.py` | Add `SlotSpec("clip", "CLIP")` to `INPUT_SLOTS`; pass `clip` through to `mod.sample()` |
| MODIFY | `worker/tests/test_arch_zit.py` | Update two real-mode tests to pass mock `clip`; remove `.tokenizer`/`.text_encoder` from conditioning mocks |
| MODIFY | `docs/TESTS.md` | Update catalogue entries for both tests to reflect the `clip` argument |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_zit.py` | `test_sample_real_assembles_pipeline_via_cache` | After adding `clip` parameter, real-mode path calls `pipeline_cache.get_or_load()` with correct key; mock `clip` supplies tokenizer/text_encoder to `loader_fn` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_real_invokes_pipeline_with_correct_args` | After adding `clip` parameter, real-mode path invokes pipeline with all expected kwargs; mock `clip` supplies tokenizer/text_encoder to `loader_fn` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_returns_mock_latent_and_seed` | Mock-mode `sample()` still returns `(MockLatent(), seed)` without `clip` argument (three mock calls omit `clip`, relying on default) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_preserves_seed_value` | Mock-mode seed preservation still works with new signature | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value -v` exits 0 |

## CI Impact

No CI changes required. The modified test file (`test_arch_zit.py`) is already collected by the default `pytest worker/tests/` invocation. The mock-mode tests (which are the only ones CI runs) are unaffected because `clip=None` is the default and the mock path never reaches `loader_fn`.

## Platform Considerations

None identified. The `clip` parameter is a plain Python object with no platform-specific behavior. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The positional argument reorder in `sample()` (inserting `clip` between `conditioning` and `latent`) could break any external caller that passes positional arguments in the old order. However, `sample()` is only called from `Sampler.execute()` within this project, and the call site is updated in the same task. | Low | High | The call site update (step 4) is part of the same task. No other callers exist — verified by grep: `mod.sample(` appears only once in `sampler.py`. |
| The mock conditioning objects in the real-mode tests currently carry `.tokenizer` and `.text_encoder` attributes as a workaround for the bug. After removing them, if any test assertion accidentally depends on those attributes, the test would fail. | Low | Medium | The test assertions only check `get_or_load` call args and pipeline `__call__` kwargs — none reference the conditioning object's attributes. Removing `.tokenizer`/`.text_encoder` from the conditioning mock is safe. |
| The `if clip` guard in `loader_fn` could mask a real-mode bug where `clip` is `None` at runtime. If `clip=None` reaches real mode, `tokenizer` and `text_encoder` would both be `None`, causing `ZImagePipeline` construction to fail with a cryptic error. | Low | High | The guard is defensive — in real mode, `Sampler.execute()` always passes `clip=inputs.get("clip")` which is a `RealClip` from `LoadClip`. If `clip` were `None` in real mode, the error would surface at pipeline construction time with a clear attribute error, not silently. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (all mock-mode and real-mode tests in the file pass)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 (Sampler node tests unaffected)
- [ ] `python3 -c "import inspect, os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import sample; s=inspect.signature(sample); params=list(s.parameters.keys()); assert params[2] == 'clip'"` exits 0 (clip is the 3rd positional parameter)
- [ ] `grep -n "getattr(clip" worker/nodes/arch/diffusion/zit.py` returns at least 2 matches (loader_fn reads from clip)
- [ ] `grep -n "SlotSpec.*clip" worker/nodes/sampler.py` returns at least 1 match (clip in INPUT_SLOTS)
- [ ] `grep -n "clip=inputs.get" worker/nodes/sampler.py` returns at least 1 match (clip passed through)
- [ ] `grep -n "test_sample_real_assembles_pipeline_via_cache\|test_sample_real_invokes_pipeline_with_correct_args" docs/TESTS.md` returns both entries updated to mention the clip argument
