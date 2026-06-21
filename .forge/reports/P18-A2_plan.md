# Plan Report: P18-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-A2                                        |
| Phase       | 018 — ZiT Generic Nodes                       |
| Description | worker/nodes/loader.py: LoadVae node          |
| Depends on  | P18-A1 (LoadModel in loader.py)               |
| Project     | anvilml                                       |
| Planned at  | 2026-06-21T11:45:00Z                          |
| Attempt     | 1                                             |

## Objective

Add the `LoadVae` node to `worker/nodes/loader.py` alongside the existing `LoadModel` node. The `LoadVae` node accepts a `model_id` STRING input and outputs a `VAE` slot. In mock mode (`ANVILML_WORKER_MOCK=1`) it returns a lightweight `MockVae` sentinel. In real mode it is stubbed with `NotImplementedError` pending `pipeline_cache.py` (P18-D1). This completes Group A of Phase 018's loader nodes and enables the VAE wire in the ZiT workflow graph.

## Scope

### In Scope
- Add `MockVae` sentinel class to `worker/nodes/loader.py` (mirrors `MockModel` pattern, no attributes needed).
- Add `LoadVae` node class to `worker/nodes/loader.py` with `@register` decorator, six metadata attributes, and `execute()` method.
- Update `__all__` in `loader.py` to export `LoadVae` and `MockVae`.
- Add `TestLoadVae` test class to `worker/tests/test_nodes_loader.py` with ≥ 3 tests:
  1. `test_loadvae_registered_in_registry` — verifies `LoadVae` is in `NODE_REGISTRY`.
  2. `test_loadvae_execute_returns_mock_vae` — verifies `execute()` returns `{"vae": MockVae()}` in mock mode.
  3. `test_loadvae_metadata_attributes` — verifies all six metadata attributes.

### Out of Scope
- Real safetensors loading path for VAE (deferred to a future task that also implements `pipeline_cache.py` in P18-D1).
- `LoadClip` node (task P18-A3).
- Any changes to `worker/nodes/__init__.py` (auto-import already handles new `.py` modules).
- Any Rust-side changes.
- Any changes to `docs/TESTS.md` (handled by ACT agent per FORGE_AGENT_RULES §5.10, but this is a PLAN session).

## Existing Codebase Assessment

**What exists:** `worker/nodes/loader.py` already contains the `LoadModel` node and `MockModel` sentinel class, both established as the pattern for loader nodes in Phase 018. The `LoadModel` node uses `@register` to auto-register in `NODE_REGISTRY`, defines all six metadata attributes, checks `ANVILML_WORKER_MOCK` at runtime inside `execute()`, and returns a sentinel in mock mode while stubbing the real path with `NotImplementedError`.

**Established patterns:**
- Sentinel classes (`MockModel`) carry only the attributes needed by downstream consumers (`arch` attribute for architecture dispatch).
- The mock mode check is `os.environ.get("ANVILML_WORKER_MOCK") == "1"` inside `execute()`, not at import time.
- Real path is stubbed with `NotImplementedError` referencing the future task (e.g., `TODO(P18-A1)`).
- Tests use `importlib.reload()` to re-execute module bodies against a cleared `NODE_REGISTRY` fixture.
- Tests follow a three-test pattern per node: registry registration, mock execution, metadata attributes.

**Gap:** `pipeline_cache.py` does not yet exist (it will be created in P18-D1). The VAE real-loading path cannot be fully implemented without it. The plan stubs the real path with `NotImplementedError`, consistent with how `LoadModel` handles its real path.

## Resolved Dependencies

None. This task introduces no new Python packages or Rust crates. All dependencies are already present in the project:
- `os` (stdlib) — mock mode check
- `worker.nodes.base` (local) — `BaseNode`, `SlotSpec`, `NodeContext`, `register`
- `worker.nodes` (local) — `NODE_REGISTRY`

## Approach

1. **Add `MockVae` class to `loader.py`.** Create a simple sentinel class below `MockModel` with no required attributes (VAE objects in the real path will have their own structure defined later). Include a Google-style docstring. Add `"MockVae"` to `__all__`.

2. **Add `LoadVae` node class to `loader.py`.** Create the class below `LoadModel` with:
   - `@register` decorator (placed before the class definition, same as `LoadModel`).
   - Metadata attributes:
     - `NODE_TYPE = "LoadVae"`
     - `CATEGORY = "Loaders"`
     - `DISPLAY_NAME = "Load VAE"`
     - `DESCRIPTION = "Load a VAE from a standalone safetensors file"`
     - `INPUT_SLOTS = [SlotSpec("model_id", "STRING")]`
     - `OUTPUT_SLOTS = [SlotSpec("vae", "VAE")]`
   - `execute(**inputs)` method that:
     - Reads `model_id` from inputs (same pattern as `LoadModel`).
     - Checks `ANVILML_WORKER_MOCK` env var.
     - In mock mode: returns `{"vae": MockVae()}`.
     - In real mode: stubs with `NotImplementedError` referencing that `pipeline_cache.py` (P18-D1) is required.
   - Google-style docstrings on the class and method.
   - Inline comments explaining the mock mode check and the stubbed real path, consistent with `LoadModel`'s comments.

3. **Update `__all__` in `loader.py`.** Add `"LoadVae"` and `"MockVae"` to the existing `__all__` list.

4. **Add `TestLoadVae` test class to `test_nodes_loader.py`.** Three tests mirroring the `LoadModel` test pattern:
   - `test_loadvae_registered_in_registry`: Re-imports and reloads `worker.nodes.loader`, asserts `"LoadVae"` in `NODE_REGISTRY` and `NODE_REGISTRY["LoadVae"] is LoadVae` and `LoadVae.NODE_TYPE == "LoadVae"`.
   - `test_loadvae_execute_returns_mock_vae`: Instantiates `LoadVae(mock_context)`, calls `execute(model_id="test-vae")`, asserts `result["vae"]` is a `MockVae` instance.
   - `test_loadvae_metadata_attributes`: Asserts all six metadata attributes match expected values (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION` non-empty, `INPUT_SLOTS` has one `SlotSpec("model_id", "STRING")`, `OUTPUT_SLOTS` has one `SlotSpec("vae", "VAE")`).

5. **Pre-flight syntax check.** Before writing the report, verify that `loader.py` and the test file have no syntax errors by running `python -m py_compile` on both files. (This is a PLAN action — the ACT agent will do this properly.)

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `MockVae` | class | `worker.nodes.loader.MockVae` | Sentinel VAE object for mock mode. No required attributes. |
| `LoadVae` | class | `worker.nodes.loader.LoadVae` | Node that loads a VAE from a safetensors file. Decorated with `@register`. |
| `LoadVae.execute(self, **inputs: Any) -> dict[str, Any]` | method | `worker.nodes.loader.LoadVae` | Reads `model_id`, returns `{"vae": MockVae()}` in mock mode. |

No new `pub` items in Rust crates. No external crate additions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `MockVae` class and `LoadVae` node class; update `__all__` |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add `TestLoadVae` test class with ≥ 3 tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadvae_registered_in_registry` | `LoadVae` is registered in `NODE_REGISTRY` after import | `registry_clean` fixture clears registry; `worker.nodes.loader` reloaded | None | `"LoadVae" in NODE_REGISTRY`, `NODE_REGISTRY["LoadVae"] is LoadVae`, `LoadVae.NODE_TYPE == "LoadVae"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_execute_returns_mock_vae` | `execute()` returns `{"vae": MockVae()}` in mock mode | `ANVILML_WORKER_MOCK=1` (autouse fixture); `registry_clean` | `model_id="test-vae"` | `result["vae"]` is `MockVae` instance | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_metadata_attributes` | All six metadata attributes on `LoadVae` have correct values | Direct import of `LoadVae` from `loader` module | None | `NODE_TYPE="LoadVae"`, `CATEGORY="Loaders"`, `DISPLAY_NAME="Load VAE"`, `DESCRIPTION` non-empty, `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]`, `OUTPUT_SLOTS=[SlotSpec("vae", "VAE")]` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. The task modifies existing Python test files and source files that are already covered by the `worker-linux` and `worker-windows` CI jobs. The new tests in `test_nodes_loader.py` will be picked up automatically by `pytest worker/tests/ -v`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The task is platform-neutral: it adds Python code that only uses the `os` standard library and local project imports. No `#[cfg(unix)]` / `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `importlib.reload()` on `loader.py` may re-execute module-level side effects (e.g., `__all__` updates, class definitions) causing stale references in tests that imported before reload. | Low | Medium | Follow the exact same pattern used by existing `LoadModel` tests: re-import the module, call `importlib.reload()`, then re-import the classes from the reloaded module. This ensures the test sees the freshly registered classes. |
| The `MockVae` class has no attributes, which could cause confusion for downstream code that expects VAE objects to have methods (e.g., `decode()`). However, this is intentional — the real `MockVae` will be fleshed out when `pipeline_cache.py` and `VaeDecode` are implemented. | Low | Low | The docstring explicitly states that `MockVae` is a lightweight sentinel and that real VAE objects have their own structure. The test only checks instantiation, not method availability. |
| Modifying `loader.py` (an existing file) could introduce regressions in `LoadModel` tests if the module-level structure changes (e.g., `__all__` order, import ordering). | Low | Medium | The plan only appends new code after existing `LoadModel` code. No existing lines are modified or deleted. The `__all__` update only adds entries. The existing `LoadModel` tests remain unchanged and serve as a regression signal. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_nodes_loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::TestLoadVae -v` exits 0 with ≥ 3 tests
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 (full loader test suite, ensuring LoadModel tests still pass)
