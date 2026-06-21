# Plan Report: P18-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-B1                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/encoder.py: ClipTextEncode node  |
| Depends on  | P18-A3 (LoadClip), P18-D1 (pipeline_cache)   |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/encoder.py` implementing the `ClipTextEncode` node — a conditioning node that accepts a `CLIP` object, a prompt string, and an optional negative prompt string, then returns a `CONDITIONING` output. In mock mode (`ANVILML_WORKER_MOCK=1`), it returns a lightweight `MockConditioning` sentinel carrying the text. The real path is stubbed with `NotImplementedError`. This task also creates the corresponding test file `worker/tests/test_nodes_encoder.py` with at least 3 passing tests.

## Scope

### In Scope
- Create `worker/nodes/encoder.py` with:
  - `MockConditioning` sentinel class (carries `text` attribute)
  - `@register`-decorated `ClipTextEncode` node class
  - `INPUT_SLOTS = [SlotSpec("clip","CLIP"), SlotSpec("text","STRING"), SlotSpec("negative_text","STRING", optional=True)]`
  - `OUTPUT_SLOTS = [SlotSpec("conditioning","CONDITIONING")]`
  - Mock path: `{"conditioning": MockConditioning(text=text)}`
  - Real path: stub with `NotImplementedError`
  - Docstring on the class and `execute()` method
  - Inline comments on the encoding branch (mock vs real)
  - `__all__` export list
- Create `worker/tests/test_nodes_encoder.py` with ≥ 3 tests:
  - Registry registration test
  - Mock-mode execute test
  - Metadata attributes test
  - Optional: negative_text defaults to empty string

### Out of Scope
- Real safetensors/CLIP encoding implementation (deferred to later phase)
- `pipeline_cache.py` (task P18-D1)
- Architecture dispatch module (task P18-C1)
- EmptyLatent, Sampler, VaeDecode nodes (tasks P18-B2, P18-B3)
- Any Rust-side changes

## Existing Codebase Assessment

The node registration infrastructure is fully in place. `worker/nodes/base.py` defines `BaseNode` (ABC with `execute()` abstract method), `NodeContext`, `SlotSpec`, and the `@register` decorator. `worker/nodes/__init__.py` auto-imports all sibling modules via `pkgutil.iter_modules()`, making node registration automatic on import.

The loader module (`worker/nodes/loader.py`) establishes the established pattern: `MockModel`, `MockVae`, and `MockClip` sentinel classes; `@register`-decorated node classes with all six metadata attributes; mock-mode branching via `os.environ.get("ANVILML_WORKER_MOCK") == "1"`; real-mode stubs raising `NotImplementedError`; Google-style docstrings; inline comments at decision points; `__all__` exports.

No `encoder.py` or `MockConditioning` class exists yet — this task creates them from scratch. The test file `worker/tests/test_nodes_loader.py` shows the established test pattern: `registry_clean` fixture to clear `NODE_REGISTRY`, `mock_context` fixture to build a `NodeContext`, `importlib.reload()` to re-execute modules against the clean registry, and assertions on `NODE_REGISTRY` membership, `execute()` return values, and metadata attributes.

## Resolved Dependencies

None. This task introduces no external packages. All dependencies are existing Python stdlib (`os`, `typing`, `importlib`, `pkgutil`, `pytest`) already available in the worker venv.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Create `worker/nodes/encoder.py`** with the following structure:
   - Module docstring explaining the file's purpose, the mock/real split, and the no-top-level-import constraint (mirroring `loader.py`'s docstring style).
   - `from __future__ import annotations` import.
   - `import os` and `from typing import Any` imports.
   - `from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register` import.
   - `__all__ = ["ClipTextEncode", "MockConditioning"]` export list.
   - **`MockConditioning` class**: A simple sentinel dataclass/class with a single `text: str` attribute. Docstring explaining it carries the encoded text output for downstream nodes to inspect without a real conditioning object. `__init__` accepting `text` parameter.
   - **`@register class ClipTextEncode(BaseNode)`**: Six metadata attributes (`NODE_TYPE = "ClipTextEncode"`, `CATEGORY = "Conditioning"`, `DISPLAY_NAME = "Clip Text Encode"`, `DESCRIPTION = "Encode a text prompt using a loaded CLIP encoder"`, `INPUT_SLOTS`, `OUTPUT_SLOTS` as specified). `execute(self, **inputs: Any)` method that reads `clip` and `text` from inputs, checks mock mode, returns `{"conditioning": MockConditioning(text=text)}` in mock mode, and raises `NotImplementedError` in real mode. Docstring on the class and on `execute()`. Inline comment on the mock-mode branch explaining why the sentinel is returned. Inline comment on the real-mode stub referencing the future real implementation.

2. **Create `worker/tests/test_nodes_encoder.py`** with the following structure:
   - Module docstring mirroring `test_nodes_loader.py` style.
   - `registry_clean` fixture (same as loader tests) — clears `NODE_REGISTRY`.
   - `mock_context` fixture (same as loader tests) — builds `NodeContext` with captured emit.
   - **Test 1** — `test_cliptextencode_registered_in_registry`: Re-import and reload `worker.nodes.encoder`, assert `"ClipTextEncode"` is in `NODE_REGISTRY`, assert `NODE_REGISTRY["ClipTextEncode"] is ClipTextEncode`, assert `NODE_TYPE == "ClipTextEncode"`.
   - **Test 2** — `test_cliptextencode_execute_returns_mock_conditioning`: Instantiate `ClipTextEncode(mock_context)`, call `execute(clip=MockClip(), text="a cat sitting on a fence")`, assert `"conditioning"` in result, assert `isinstance(result["conditioning"], MockConditioning)`, assert `result["conditioning"].text == "a cat sitting on a fence"`.
   - **Test 3** — `test_cliptextencode_metadata_attributes`: Assert `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION` (non-empty string), `INPUT_SLOTS` (3 specs: clip CLIP required, text STRING required, negative_text STRING optional), `OUTPUT_SLOTS` (1 spec: conditioning CONDITIONING required).
   - **Test 4** — `test_cliptextencode_negative_text_defaults_to_empty`: Call `execute(clip=MockClip(), text="hello")` without `negative_text`, assert `"conditioning"` in result and `result["conditioning"].text == "hello"` (negative text is unused in mock path but the input should be accepted without error).

3. **Verify syntax**: Run `worker/.venv/bin/python -m py_compile worker/nodes/encoder.py worker/tests/test_nodes_encoder.py` to confirm no syntax errors before running pytest.

4. **Run tests**: `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py -v` — must exit 0 with ≥ 3 tests passing.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `class MockConditioning` | `worker.nodes.encoder` | Sentinel class with `text: str` attribute for mock-mode conditioning output |
| `class ClipTextEncode(BaseNode)` | `worker.nodes.encoder` | Registered node: encodes text prompts using a CLIP object, outputs CONDITIONING |
| `ClipTextEncode.NODE_TYPE` | `worker.nodes.encoder.ClipTextEncode` | `"ClipTextEncode"` |
| `ClipTextEncode.CATEGORY` | `worker.nodes.encoder.ClipTextEncode` | `"Conditioning"` |
| `ClipTextEncode.DISPLAY_NAME` | `worker.nodes.encoder.ClipTextEncode` | `"Clip Text Encode"` |
| `ClipTextEncode.DESCRIPTION` | `worker.nodes.encoder.ClipTextEncode` | `"Encode a text prompt using a loaded CLIP encoder"` |
| `ClipTextEncode.INPUT_SLOTS` | `worker.nodes.encoder.ClipTextEncode` | `[SlotSpec("clip","CLIP"), SlotSpec("text","STRING"), SlotSpec("negative_text","STRING",optional=True)]` |
| `ClipTextEncode.OUTPUT_SLOTS` | `worker.nodes.encoder.ClipTextEncode` | `[SlotSpec("conditioning","CONDITIONING")]` |
| `ClipTextEncode.execute(**inputs)` | `worker.nodes.encoder.ClipTextEncode` | Returns `{"conditioning": MockConditioning(text=text)}` in mock mode |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/encoder.py` | ClipTextEncode node + MockConditioning sentinel |
| CREATE | `worker/tests/test_nodes_encoder.py` | Unit tests for ClipTextEncode node |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_encoder.py` | `test_cliptextencode_registered_in_registry` | ClipTextEncode is registered in NODE_REGISTRY after import | NODE_REGISTRY cleared by `registry_clean` fixture; `worker.nodes.encoder` imported and reloaded | None | `"ClipTextEncode"` in NODE_REGISTRY, `NODE_TYPE == "ClipTextEncode"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_cliptextencode_execute_returns_mock_conditioning` | `execute()` returns MockConditioning with correct text in mock mode | `ANVILML_WORKER_MOCK=1` set by conftest; NODE_REGISTRY cleared | `clip=MockClip()`, `text="a cat sitting on a fence"` | `result["conditioning"]` is `MockConditioning(text="a cat sitting on a fence")` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_cliptextencode_metadata_attributes` | All six required metadata attributes have correct values | None (direct import) | None | NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS (3), OUTPUT_SLOTS (1) all match spec | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_cliptextencode_negative_text_defaults_to_empty` | `execute()` accepts inputs without `negative_text` without error | `ANVILML_WORKER_MOCK=1` set by conftest; NODE_REGISTRY cleared | `clip=MockClip()`, `text="hello"` (no negative_text) | `result["conditioning"]` is `MockConditioning(text="hello")` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty -v` exits 0 |

## CI Impact

No CI changes required. The new `worker/tests/test_nodes_encoder.py` test file is picked up automatically by the existing `worker-linux` and `worker-windows` CI jobs which run `pytest worker/tests/ -v`. The new `worker/nodes/encoder.py` module is auto-imported by `__init__.py`'s `_ensure_imported()` scan, so no CI configuration changes are needed.

## Platform Considerations

None identified. The module uses only `os.environ.get()` and standard Python types — no platform-specific APIs, path separators, or line-ending handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `MockConditioning` text attribute is accessed by downstream nodes (Sampler, VaeDecode) expecting a different attribute name or structure | Low | Medium | The mock conditioning object is only returned by the mock path; downstream mock nodes in later tasks (P18-B2, P18-B3) will receive it. Document the `text` attribute in `MockConditioning`'s docstring so future mock nodes know what to expect. No action needed now — this is a contract for future tasks. |
| Test module reload race: `importlib.reload(worker.nodes.encoder)` after `NODE_REGISTRY.clear()` may fail if another test already imported encoder | Low | Low | Follow the established pattern from `test_nodes_loader.py`: import the module first, then call `importlib.reload()`. The `registry_clean` fixture clears the registry before each test. This is the exact pattern used successfully by all loader tests. |
| `negative_text` input is not used in mock path but is listed in INPUT_SLOTS as optional — a future real implementation must handle it | Low | Low | The mock path ignores `negative_text` entirely (consistent with how `LoadModel` ignores `model_id` in mock mode). Add an inline comment noting that `negative_text` is passed through in the real path to `clip.encode(text, negative_text)`. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/encoder.py worker/tests/test_nodes_encoder.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py -v` exits 0 with ≥ 3 tests passing
- [ ] `grep -c "ClipTextEncode" worker/nodes/encoder.py` returns ≥ 5 (class defined, registered, referenced in docstrings)
- [ ] `grep -c "MockConditioning" worker/nodes/encoder.py` returns ≥ 3 (class defined, used in execute, in __all__)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` still exits 0 (no regression in existing loader tests)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full test suite green)
