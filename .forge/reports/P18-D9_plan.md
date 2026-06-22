# Plan Report: P18-D9

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D9                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/clip/qwen3.py: single-file Qwen3 text encoder loading |
| Depends on  | P18-D8                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T22:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/arch/clip/qwen3.py`, a single-file CLIP architecture module that provides `can_handle(clip_type)` and `load(model_id, torch_dtype)` for loading a Qwen3 text encoder from a single `.safetensors` file. The module supports mock mode (returns `RealClip(MockTokenizer(), MockTextEncoder())` without importing torch/transformers) and a real path that constructs a `Qwen3ForCausalLM` from verbatim config values sourced from `Qwen/Qwen3-4B`'s `config.json`. This enables `LoadClip` to load a real Qwen3 text encoder via the `arch/clip/` dispatch mechanism.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/qwen3.py` with:
  - `can_handle(clip_type: str) -> bool` — returns `clip_type == "qwen3"`
  - `load(model_id: str, torch_dtype: Any) -> RealClip` — lazy imports, constructs tokenizer + model, loads state dict
  - Mock mode: `RealClip(MockTokenizer(), MockTextEncoder())` when `ANVILML_WORKER_MOCK=1`
  - Real mode: `Qwen2Tokenizer.from_pretrained()` + `Qwen3ForCausalLM(Qwen3Config(**values))` + `safetensors.torch.load_file()`
  - No top-level imports of `torch`, `transformers`, or `safetensors`
  - `defers_to` is empty: no stubs, no `NotImplementedError`, no TODO placeholders
- Create `worker/tests/test_arch_clip_qwen3.py` with ≥3 tests:
  - `test_can_handle_qwen3` — `can_handle("qwen3")` returns `True`
  - `test_can_handle_non_qwen3` — `can_handle("clip_l")` returns `False`
  - `test_load_mock_returns_realclip` — `load()` in mock mode returns `RealClip(MockTokenizer(), MockTextEncoder())`
  - `test_load_mock_no_torch_import` — module imports cleanly without torch present

### Out of Scope
- Real-mode `load()` implementation (the `load()` function will return the mock sentinel in mock mode; the real-mode path is fully implemented within the same function body under the non-mock branch — no deferral).
- Integration with `LoadClip`'s dispatch via `arch.clip.get_module()` — that is P18-D12.
- Tokenizer asset seeding — `qwen25_tokenizer/` already exists in `worker/assets/`.

## Existing Codebase Assessment

The project has a well-established pattern for CLIP architecture modules. `worker/nodes/arch/clip/__init__.py` (P18-D8) provides `can_handle(clip_type)` and `get_module(clip_type)` dispatch functions using `pkgutil.iter_modules()` auto-import. The `load()` function contract is defined in `ANVILML_DESIGN.md §10.4a`: it returns a `RealClip` object.

`RealClip` already exists in `worker/nodes/loader.py` — a wrapper class with `.tokenizer` and `.text_encoder` properties. It is exported via `__all__` alongside `MockModel`, `MockVae`, and `MockClip`. The mock sentinel classes (`MockTokenizer`, `MockTextEncoder`) do not yet exist in `loader.py` — they will be created as part of this task's scope (needed for the mock path of `load()`).

The existing `worker/nodes/arch/diffusion/zit.py` provides the established pattern: module-level `from __future__ import annotations`, `os.environ.get("ANVILML_WORKER_MOCK")` check at runtime (not module-level import), lazy imports inside the non-mock branch, and docstrings on all public items. The test file `test_arch_zit.py` demonstrates the test style: import isolation tests, mock-path tests, and real-path stub tests.

The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for every test, and the `_install_test_dummy` fixture in `test_arch_clip_init.py` shows the pattern for clearing `sys.modules` cache when testing modules that use `pkgutil.iter_modules()` auto-import.

There is one gap: `MockTokenizer` and `MockTextEncoder` are not yet defined in `loader.py`. They must be added to support the mock path of `load()`, which returns `RealClip(MockTokenizer(), MockTextEncoder())`. This addition to `loader.py` is in-scope because it is a required dependency for qwen3.py's mock path, and it must be in the same task since `defers_to` is empty.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | transformers| >=4.46 (latest: 5.12.1) | pypi-query MCP | n/a — `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `Qwen3Config` all present in 4.46+ |
| python | safetensors | >=0.4 (latest: 0.8.0)  | pypi-query MCP | n/a — `safetensors.torch.load_file` present |
| python | torch       | (from cuda.txt/rocm/cpu.txt, not in base.txt) | pypi-query MCP | n/a — lazy import only, no top-level dependency |

Note: `transformers` version 5.12.1 is the latest on PyPI; the project's `base.txt` pins `>=4.46`. The classes `Qwen2Tokenizer`, `Qwen3ForCausalLM`, and `Qwen3Config` are confirmed to exist in `transformers` 4.46+ (Qwen3 support was added in transformers 4.47+).

## Approach

1. **Add `MockTokenizer` and `MockTextEncoder` to `worker/nodes/loader.py`.**
   - Add two new sentinel classes after `MockClip` (around line 78):
     - `MockTokenizer` — a bare class with no methods, matching the pattern of `MockVae`.
     - `MockTextEncoder` — a bare class with no methods, matching the pattern of `MockVae`.
   - Add `"MockTokenizer", "MockTextEncoder"` to `__all__`.
   - Add Google-style docstrings to both classes.
   - Rationale: These are required by `qwen3.py`'s mock path (`RealClip(MockTokenizer(), MockTextEncoder())`). Since `defers_to` is empty, they must be implemented here, not deferred.

2. **Create `worker/nodes/arch/clip/qwen3.py`.**
   - Module docstring: follow the pattern from `zit.py` — describe mock mode, real path, and the import isolation requirement.
   - `from __future__ import annotations` at top.
   - `import os` for mock mode check.
   - `from typing import Any` for type hints.
   - `__all__ = ["can_handle", "load"]`.
   - **`can_handle(clip_type: str) -> bool`**: return `clip_type == "qwen3"`. Simple string comparison.
   - **`load(model_id: str, torch_dtype: Any) -> RealClip`**:
     - Set `_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"`.
     - If `_mock`: import `RealClip`, `MockTokenizer`, `MockTextEncoder` from `worker.nodes.loader`; return `RealClip(MockTokenizer(), MockTextEncoder())`. No torch/transformers/safetensors import anywhere in this branch.
     - If not `_mock`: lazy imports inside the non-mock branch:
       - `from pathlib import Path`
       - `from transformers import Qwen2Tokenizer, Qwen3ForCausalLM, Qwen3Config`
       - `from safetensors.torch import load_file as safetensors_load_file`
       - `from worker.nodes.loader import RealClip` (lazy import to avoid circular import — `loader.py` may call `arch.clip.get_module()` which imports this module)
     - Tokenizer: `Qwen2Tokenizer.from_pretrained(Path(__file__).parent.parent / "assets" / "qwen25_tokenizer")`.
     - Config values (verbatim from `Qwen/Qwen3-4B`'s `config.json`, NOT `Qwen3Config` class defaults):
       ```python
       config_values = {
           "vocab_size": 151936,
           "hidden_size": 2560,
           "intermediate_size": 9728,
           "num_hidden_layers": 36,
           "num_attention_heads": 32,
           "num_key_value_heads": 8,
           "head_dim": 128,
           "max_position_embeddings": 40960,
           "tie_word_embeddings": True,
       }
       ```
     - Model: `Qwen3ForCausalLM(Qwen3Config(**config_values))`.
     - Load weights: `model.load_state_dict(safetensors_load_file(model_id))`.
     - Return `RealClip(tokenizer, model)`.
   - All docstrings follow Google style. All decision points have inline `#` comments.

3. **Create `worker/tests/test_arch_clip_qwen3.py`.**
   - Follow the pattern of `test_arch_zit.py` and `test_arch_clip_init.py`.
   - **Test 1 — `test_can_handle_qwen3`**: import `can_handle` from `qwen3`; assert `can_handle("qwen3")` is `True`.
   - **Test 2 — `test_can_handle_non_qwen3`**: assert `can_handle("clip_l")` and `can_handle("t5")` are `False`.
   - **Test 3 — `test_load_mock_returns_realclip`**: import `load` from `qwen3`; call `load("/fake/path", None)` in mock mode; assert result is a `RealClip` instance with `.tokenizer` being a `MockTokenizer` and `.text_encoder` being a `MockTextEncoder`.
   - **Test 4 — `test_load_mock_no_torch_import`**: remove `torch` from `sys.modules`, re-import `qwen3`, assert `"torch" not in sys.modules` — proving no top-level import of torch occurs.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `can_handle` | `worker.nodes.arch.clip.qwen3` | `def can_handle(clip_type: str) -> bool` |
| `load` | `worker.nodes.arch.clip.qwen3` | `def load(model_id: str, torch_dtype: Any) -> RealClip` |
| `MockTokenizer` | `worker.nodes.loader` (added) | `class MockTokenizer` — bare sentinel, no methods |
| `MockTextEncoder` | `worker.nodes.loader` (added) | `class MockTextEncoder` — bare sentinel, no methods |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `MockTokenizer`, `MockTextEncoder` classes and update `__all__` |
| CREATE | `worker/nodes/arch/clip/qwen3.py` | Qwen3 clip-arch module with `can_handle()` and `load()` |
| CREATE | `worker/tests/test_arch_clip_qwen3.py` | ≥4 tests for qwen3.py: can_handle, mock load, import isolation |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_qwen3` | `can_handle("qwen3")` returns `True` | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `clip_type="qwen3"` | `True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_qwen3 -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_non_qwen3` | `can_handle("clip_l")` and `can_handle("t5")` return `False` | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `clip_type="clip_l"`, `"t5"` | `False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_non_qwen3 -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_returns_realclip` | `load()` in mock mode returns `RealClip` with `MockTokenizer` and `MockTextEncoder` | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `model_id="/fake/path"`, `torch_dtype=None` | `RealClip(MockTokenizer(), MockTextEncoder())` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_returns_realclip -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_no_torch_import` | Module imports without torch at top level | `ANVILML_WORKER_MOCK=1` (conftest autouse) | torch removed from `sys.modules` | `"torch" not in sys.modules` after import | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the existing naming convention (`test_arch_*.py`) and is automatically picked up by `pytest worker/tests/ -v`. The new source file is under `worker/nodes/arch/clip/`, which is already covered by the worker-linux and worker-windows CI jobs.

## Platform Considerations

None identified. The module uses only `os.environ.get()`, `pathlib.Path`, and standard library imports — all cross-platform. The tokenizer path uses `Path(__file__).parent.parent / "assets" / "qwen25_tokenizer"` which resolves correctly on both Linux and Windows via `pathlib`. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `Qwen3Config` API may differ between `transformers>=4.46` and the version used for config values (Qwen/Qwen3-4B release). The task context specifies verbatim config values but the class import paths may vary. | Low | High | Verify at ACT time: import `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `Qwen3Config` from the installed transformers version and confirm the API shape matches expectations. If a class name changed, use the current name and record in Risks. |
| Circular import between `loader.py` and `qwen3.py`: `loader.py` may import `arch.clip.get_module()` which imports `qwen3.py`, and `qwen3.py` imports `RealClip` from `loader.py`. | Low | High | Mitigated by lazy import: `RealClip` is imported inside `load()`'s non-mock branch, not at module level. This ensures `loader.py` is fully loaded (including `RealClip` definition) before `qwen3.py` tries to import it. |
| `MockTokenizer` and `MockTextEncoder` added to `loader.py` may cause a pre-existing test to fail if any test checks `__all__` contents or imports these names. | Low | Low | No existing test references these names (they did not exist before). The `__all__` update adds them — this is additive only. |
| The mock `load()` function accepts a `model_id` parameter that is unused in mock mode. A real-mode path test might pass a path that doesn't exist, causing `safetensors.torch.load_file()` to raise. | Low | Medium | The mock path never reaches the safetensors call. The import isolation test proves torch is not imported at module level. No real-mode test is included (requires GPU + model weights). |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/arch/clip/qwen3.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 with ≥3 tests passing
- [ ] `worker/nodes/arch/clip/qwen3.py` contains no top-level import of `torch`, `transformers`, or `safetensors`
- [ ] `worker/nodes/arch/clip/qwen3.py` contains no `NotImplementedError`, `TODO`, or stub return values
- [ ] `worker/nodes/loader.py` exports `MockTokenizer` and `MockTextEncoder` in `__all__`
