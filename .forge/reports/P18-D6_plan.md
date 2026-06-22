# Plan Report: P18-D6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D6                                        |
| Phase       | 018 — ZiT Generic Nodes                       |
| Description | Replace LoadClip's NotImplementedError real path with real safetensors loading |
| Depends on  | P903-A1, P903-A2, P18-A3                      |
| Project     | anvilml                                       |
| Planned at  | 2026-06-22T13:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Replace the `NotImplementedError` stub in `LoadClip.execute()`'s real (non-mock) code path with a functional implementation that loads a text encoder (tokenizer + text_encoder pair) from a safetensors file using `pipeline_cache.get_or_load()`. The returned object must expose `.tokenizer` and `.text_encoder` attributes so that `ClipTextEncode` (P18-D7) can consume it. The mock path must remain completely untouched — existing tests must continue to pass.

## Scope

### In Scope
- Replace the `raise NotImplementedError(...)` block in `LoadClip.execute()` (lines 404–413 of `loader.py`) with a real loading path.
- Create a `RealClip` wrapper class exposing `.tokenizer` and `.text_encoder` attributes (mirroring the `RealModel` pattern).
- Implement `clip_type` dispatch: `"qwen3"` → `Qwen2Tokenizer` + `Qwen3ForCausalLM`; other clip types (`"clip_l"`, `"t5"`) → their respective transformers classes.
- Use `ctx.pipeline_cache.get_or_load(model_id, "bf16", loader_fn)` for caching, consistent with `LoadVae`'s pattern.
- Lazy imports of `transformers` classes inside the non-mock path (same guard pattern as `LoadModel`/`LoadVae`).
- Inline comment on the `clip_type` dispatch branch explaining the mapping.

### Out of Scope
- Mock mode changes — `ANVILML_WORKER_MOCK=1` path is untouched (P18-A3 mock tests must pass unchanged).
- `ClipTextEncode` real path — that is P18-D7's scope.
- Pipeline assembly in `arch/zit.py` — that is P18-D9a–c's scope.
- Any changes to `pipeline_cache.py` or `base.py`.
- Rust-side changes — this task touches only Python worker code.

## Existing Codebase Assessment

The `LoadClip` class already exists in `worker/nodes/loader.py` with a complete mock path and metadata (NODE_TYPE, INPUT_SLOTS, OUTPUT_SLOTS). The real path is a single `raise NotImplementedError(...)` block at lines 404–413.

The established patterns in this file are clear:
1. **Mock guard:** `if os.environ.get("ANVILML_WORKER_MOCK") == "1"` — checked at runtime, not import time.
2. **Lazy imports:** `from transformers import ...` inside the non-mock path.
3. **Caching via pipeline_cache:** `ctx.pipeline_cache.get_or_load(model_id, dtype_str, loader_fn)` where `loader_fn` is a zero-argument closure.
4. **Wrapper classes:** `RealModel` wraps a diffusers transformer and exposes `.arch` and `.in_channels`; `MockModel` mirrors this interface.
5. **Dtype selection:** Text encoder / VAE use `"bf16"`; diffusion transformer uses `"fp8"`.

The `pipeline_cache.py` module provides `PipelineCache.get_or_load(model_id, dtype, loader_fn)` with LRU eviction and OOM retry. In the test fixture, `pipeline_cache` is an empty `dict` (not a `PipelineCache` instance), but mock-mode tests never reach the real path, so this is safe.

The `ClipTextEncode` node in `encoder.py` expects a `clip` object from `LoadClip`. The task states the returned object must expose `.tokenizer` and `.text_encoder` attributes — these are the contract P18-D7 will consume.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source       | Feature flags confirmed |
|--------|-------------|-----------------|------------------|------------------------|
| python | transformers | >=4.46 (latest 5.12.1) | pypi-query MCP | n/a |

**Note on transformers class names:** The task context specifies `Qwen2Tokenizer` + `Qwen3Model`. At the time of planning, the exact class name for the Qwen3 text encoder could not be confirmed via MCP lookup (no `rust-docs` or equivalent MCP tool for Python transformers class names). The ACT agent MUST verify the exact class name at session start by checking the installed transformers package. The most likely class name is `Qwen3ForCausalLM` (following transformers naming convention), but this must be confirmed. The plan below uses placeholder names marked for verification.

## Approach

1. **Add `RealClip` class to `loader.py`** (after `MockClip`, before `LoadClip`):
   - A lightweight wrapper with `__init__(self, tokenizer: Any, text_encoder: Any)`.
   - Exposes `self._tokenizer` and `self._text_encoder` as public `.tokenizer` and `.text_encoder` properties.
   - Follows the `RealModel` pattern: stores private refs, exposes public attrs.
   - Includes Google-style docstring with Args section.

2. **Replace the `NotImplementedError` block** in `LoadClip.execute()` (lines 404–413):
   - Add lazy imports for the transformers classes needed for each `clip_type`.
   - Implement a `clip_type` dispatch using an `if/elif/else` chain:
     - `"qwen3"` → import and use `Qwen2Tokenizer` + `Qwen3ForCausalLM` (ACT agent must verify exact class names against installed transformers version).
     - `"clip_l"` → import and use `CLIPModel` + `CLIPTextModelWithProjection`.
     - `"t5"` → import and use `T5Tokenizer` + `T5ForConditionalGeneration`.
     - Any other value → raise `NodeError(f"unsupported clip_type: {clip_type}")`.
   - Each branch creates the tokenizer and text_encoder via `from_pretrained(model_id, ...)`, wrapping them in `RealClip(tokenizer, text_encoder)`.
   - Wrap the loading logic in a `loader_fn` closure and call `ctx.pipeline_cache.get_or_load(model_id, "bf16", loader_fn)`.
   - Add an inline comment on the `clip_type` dispatch explaining the mapping rationale.

3. **Ensure `__all__` exports `RealClip`** alongside existing exports.

4. **No test changes needed** — all existing tests exercise the mock path and will pass unchanged. The task's acceptance criterion is that existing mock tests continue to pass.

## Public API Surface

No new public module-level API is introduced beyond `RealClip` (which is not a node class, not registered in `NODE_REGISTRY`, and is an internal implementation detail). The `LoadClip` class signature is unchanged. The only behavioral change is that the real path no longer raises `NotImplementedError`.

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `RealClip` | class | `worker.nodes.loader.RealClip` | Internal wrapper exposing `.tokenizer` and `.text_encoder` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Add `RealClip` class; replace `NotImplementedError` in `LoadClip.execute()` with real loading path |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadclip_registered_in_registry` | LoadClip is registered in NODE_REGISTRY | Mock mode enabled; registry cleared | None | `"LoadClip" in NODE_REGISTRY` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_execute_returns_mock_clip_default_type` | execute() returns MockClip with clip_type="qwen3" in mock mode | Mock mode enabled | `model_id="test-model"` | `result["clip"].clip_type == "qwen3"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_execute_returns_mock_clip_explicit_type` | execute() returns MockClip with explicit clip_type in mock mode | Mock mode enabled | `model_id="test-model", clip_type="clip_l"` | `result["clip"].clip_type == "clip_l"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_metadata_attributes` | LoadClip metadata (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS) is correct | None | None | All six attributes verified | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. The task modifies only `worker/nodes/loader.py`. The existing `worker-linux` and `worker-windows` CI jobs run `py_compile` on all `worker/*.py` files and then `pytest worker/tests/` with mock mode — both will pick up the change automatically. No new test files are added, so no catalogue sync is needed.

## Platform Considerations

None identified. The transformers library is platform-neutral for text encoder loading (no CUDA/ROCm-specific code paths in tokenizer/text_encoder `from_pretrained`). The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The lazy import pattern ensures the transformers module is never imported in mock mode on any platform.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Transformers class names differ from task context — e.g., `Qwen3Model` does not exist; the actual class is `Qwen3ForCausalLM` or `Qwen3TextEncoderModel`. | High | High | ACT agent MUST verify at session start by checking `dir(transformers)` for available Qwen3 classes. Use the confirmed name in the implementation. Record the resolved name in the implementation report. |
| The `from_pretrained(model_id)` call for tokenizer/text_encoder expects a directory with `config.json`, not a `.safetensors` file directly — but `model_id` is a file path (not directory) per P903-A1. | Medium | High | Verify whether Z-Image-Turbo's text encoder safetensors use a directory layout (with `config.json` in the same directory as the safetensors weights) or a single-file layout. If single-file, use `from_pretrained(model_id, subfolder=...)` or the `safetensors` direct-loading path. The ACT agent must confirm the actual filesystem layout. |
| `Qwen2Tokenizer` may have been renamed to `Qwen2TokenizerFast` in newer transformers versions, or vice versa. | Medium | Medium | ACT agent must verify at session start. If `Qwen2TokenizerFast` is the only option, use it and note the substitution. |
| The `RealClip` object's `.tokenizer` and `.text_encoder` attributes may not match what P18-D7's `ClipTextEncode` expects. | Low | High | P18-D6 and P18-D7 should coordinate at ACT time. If P18-D7 has already been implemented, check its source for the attribute names it consumes. If P18-D7 is not yet implemented, use `.tokenizer` and `.text_encoder` as specified in the task description and note the coordination point. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 — all existing mock tests pass (≥ 4 tests including LoadClip tests)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0 — syntax check passes
- [ ] `RealClip` class exists and exposes `.tokenizer` and `.text_encoder` attributes
- [ ] `LoadClip.execute()` real path no longer raises `NotImplementedError` (verifiable by reading source)
- [ ] Inline comment present on the `clip_type` dispatch branch explaining the mapping
- [ ] `__all__` in `loader.py` includes `RealClip`
