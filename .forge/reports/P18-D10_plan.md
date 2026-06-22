# Plan Report: P18-D10

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D10                                     |
| Phase       | 18 — ZiT Generic Nodes                      |
| Description | worker/nodes/arch/clip/clip_l.py: single-file CLIP-L text encoder loading |
| Depends on  | P18-D8                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T23:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/arch/clip/clip_l.py`, a single-file CLIP-L text encoder loading module that mirrors the pattern established by `qwen3.py` (P18-D9). The module provides `can_handle(clip_type)` returning `True` only for `"clip_l"`, and `load(model_id, torch_dtype)` that returns a `RealClip(tokenizer, model)` wrapping a `CLIPTokenizer` (loaded from the vendored `clip_l_tokenizer` assets) and a `CLIPTextModelWithProjection` constructed from verbatim `openai/clip-vit-large-patch14` config values with weights loaded via `safetensors.torch.load_file`. In mock mode (`ANVILML_WORKER_MOCK=1`), `load()` returns `RealClip(MockTokenizer(), MockTextEncoder())` without importing torch or transformers.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/clip_l.py` with `can_handle()` and `load()` functions.
- Create `worker/tests/test_arch_clip_l.py` with ≥3 tests covering: `can_handle("clip_l")` returns True, `can_handle("qwen3")`/`can_handle("t5")` returns False, mock `load()` returns `RealClip` with sentinel objects, and import isolation (no torch at module load time).

### Out of Scope
None. `defers_to` is empty — this task implements its full scope. No stub, mock-only return path, or `NotImplementedError` is permitted for any functionality described in the task context.

## Existing Codebase Assessment

The codebase already has a complete pattern to follow. `worker/nodes/arch/clip/qwen3.py` (created in P18-D9) provides the exact structural template: a module-level `can_handle(clip_type)` that does a string comparison, and a `load(model_id, torch_dtype)` function that checks `ANVILML_WORKER_MOCK` at runtime, returns a `RealClip(MockTokenizer(), MockTextEncoder())` in mock mode via lazy import from `worker.nodes.loader`, and in real mode constructs the model from verbatim config values.

The `RealClip` wrapper class lives in `worker/nodes/loader.py` with `tokenizer` and `text_encoder` properties. `MockTokenizer` and `MockTextEncoder` are also defined there as lightweight sentinel classes. These are imported lazily inside the mock code path of `qwen3.py` to avoid circular imports (since `loader.py`'s `LoadClip` node will eventually call `arch.clip.get_module()`).

The `worker/nodes/arch/clip/__init__.py` dispatcher already uses `pkgutil.iter_modules()` to auto-import all sibling `.py` files and provides `can_handle(clip_type)` and `get_module(clip_type)` functions. Adding `clip_l.py` to the directory means the dispatcher will automatically discover and use it — no changes to `__init__.py` are needed.

The test file `worker/tests/test_arch_clip_qwen3.py` establishes the exact test pattern: `can_handle` positive/negative tests, mock `load()` type assertions, and an import isolation test that removes `torch` from `sys.modules` before re-importing the module. The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for all tests.

No gap exists between the design doc and current source for this task — the qwen3.py module is a near-identical structural match for what clip_l.py needs, differing only in the specific transformers classes and config values used.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source         | Feature flags confirmed |
|--------|-------------|-----------------|--------------------|------------------------|
| python | transformers| 5.12.1          | pypi-query MCP     | n/a                    |
| python | safetensors | 0.8.0           | pypi-query MCP     | torch extra for `load_file` |

The project's `worker/requirements/base.txt` pins `transformers>=5.12` and `safetensors>=0.4`, which are satisfied by the MCP-verified versions. The types used (`CLIPTokenizer`, `CLIPTextConfig`, `CLIPTextModelWithProjection`) all exist in transformers 5.12.1. The `safetensors.torch.load_file` function exists in safetensors 0.8.0.

## Approach

1. **Create `worker/nodes/arch/clip/clip_l.py`.** Copy the structural template from `qwen3.py`:
   - Module docstring describing mock vs real behavior and import isolation requirements.
   - `from __future__ import annotations` and `import os` at top level only.
   - `__all__ = ["can_handle", "load"]`.
   - `can_handle(clip_type: str) -> bool`: return `clip_type == "clip_l"`. Add a docstring explaining the string comparison and inline comment noting this is the canonical identifier for CLIP-L text encoders.
   - `load(model_id: str, torch_dtype: Any) -> RealClip` (with `# noqa: F821` for the forward reference):
     - Check `_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"`.
     - If mock: lazy import `MockTokenizer`, `MockTextEncoder`, `RealClip` from `worker.nodes.loader`; return `RealClip(MockTokenizer(), MockTextEncoder())`.
     - If real: lazy imports — `from pathlib import Path`, `from safetensors.torch import load_file as safetensors_load_file`, `from transformers import CLIPTokenizer, CLIPTextConfig, CLIPTextModelWithProjection`, `from worker.nodes.loader import RealClip`.
     - Resolve tokenizer dir: `Path(__file__).parent.parent / "assets" / "clip_l_tokenizer"`.
     - Load tokenizer: `CLIPTokenizer.from_pretrained(tokenizer_dir)`.
     - Verbatim config values from `openai/clip-vit-large-patch14`'s `config.json`:
       ```python
       config_values = {
           "vocab_size": 49408,
           "hidden_size": 768,
           "intermediate_size": 3072,
           "num_hidden_layers": 12,
           "num_attention_heads": 12,
           "projection_dim": 768,
           "max_position_embeddings": 77,
       }
       ```
     - Construct model: `model = CLIPTextModelWithProjection(CLIPTextConfig(**config_values))`.
     - Load weights: `model.load_state_dict(safetensors_load_file(model_id))`.
     - Return `RealClip(tokenizer, model)`.
   - Every function gets a Google-style docstring. Every decision point gets an inline `#` comment explaining the rationale.

2. **Create `worker/tests/test_arch_clip_l.py`.** Mirror the test structure from `test_arch_clip_qwen3.py`:
   - `test_can_handle_clip_l()`: assert `can_handle("clip_l")` is `True`.
   - `test_can_handle_non_clip_l()`: assert `can_handle("qwen3")` and `can_handle("t5")` are `False`.
   - `test_load_mock_returns_realclip()`: call `load("/fake/path", None)`, assert result is `RealClip` with `MockTokenizer` and `MockTextEncoder`.
   - `test_load_mock_no_torch_import()`: remove `torch` from `sys.modules`, reload the module, assert `"torch"` not in `sys.modules` after import and that public API is intact.

3. **Verify no torch/transformers import at module level.** The mock-mode code path imports these only inside the `if _mock:` guard, identical to qwen3.py's pattern. The test `test_load_mock_no_torch_import()` verifies this.

## Public API Surface

| Module Path | Item | Signature |
|-------------|------|-----------|
| `worker.nodes.arch.clip.clip_l` | `can_handle` | `def can_handle(clip_type: str) -> bool` |
| `worker.nodes.arch.clip.clip_l` | `load` | `def load(model_id: str, torch_dtype: Any) -> RealClip` |

Both functions are module-level public API. No classes, traits, or re-exports are introduced. The `RealClip` type is imported from `worker.nodes.loader` (forward reference via `# noqa: F821` in the function signature, resolved at call time).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/clip_l.py` | CLIP-L text encoder arch module with `can_handle()` and `load()` |
| CREATE | `worker/tests/test_arch_clip_l.py` | Unit tests for clip_l.py (≥4 tests) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_l.py` | `test_can_handle_clip_l` | `can_handle("clip_l")` returns `True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_can_handle_clip_l -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_can_handle_non_clip_l` | `can_handle("qwen3")` and `can_handle("t5")` return `False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_can_handle_non_clip_l -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_returns_realclip` | `load()` returns `RealClip(MockTokenizer(), MockTextEncoder())` in mock mode | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_load_mock_returns_realclip -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_no_torch_import` | Module imports cleanly without torch in mock mode | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import -v` exits 0 |

Acceptance command for full test suite: `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py -v` exits 0 with ≥3 tests.

## CI Impact

No CI changes required. The new test file follows the established naming convention (`test_arch_clip_l.py` for `clip_l.py`) and is automatically picked up by the existing `worker-linux` and `worker-windows` CI jobs that run `pytest worker/tests/ -v`. No new file types, gates, or test modules are introduced beyond what the CI already handles.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The module uses only standard library (`os`, `pathlib`, `importlib`, `pkgutil`-compatible patterns) and platform-agnostic transformers/safetensors APIs. Path resolution via `Path(__file__).parent.parent / "assets" / "clip_l_tokenizer"` works correctly on both Linux and Windows (pathlib normalizes separators).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `clip_l_tokenizer` directory may not exist in the workspace (only `qwen25_tokenizer` and `t5_tokenizer` directories confirmed present). If missing, real-mode `load()` will raise `FileNotFoundError` on `CLIPTokenizer.from_pretrained(tokenizer_dir)`. | Medium | High | The task context states the tokenizer is vendored at `worker/assets/clip_l_tokenizer/`. The ARCHITECTURE.md §2 layout confirms this directory exists. If it is absent at ACT time, the ACT agent must create it or block. |
| `CLIPTextConfig` in transformers 5.12.1 may not accept the exact config keys listed (e.g. `projection_dim` might be named differently or have a different default). | Low | Medium | The task context specifies these are verbatim values from `openai/clip-vit-large-patch14`'s `config.json`. The ACT agent should confirm at session start that `CLIPTextConfig` accepts these keys. If not, use the actual key names from the resolved version. |
| `safetensors.torch.load_file` returns a dict with tensor keys prefixed `text_model.` (per task context). If the model checkpoint uses a different prefix convention, `load_state_dict()` will fail with missing/unexpected keys. | Low | High | The task context explicitly states keys are prefixed `text_model.` and were confirmed against an actual constructed model's `state_dict()`. The ACT agent should verify this assumption at session start. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/arch/clip/clip_l.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch.clip.clip_l import can_handle, load; assert can_handle('clip_l') is True; assert can_handle('qwen3') is False"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch.clip.clip_l import load; from worker.nodes.loader import RealClip, MockTokenizer, MockTextEncoder; r = load('/fake', None); assert isinstance(r, RealClip); assert isinstance(r.tokenizer, MockTokenizer); assert isinstance(r.text_encoder, MockTextEncoder)"` exits 0
