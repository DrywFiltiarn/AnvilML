# Plan Report: P18-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-A3                                            |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | Add LoadClip @register to loader.py with clip_type hint |
| Depends on  | P18-A1, P18-A2                                    |
| Project     | anvilml                                           |
| Planned at  | 2026-06-21T12:30:00Z                              |
| Attempt     | 1                                                   |

## Objective

Add the `LoadClip` node class to `worker/nodes/loader.py` with a `clip_type` optional input slot that selects the tokeniser type (`"qwen3"`, `"clip_l"`, or `"t5"`). In mock mode, return a `MockClip` sentinel carrying the resolved `clip_type`. Create at least 3 unit tests in `worker/tests/test_nodes_loader.py` covering registration, mock-mode execution with default clip_type, and mock-mode execution with explicit clip_type.

## Scope

### In Scope
- Add `MockClip` class to `worker/nodes/loader.py` with a `clip_type` attribute.
- Add `LoadClip` node class decorated with `@register` to `worker/nodes/loader.py`:
  - `NODE_TYPE = "LoadClip"`
  - `CATEGORY = "Loaders"`
  - `DISPLAY_NAME = "Load CLIP"`
  - `DESCRIPTION = "Load a text encoder (CLIP/T5/Qwen3) from a safetensors file"`
  - `INPUT_SLOTS = [SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]`
  - `OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]`
  - `execute()` returns `{"clip": MockClip(clip_type=inputs.get("clip_type", "qwen3"))}` in mock mode.
  - `execute()` raises `NotImplementedError` in non-mock mode (stubbed, same pattern as LoadModel/LoadVae).
- Update `__all__` in `loader.py` to include `"MockClip"`.
- Add `TestLoadClip` test class with ≥ 3 tests to `worker/tests/test_nodes_loader.py`:
  - Registration test (LoadClip in NODE_REGISTRY).
  - Mock-mode execution with default clip_type (`"qwen3"`).
  - Mock-mode execution with explicit clip_type (e.g. `"clip_l"`).
  - Metadata attributes test (all six required attributes).

### Out of Scope
- Real safetensors loading path for text encoders (stubbed, will be implemented in a future task).
- Any changes to `worker/nodes/base.py`, `worker/nodes/__init__.py`, or Rust crates.
- Changes to `docs/TESTS.md` — this is an ACT obligation, not a PLAN obligation.
- Integration tests or parity tests.

## Existing Codebase Assessment

The `worker/nodes/loader.py` module already defines `LoadModel` and `LoadVae` nodes following a consistent pattern: a `MockModel`/`MockVae` sentinel class, a `@register`-decorated `BaseNode` subclass with six metadata attributes, and an `execute()` method that checks `ANVILML_WORKER_MOCK` at runtime to branch between mock and real paths. The real path is stubbed with `NotImplementedError`.

The `worker/nodes/base.py` module defines `BaseNode` (ABC with `execute` abstract method), `SlotSpec` (dataclass with `name`, `slot_type`, `optional`), `NodeContext`, and the `@register` decorator that validates six attributes and stores the class in `NODE_REGISTRY`.

The `worker/tests/test_nodes_loader.py` test file uses `importlib.reload()` to re-execute `loader.py` against a cleared `NODE_REGISTRY` for each test, ensuring clean isolation. Tests follow a consistent pattern: registry test, execute test, metadata test per node. A `registry_clean` fixture and a `mock_context` fixture are shared.

The `worker/tests/conftest.py` sets `ANVILML_WORKER_MOCK=1` via an autouse fixture with proper capture-and-restore semantics.

No `MockClip` or `LoadClip` exists anywhere in the codebase. This task is the first introduction of the CLIP loading path.

## Resolved Dependencies

None. This task introduces no new Python packages or Rust crates. All types used (`BaseNode`, `SlotSpec`, `@register`, `NODE_REGISTRY`) are already defined in the existing `worker/nodes/base.py` module. No MCP lookup required.

## Approach

1. **Add `MockClip` class to `loader.py`.** Add a `MockClip` class after `MockVae` following the same pattern: a lightweight sentinel with a `clip_type` attribute defaulting to `"qwen3"`. The class takes an optional `clip_type` string parameter in its `__init__`.

2. **Add `LoadClip` node class to `loader.py`.** Following the exact same pattern as `LoadModel` and `LoadVae`:
   - Decorate with `@register`.
   - Define metadata attributes: `NODE_TYPE = "LoadClip"`, `CATEGORY = "Loaders"`, `DISPLAY_NAME = "Load CLIP"`, `DESCRIPTION = "Load a text encoder (CLIP/T5/Qwen3) from a safetensors file"`.
   - Define `INPUT_SLOTS = [SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]` — the `clip_type` is optional per the task spec, defaulting to `"qwen3"` in mock mode.
   - Define `OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]`.
   - Implement `execute(**inputs)`: check `os.environ.get("ANVILML_WORKER_MOCK") == "1"`, if true return `{"clip": MockClip(clip_type=inputs.get("clip_type", "qwen3"))}`; else raise `NotImplementedError` with a TODO comment referencing the future real path.
   - Add Google-style docstring to the class and to `execute()`.
   - Add inline comments explaining the mock-mode branch (consistent with existing pattern).

3. **Update `__all__` in `loader.py`.** Append `"MockClip"` to the existing `__all__` list: `__all__ = ["LoadModel", "LoadVae", "MockModel", "MockVae", "MockClip"]`.

4. **Add `TestLoadClip` tests to `test_nodes_loader.py`.** Follow the existing test pattern exactly:
   - `test_loadclip_registered_in_registry`: Re-import and reload `loader.py`, assert `"LoadClip"` is in `NODE_REGISTRY`, assert `NODE_REGISTRY["LoadClip"] is LoadClip`, assert `LoadClip.NODE_TYPE == "LoadClip"`.
   - `test_loadclip_execute_returns_mock_clip_default_type`: Reload loader, instantiate `LoadClip(mock_context)`, call `execute(model_id="test-model")` without `clip_type`, assert returned dict has `"clip"` key, assert it is a `MockClip` with `clip_type == "qwen3"` (default).
   - `test_loadclip_execute_returns_mock_clip_explicit_type`: Reload loader, instantiate `LoadClip(mock_context)`, call `execute(model_id="test-model", clip_type="clip_l")`, assert `clip_type == "clip_l"` on the returned `MockClip`.
   - `test_loadclip_metadata_attributes`: Reload loader, verify all six metadata attributes including `INPUT_SLOTS` has two specs (model_id STRING required, clip_type STRING optional) and `OUTPUT_SLOTS` has one spec (clip CLIP required).

5. **Run syntax check.** Before any test execution, run `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` to confirm no syntax errors.

6. **Run tests.** Execute `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` and confirm ≥ 3 tests pass (existing LoadModel/LoadVae tests + new LoadClip tests).

## Public API Surface

No new Rust pub items. Python-only additions:

| Module | Item | Kind | Signature / Description |
|--------|------|------|------------------------|
| `worker/nodes/loader.py` | `MockClip` | class | `def __init__(self, clip_type: str = "qwen3") -> None` — sentinel CLIP object carrying the tokeniser type hint. |
| `worker/nodes/loader.py` | `LoadClip` | class (decorated with `@register`) | `NODE_TYPE = "LoadClip"`, `INPUT_SLOTS = [SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]`, `OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]`, `def execute(self, **inputs: Any) -> dict[str, Any]` |
| `worker/nodes/loader.py` | `__all__` | tuple (modified) | Extended to include `"MockClip"` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `MockClip` class, `LoadClip` node, update `__all__` |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add `TestLoadClip` test class with ≥ 3 tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadclip_registered_in_registry` | `LoadClip` is registered in `NODE_REGISTRY` after import | `NODE_REGISTRY` cleared by `registry_clean` fixture | Re-import + reload of `loader.py` | `"LoadClip" in NODE_REGISTRY`, `NODE_REGISTRY["LoadClip"] is LoadClip`, `LoadClip.NODE_TYPE == "LoadClip"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_execute_returns_mock_clip_default_type` | `execute()` returns `MockClip(clip_type="qwen3")` when `clip_type` not provided | `ANVILML_WORKER_MOCK=1` from `conftest.py` | `execute(model_id="test-model")` | `result["clip"].clip_type == "qwen3"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_execute_returns_mock_clip_explicit_type` | `execute()` returns `MockClip(clip_type="clip_l")` when explicit `clip_type` provided | `ANVILML_WORKER_MOCK=1` from `conftest.py` | `execute(model_id="test-model", clip_type="clip_l")` | `result["clip"].clip_type == "clip_l"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadclip_metadata_attributes` | All six metadata attributes on `LoadClip` have correct values | `LoadClip` accessible via import | Direct import of `LoadClip` | `NODE_TYPE="LoadClip"`, `CATEGORY="Loaders"`, `INPUT_SLOTS` has 2 specs (model_id STRING required, clip_type STRING optional), `OUTPUT_SLOTS` has 1 spec (clip CLIP required) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. The task modifies existing Python test files and source files under `worker/`, which are already picked up by the `worker-linux` and `worker-windows` CI jobs (`pytest worker/tests/`). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `LoadClip` node is pure Python with no platform-specific code paths. The mock-mode branch uses `os.environ.get()` which is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `importlib.reload()` on `loader.py` re-executes the entire module, causing `@register` to re-add `LoadModel` and `LoadVae` classes on each reload. If `NODE_REGISTRY` is not cleared by the `registry_clean` fixture before the reload, stale entries from prior tests may persist. | Low | Medium | The existing `registry_clean` fixture already clears `NODE_REGISTRY` before each test. This is the same pattern used by all existing tests in the file. No change needed. |
| The `clip_type` default value `"qwen3"` may not match the tokeniser expected by downstream nodes (e.g. `ClipTextEncode`). This is a design choice from the task spec, not an implementation defect. | Low | Low | The task spec explicitly states `clip_type=inputs.get("clip_type", "qwen3")`. The default is a reasonable baseline for the Z-Image Turbo FP8 model. |
| Adding `MockClip` to `__all__` could break imports if another module imports `from worker.nodes.loader import *` and expects only the current set of exports. | Low | Low | No other module in the codebase uses wildcard import from `loader.py`. Only tests import specific names. Verified via grep. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_nodes_loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with ≥ 3 new LoadClip tests passing (total tests ≥ 9: 4 existing LoadModel + 3 existing LoadVae + ≥ 3 new LoadClip)
- [ ] `LoadClip` class is registered in `NODE_REGISTRY` under key `"LoadClip"`
- [ ] `MockClip` class exists with `clip_type` attribute
- [ ] `INPUT_SLOTS` contains `SlotSpec("model_id", "STRING")` and `SlotSpec("clip_type", "STRING", optional=True)`
- [ ] `OUTPUT_SLOTS` contains `SlotSpec("clip", "CLIP")`
