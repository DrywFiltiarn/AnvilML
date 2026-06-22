# Plan Report: P18-D11

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D11                                           |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/arch/clip/t5.py: single-file T5-XXL text encoder loading |
| Depends on  | P18-D8                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/nodes/arch/clip/t5.py`, the T5-XXL text encoder architecture dispatch module that enables `LoadClip` to load a real T5 text encoder from a single `.safetensors` file. The module provides `can_handle(clip_type)` (returns `True` for `"t5"`) and `load(model_id, torch_dtype)` (returns `RealClip(tokenizer, model)`). In mock mode (`ANVILML_WORKER_MOCK=1`), it returns `RealClip(MockTokenizer(), MockTextEncoder())` without importing torch or transformers. Also modify `worker/nodes/loader.py` to switch the T5 branch's tokenizer import from `T5Tokenizer` to `T5TokenizerFast`, matching the real-mode usage in this task.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/t5.py` with `can_handle()` and `load()` following the established qwen3.py / clip_l.py pattern exactly.
- Modify `worker/nodes/loader.py` line ~514: change `from transformers import T5ForConditionalGeneration, T5Tokenizer` to `from transformers import T5ForConditionalGeneration, T5TokenizerFast` and update `tokenizer_cls = T5Tokenizer` to `tokenizer_cls = T5TokenizerFast`.
- Create `worker/tests/test_arch_clip_t5.py` with ≥3 tests covering: `can_handle("t5")` returns True, mock `load()` returns `RealClip` with sentinel objects, import isolation (no torch import at module level).

### Out of Scope
None. `defers_to (from JSON): []` — no deferrals permitted. This task implements its full scope without stubs or placeholders.

## Existing Codebase Assessment

The codebase has two established CLIP architecture dispatch modules (`qwen3.py` and `clip_l.py`) in `worker/nodes/arch/clip/` that serve as the exact template for this task. Both follow the same structure:

(a) **What already exists**: `can_handle()` returns `True` for the canonical clip type string; `load()` checks `ANVILML_WORKER_MOCK` env var, returns `RealClip(MockTokenizer(), MockTextEncoder())` in mock mode, and in real mode lazily imports `T5TokenizerFast`/`T5Config`/`T5EncoderModel`/`safetensors`, constructs the model from verbatim config values, loads weights via `load_file()`, and returns `RealClip(tokenizer, model)`. The `arch/clip/__init__.py` auto-imports all sibling modules via `pkgutil.iter_modules()`. The `loader.py` module already has a T5 branch (lines 510–517) but uses the wrong tokenizer class (`T5Tokenizer` instead of `T5TokenizerFast`).

(b) **Established patterns**: Module docstring at top (Google-style), `from __future__ import annotations`, `import os`, `from typing import Any`, `__all__ = ["can_handle", "load"]`, `can_handle()` with docstring and inline comment, `load()` with `# noqa: F821` for `RealClip` forward reference, mock-mode guard using `os.environ.get("ANVILML_WORKER_MOCK") == "1"`, lazy imports inside the real path, tokenizer resolved via `Path(__file__).parent.parent / "assets" / "<name>_tokenizer"`, verbatim config dict (not defaults), `model = <Class>(<Config>(**config_values))` then `model.load_state_dict(safetensors_load_file(model_id))`, return `RealClip(tokenizer, model)`. Tests follow the same 4-test pattern: `can_handle` true, `can_handle` false for others, mock load returns RealClip, import isolation.

(c) **Gap**: The `loader.py` T5 branch uses `T5Tokenizer` (slow) while the real-mode `load()` in `t5.py` will use `T5TokenizerFast`. This inconsistency must be fixed in `loader.py` as part of this task. The `t5.py` file does not yet exist — it is the sole new file.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | transformers| 5.12.x          | pypi-query MCP | n/a                    |
| python | safetensors | 0.8.x           | pypi-query MCP | n/a                    |

Both `T5TokenizerFast` and `T5EncoderModel` are standard, stable APIs in `transformers>=5.12`. The `safetensors.torch.load_file` function is the standard single-file loader. No feature flags needed.

## Approach

1. **Create `worker/nodes/arch/clip/t5.py`** — mirror the qwen3.py / clip_l.py pattern exactly:
   - Module docstring describing T5-XXL dispatch, mock mode behavior, and the no-import-at-top-level constraint. Include `.. versionadded:: 0.1.0`.
   - `from __future__ import annotations`, `import os`, `from typing import Any`.
   - `__all__ = ["can_handle", "load"]`.
   - `def can_handle(clip_type: str) -> bool:` — docstring with Args/Returns, inline comment explaining the canonical identifier. Return `clip_type == "t5"`.
   - `def load(model_id: str, torch_dtype: Any) -> RealClip:  # noqa: F821` — docstring with Args/Returns/Raises. Check `_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"`. In mock path: `from worker.nodes.loader import MockTokenizer, MockTextEncoder, RealClip`, return `RealClip(MockTokenizer(), MockTextEncoder())`. In real path: lazy imports — `from pathlib import Path`, `from safetensors.torch import load_file as safetensors_load_file`, `from transformers import T5Config, T5EncoderModel, T5TokenizerFast`, `from worker.nodes.loader import RealClip`. Resolve tokenizer dir: `Path(__file__).parent.parent / "assets" / "t5_tokenizer"`. Load tokenizer: `T5TokenizerFast.from_pretrained(tokenizer_dir)`. Verbatim config values from `google/t5-v1_1-xxl`: `vocab_size=32128, d_model=4096, d_kv=64, d_ff=10240, num_layers=24, num_heads=64, relative_attention_num_buckets=32, feed_forward_proj="gated-gelu", tie_word_embeddings=False`. Construct: `T5EncoderModel(T5Config(**config_values))`. Load weights: `.load_state_dict(safetensors_load_file(model_id))`. Return `RealClip(tokenizer, model)`.

2. **Modify `worker/nodes/loader.py`** — change the T5 branch (around line 514):
   - Change `from transformers import T5ForConditionalGeneration, T5Tokenizer` to `from transformers import T5ForConditionalGeneration, T5TokenizerFast`.
   - Change `tokenizer_cls = T5Tokenizer` to `tokenizer_cls = T5TokenizerFast`.
   - This is a mechanical one-line change to keep the loader.py path consistent with the t5.py real-mode usage.

3. **Create `worker/tests/test_arch_clip_t5.py`** — mirror the qwen3.py test pattern exactly:
   - `test_can_handle_t5()` — assert `can_handle("t5") is True`.
   - `test_can_handle_non_t5()` — assert `can_handle("qwen3")` and `can_handle("clip_l")` are `False`.
   - `test_load_mock_returns_realclip()` — assert `load("/fake/path", None)` returns `RealClip` with `MockTokenizer` and `MockTextEncoder`.
   - `test_load_mock_no_torch_import()` — remove torch from sys.modules, re-import, assert torch absent, assert public API callable.

4. **Pre-stop verification** — write the report, run the three pre-stop checks.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `can_handle` | function | `worker.nodes.arch.clip.t5` | `def can_handle(clip_type: str) -> bool` |
| `load` | function | `worker.nodes.arch.clip.t5` | `def load(model_id: str, torch_dtype: Any) -> RealClip` |

Both are in `__all__`. No new types or classes are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/t5.py` | T5-XXL text encoder dispatch module with `can_handle()` and `load()` |
| MODIFY | `worker/nodes/loader.py` | Switch T5Tokenizer → T5TokenizerFast in the T5 branch (line ~514) |
| CREATE | `worker/tests/test_arch_clip_t5.py` | ≥3 tests: can_handle dispatch, mock load, import isolation |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_t5.py` | `test_can_handle_t5` | `can_handle("t5")` returns `True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py::test_can_handle_t5 -v` exits 0 |
| `worker/tests/test_arch_clip_t5.py` | `test_can_handle_non_t5` | `can_handle("qwen3")` and `can_handle("clip_l")` return `False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py::test_can_handle_non_t5 -v` exits 0 |
| `worker/tests/test_arch_clip_t5.py` | `test_load_mock_returns_realclip` | `load()` returns `RealClip(MockTokenizer(), MockTextEncoder())` in mock mode | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py::test_load_mock_returns_realclip -v` exits 0 |
| `worker/tests/test_arch_clip_t5.py` | `test_load_mock_no_torch_import` | Module imports without torch in mock mode | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py::test_load_mock_no_torch_import -v` exits 0 |

## CI Impact

No CI changes required. The new test file is picked up automatically by the existing `worker-linux` and `worker-windows` CI jobs which run `pytest worker/tests/ -v`. No new file types, gates, or test modules are added — just one new test file in the existing directory.

## Platform Considerations

None identified. The `pathlib.Path` resolution (`Path(__file__).parent.parent / "assets" / "t5_tokenizer"`) is cross-platform — it works identically on Linux and Windows. The `ANVILML_WORKER_MOCK` environment variable check is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `T5TokenizerFast` API shape may differ in the installed transformers version — e.g. `from_pretrained` signature or required arguments could change. | Low | High | The MCP confirms transformers>=5.12 is required, and `T5TokenizerFast.from_pretrained(path)` is a stable API that has existed since transformers 4.x. The ACT agent must verify the exact import at session start and confirm the method exists. |
| `T5EncoderModel` state dict keys may not match the safetensors file layout — the task specifies `encoder.block.N.layer.M.SelfAttention.{q,k,v,o}.weight` keys, but the actual checkpoint may use different prefixes (e.g. `shared.weight`). | Low | High | The task states these keys are "confirmed against an actual constructed model's state_dict() keys." The ACT agent should verify this at session start by constructing a model and inspecting its state_dict keys before loading weights. |
| The `t5_tokenizer` vendored asset directory may be missing or incomplete at ACT time. | Low | Medium | The directory already exists in the repo (confirmed: `tokenizer.json`, `tokenizer_config.json`, `special_tokens_map.json`). The plan notes this as a precondition. If missing at ACT time, the ACT agent must re-seed via `worker/tools/seed_tokenizers.sh`. |
| `loader.py` T5 branch modification may break existing tests if the test suite depends on `T5Tokenizer` being importable (unlikely since tests run in mock mode). | Low | Low | Tests run with `ANVILML_WORKER_MOCK=1` which never enters the real-mode path in `loader.py`. The change is purely an import statement swap within the real-mode branch — no test behavior changes. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/clip/t5.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py -v` exits 0 with ≥ 3 tests passing
- [ ] `grep -n "T5TokenizerFast" worker/nodes/loader.py` returns at least 1 match (import and assignment both updated)
- [ ] `grep -n "T5Tokenizer[^F]" worker/nodes/loader.py` returns 0 matches (no stale T5Tokenizer references remain)
