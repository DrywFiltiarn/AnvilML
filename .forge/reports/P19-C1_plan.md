# Plan Report: P19-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P19-C1                                            |
| Phase       | 19 — Model Loading Contract Groundwork            |
| Description | worker/nodes/loader.py: LoadModel node, mock branch only |
| Depends on  | P19-B1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T23:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/nodes/loader.py` with the `LoadModel` node class implementing the exact
slot specification from `ANVILML_DESIGN.md §10.3`: `NODE_TYPE="LoadModel"`,
`CATEGORY="Loaders"`, one string input (`model_id`), one model output (`MODEL`).
The `execute()` method branches on `ctx.mock` at the top per §14.6 — the mock branch
returns a sentinel dict `{"model": {"mock": True, "model_id": inputs["model_id"]}}`
with no real loading. The real branch is a bare `raise NotImplementedError` placeholder
intentionally left incomplete for this task (completed by P19-C2). The class is
`@register`-decorated so it appears in `NODE_REGISTRY`. Ship two tests verifying the
mock sentinel shape and node registry registration.

## Scope

### In Scope
- Create `worker/nodes/loader.py` with the `LoadModel` class per §10.3 spec.
- Implement `execute(self, ctx, **inputs) -> dict` with mock/real branch per §14.6.
- Add `MOCK_PATH_VERIFIED` marker on `execute()` pointing to the mock-mode test.
- Add `REAL_PATH_VERIFIED` marker on `execute()` pointing to a real-mode test stub
  that asserts `NotImplementedError` (the real path raises by design; the test is
  collectible and verifies the expected exception).
- Create `worker/tests/test_nodes_loader.py` with >=2 tests.
- Test 1: mock-mode `execute()` returns the sentinel shape `{"model": {"mock": True, "model_id": ...}}`.
- Test 2: `LoadModel` is present in `NODE_REGISTRY` after importing `worker.nodes.loader`.

### Out of Scope
- Real-mode model loading logic (deferred to P19-C2). This task's real branch is a
  bare `raise NotImplementedError` — no `pipeline_cache.get_or_load()` call, no
  safetensors reading, no arch module dispatch.
- `LoadVae` and `LoadClip` nodes (deferred to P19-C3).
- Any fixture-checkpoint creation (deferred to P19-D1).

## Existing Codebase Assessment

The worker node system is already established from Phase 10. `worker/nodes/base.py`
defines `BaseNode` (ABC with abstract `execute()`), `SlotSpec` dataclass, `NodeContext`
class (with `mock` bool field), and the `@register` decorator that populates
`NODE_REGISTRY`. The `__init__.py` auto-imports all `.py` modules in `nodes/` (skipping
`__init__`, `base`, and packages) to trigger `@register` side effects.

Existing tests in `worker/tests/test_nodes_init.py` confirm the auto-import mechanism
works and `NODE_REGISTRY` contains registered nodes (e.g., `PassThrough`). Test style
uses plain `pytest` functions with Google-style docstrings describing what is verified,
preconditions, and expected outcomes.

No `loader.py` exists yet — this is the first node module to be created. No
`test_nodes_loader.py` exists yet. The project uses `worker/.venv/bin/python` for all
Python commands, and tests are collected by `pytest worker/tests/`.

## Resolved Dependencies

None. This task introduces no new Python packages. It uses only the project's existing
dependencies: `pytest` (test framework), `msgpack` and `pyzmq` (already in
`worker/requirements/base.txt`). No external crate or package version resolution is
needed.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | (none)  | (n/a)           | (n/a)          | (n/a)                  |

## Approach

1. **Create `worker/nodes/loader.py`.**
   - Import `BaseNode`, `SlotSpec`, and `register` from `worker.nodes.base`.
   - Define `class LoadModel(BaseNode):` with these class attributes per §10.3 EXACTLY:
     - `NODE_TYPE = "LoadModel"`
     - `CATEGORY = "Loaders"`
     - `DISPLAY_NAME = "Load Model"` (a descriptive display name for the UI)
     - `DESCRIPTION = "Loads a diffusion model from a safetensors file."` (placeholder
       description; real loading logic is deferred to P19-C2)
     - `INPUT_SLOTS = [SlotSpec("model_id", "STRING")]`
     - `OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]`
   - Add `MOCK_PATH_VERIFIED` and `REAL_PATH_VERIFIED` module-level comment markers
     next to the class, per §10.6 convention. The mock marker names the test function
     that verifies the sentinel shape (mock mode). The real marker names a test that
     asserts the expected `NotImplementedError` — this is a valid, collectible real-mode
     test per the convention's own rule that "a test asserting the expected exception
     is itself a legitimate, collectible real-mode test."
   - Implement `def execute(self, ctx: NodeContext, **inputs) -> dict:` with the
     following structure:
     - Check `ctx.mock` at the very top of the method (per §14.6: "branches on ctx.mock
       at the top").
     - **Mock branch** (when `ctx.mock` is `True`): return
       `{"model": {"mock": True, "model_id": inputs["model_id"]}}`. No real loading,
       no torch import, no file I/O.
     - **Real branch** (when `ctx.mock` is `False`): `raise NotImplementedError`
       with a message indicating the real implementation is deferred to P19-C2.
       This is a bare placeholder — no `pipeline_cache` call, no model loading logic.
   - Decorate the class with `@register` to populate `NODE_REGISTRY`.

2. **Create `worker/tests/test_nodes_loader.py`.**
   - Test 1 (`test_load_model_mock_returns_sentinel`): Import `LoadModel` from
     `worker.nodes.loader`, create a `NodeContext` with `mock=True`, call `execute()`
     with `model_id="test_model"`, assert the returned dict equals
     `{"model": {"mock": True, "model_id": "test_model"}}`. This test verifies the
     mock sentinel shape. It is the test named by the `MOCK_PATH_VERIFIED` marker.
   - Test 2 (`test_load_model_in_registry`): Import `worker.nodes.loader` (which
     triggers `@register` side effect), then assert `"LoadModel"` is in
     `worker.nodes.base.NODE_REGISTRY`. This test verifies the node is discoverable
     by the node system's registry.
   - Test 3 (`test_load_model_real_raises_not_implemented`): Import `LoadModel`,
     create a `NodeContext` with `mock=False`, call `execute()` with any inputs,
     assert `NotImplementedError` is raised. This test verifies the real branch's
     expected exception and is the test named by the `REAL_PATH_VERIFIED` marker.
     Mark with `@pytest.mark.real_mode` so it runs in the real-mode CI job.

3. **Run syntax check.** Before running tests, execute
   `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py`
   to confirm no syntax errors (per ENVIRONMENT.md §6 Step 7).

## Public API Surface

| Item | Location | Description |
|------|----------|-------------|
| `class LoadModel(BaseNode)` | `worker/nodes/loader.py` | New node class with NODE_TYPE="LoadModel", CATEGORY="Loaders" |
| `LoadModel.execute(self, ctx: NodeContext, **inputs) -> dict` | `worker/nodes/loader.py` | Mock branch returns sentinel dict; real branch raises NotImplementedError |
| `LoadModel.NODE_TYPE` | `worker/nodes/loader.py` | Class attr: `"LoadModel"` |
| `LoadModel.CATEGORY` | `worker/nodes/loader.py` | Class attr: `"Loaders"` |
| `LoadModel.INPUT_SLOTS` | `worker/nodes/loader.py` | Class attr: `[SlotSpec("model_id", "STRING")]` |
| `LoadModel.OUTPUT_SLOTS` | `worker/nodes/loader.py` | Class attr: `[SlotSpec("model", "MODEL")]` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/loader.py` | LoadModel node class with mock/real execute branches, @register decorated |
| CREATE | `worker/tests/test_nodes_loader.py` | Tests: mock sentinel shape, registry presence, real-mode NotImplementedError |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_loader.py` | `test_load_model_mock_returns_sentinel` | Mock-mode `execute()` returns `{"model": {"mock": True, "model_id": "test_model"}}` sentinel shape | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_in_registry` | `LoadModel` appears in `NODE_REGISTRY` after importing `worker.nodes.loader` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_real_raises_not_implemented (real)` | Real-mode `execute()` raises `NotImplementedError` with deferred-implementation message | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented -v -m real_mode` exits 0 |

## CI Impact

No CI changes required. Phase 9's P9-F1 already wired the full `worker/tests` suite
for both mock (`-m "not real_mode"`) and real_mode markers. The new test file
`test_nodes_loader.py` is automatically picked up by the existing `pytest worker/tests/`
invocation. The real-mode test is marked `@pytest.mark.real_mode` so it runs in the
`worker-linux-real` and `worker-windows-real` CI jobs.

## Platform Considerations

None identified. The `LoadModel` node's mock branch returns a plain Python dict with no
platform-specific behavior (no file I/O, no path handling, no platform-specific types).
The real branch raises `NotImplementedError` which is platform-neutral. The Windows
cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NodeContext` constructor signature may differ from what the test needs to mock | Low | Medium | Read the actual `NodeContext.__init__` signature in `worker/nodes/base.py` (already done: `def __init__(self, job_id, device, caps, cancel_flag, emit, pipeline_cache, mock)`) and construct the test fixture accordingly. |
| `pytest` marker `real_mode` may not be registered, causing a collection warning or error | Low | Low | The project's `pyproject.toml` or `pytest.ini` registers the `real_mode` marker per ENVIRONMENT.md §11.2. If it is not yet registered, add it — this is a project configuration issue, not a code issue. |
| Auto-import in `__init__.py` may fail if `loader.py` has a top-level import error | Medium | High | Run `py_compile` on the new file before running tests (ENVIRONMENT.md §6 Step 7). The `py_compile` step catches syntax and import errors before any subprocess is spawned. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_in_registry -v` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented -v -m real_mode` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with >=2 tests collected
