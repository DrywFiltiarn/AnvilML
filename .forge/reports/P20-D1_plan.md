# Plan Report: P20-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-D1                                      |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/loader.py: LoadModel real branch calls zit.py via dispatch |
| Depends on  | P20-C3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T22:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the `NotImplementedError` placeholder in `LoadModel.execute()`'s real branch with actual architecture dispatch: call `arch.diffusion.get_module("zit").load(path, ctx.caps)` via `pipeline_cache.get_or_load()`. This is the first real, non-raising `LoadModel` execution end-to-end. The stale `REAL_PATH_VERIFIED` marker (pointing at a `NotImplementedError`-asserting test that no longer exists) is updated to point at the new passing test.

## Scope

### In Scope
- Modify `worker/nodes/loader.py`: replace the `NotImplementedError` placeholder in `LoadModel.execute()`'s real branch with a dispatch call to `arch.diffusion.get_module("zit").load(inputs["model_id"], ctx.caps)` wrapped in `pipeline_cache.get_or_load()`.
- Remove three stale real-mode tests from `worker/tests/test_nodes_loader.py` that assert `NotImplementedError`: `test_load_model_real_raises_not_implemented`, `test_load_model_real_cache_key_format`, and `test_load_model_real_raises_no_diffusion_arch`.
- Add one new real-mode test `test_load_model_real_loads_zit_fixture` that calls `LoadModel.execute()` against the P20-A1 fixture path (`zit_tiny.safetensors`), verifies it returns a dict with a `"model"` key containing a loaded `ZiTModel` (a `torch.nn.Module` with `.arch == "zit"`).
- Update the `REAL_PATH_VERIFIED` marker comment on `LoadModel.execute()` to point at the new test.
- Update `docs/TESTS.md` to add the new test entry and remove entries for the deleted tests.

### Out of Scope
None. This task's `defers_to` field is `[]` (absent). No scope is deferred.

## Existing Codebase Assessment

**What already exists:** `LoadModel.execute()` already has the full scaffolding — it calls `ctx.pipeline_cache.get_or_load(inputs["model_id"], loader_fn)` in both mock and real branches. The mock branch returns a sentinel dict. The real branch wraps a `NotImplementedError`-throwing lambda inside `get_or_load`. The `arch.diffusion.__init__.py` already imports `zit` and registers it in `_REGISTERED_MODULES`. The `zit.load()` function is fully implemented (through P20-C3) and accepts `(path, caps, device)` — the `device` defaults to `"cpu"`. The fixture `zit_tiny.safetensors` exists and loads successfully via `zit.load()`.

**Established patterns:** The node system uses `@register` decorator on classes that define `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`. The `execute()` method always branches on `ctx.mock` at the top. The dual-mode parity marker convention requires both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` comment markers next to every `execute()`. Tests use `pytest.mark.real_mode` decorator. The `PipelineCache.get_or_load()` contract: on exception, the cache is not modified and the exception propagates.

**Gap between design doc and source:** None that affects this task. The design doc (§11.3) specifies the four-step loading contract; zit.py implements all four steps. The `get_module("zit")` dispatcher is already wired. The only gap is the `NotImplementedError` placeholder in `loader.py`, which this task closes.

## Resolved Dependencies

None. This task introduces no new external dependencies. It only uses existing imports: `worker.nodes.arch.diffusion.get_module` (local module), `torch` (already a dependency), `worker.pipeline_cache.PipelineCache` (local module). No MCP verification needed.

## Approach

### Step 1: Modify LoadModel.execute() real branch in worker/nodes/loader.py

Replace the real branch of `LoadModel.execute()` (lines 67–82) with actual architecture dispatch:

```python
else:
    # Real branch: dispatch to the registered diffusion arch module.
    # The arch key "zit" matches zit.py's can_handle("zit") contract.
    # get_or_load provides caching: if the same model_id is loaded
    # again, the cached ZiTModel is returned without re-loading.
    from worker.nodes.arch.diffusion import get_module

    module = get_module("zit")
    if module is None:
        raise RuntimeError(
            f"no diffusion arch module registered for 'zit'; "
            "cannot load model '{inputs['model_id']}'"
        )

    return {
        "model": ctx.pipeline_cache.get_or_load(
            inputs["model_id"],
            lambda: module.load(inputs["model_id"], ctx.caps),
        )
    }
```

Key details:
- `get_module("zit")` returns the zit module (registered in `__init__.py`).
- `module.load(inputs["model_id"], ctx.caps)` calls `zit.load(path, caps)` — the `device` parameter is omitted, defaulting to `"cpu"` (the fixture loads on CPU).
- `ctx.caps` is the `NodeContext.caps` dict with capability flags (`bf16`, `fp8`, etc.).
- The `get_or_load` wrapper provides caching per the `PipelineCache` contract.
- If `get_module("zit")` returns `None` (no module registered), we raise `RuntimeError` — this should not happen after P20-B2, but guards against future regressions.

Update the `REAL_PATH_VERIFIED` marker comment on line 37 from:
```
# REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented
```
to:
```
# REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture
```

Update the docstring on `LoadModel` class (lines 13–27) to remove the "raises NotImplementedError" language and reflect that real mode now loads models.

### Step 2: Remove stale real-mode tests from worker/tests/test_nodes_loader.py

Remove these three test functions (lines 55–76, 108–135, 138–158):
- `test_load_model_real_raises_not_implemented` — the canonical stale test the old marker pointed at.
- `test_load_model_real_cache_key_format` — asserts NotImplementedError; no longer relevant.
- `test_load_model_real_raises_no_diffusion_arch` — asserts NotImplementedError; no longer relevant.

These tests all assert `NotImplementedError` which the code no longer raises. Removing them prevents test failures and keeps the test catalogue clean.

### Step 3: Add new real-mode test test_load_model_real_loads_zit_fixture

Add a new `@pytest.mark.real_mode` test that exercises the real loading path end-to-end:

```python
@pytest.mark.real_mode
def test_load_model_real_loads_zit_fixture() -> None:
    """LoadModel.execute() loads the ZiT fixture checkpoint via the real branch.

    Calls LoadModel.execute() with mock=False against the P20-A1 fixture
    path (zit_tiny.safetensors). Verifies the return dict has a "model"
    key containing a ZiTModel (torch.nn.Module with .arch == "zit"),
    confirming the full real loading chain works end-to-end.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"model": ZiTModel(...)} is returned, not an exception.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(Path(__file__).parent / "fixtures" / "zit_tiny.safetensors")

    node = LoadModel()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(ctx, model_id=fixture_path)

    # Verify the return dict has the expected MODEL slot.
    assert "model" in result
    model = result["model"]

    # Verify the returned object is a torch.nn.Module (loaded model).
    assert isinstance(model, torch.nn.Module)

    # Verify the architecture identifier is set correctly.
    assert model.arch == "zit"

    # Verify parameters are on the real device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"
```

This is the test that the `REAL_PATH_VERIFIED` marker now points to. It is the mock-mode counterpart's partner — the mock test (`test_load_model_mock_returns_sentinel`) exercises the mock branch, this test exercises the real branch. Together they satisfy the dual-mode parity marker convention for `LoadModel.execute()`.

### Step 4: Update docs/TESTS.md

Add an entry for the new test `test_load_model_real_loads_zit_fixture` and remove entries for the three deleted tests. Follow the format defined in `ANVILML_DESIGN.md §17.1`.

## Public API Surface

None. This task modifies an existing private method implementation (`LoadModel.execute()`) but does not introduce any new public items. The `execute()` signature remains unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/loader.py | Replace LoadModel real branch NotImplementedError with arch.diffusion.get_module("zit").load() dispatch; update REAL_PATH_VERIFIED marker; update docstrings |
| MODIFY | worker/tests/test_nodes_loader.py | Remove 3 stale NotImplementedError tests; add test_load_model_real_loads_zit_fixture |
| MODIFY | docs/TESTS.md | Add new test entry; remove 3 deleted test entries |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| worker/tests/test_nodes_loader.py | test_load_model_real_loads_zit_fixture (real) | LoadModel.execute() against P20-A1 fixture path succeeds, returns ZiTModel with .arch=="zit", params on cpu | Fixture zit_tiny.safetensors exists; torch CPU available | model_id=zit_tiny.safetensors path | {"model": ZiTModel} with model.arch=="zit" | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture -v` exits 0 |
| worker/tests/test_nodes_loader.py | test_load_model_mock_returns_sentinel (mock) | LoadModel.execute() mock branch returns sentinel dict | None | model_id="test_model" | {"model": {"mock": True, "model_id": "test_model"}} | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel -v` exits 0 |

Note: The three removed tests (`test_load_model_real_raises_not_implemented`, `test_load_model_real_cache_key_format`, `test_load_model_real_raises_no_diffusion_arch`) are not listed — they are deleted, not introduced.

The dual-mode parity convention for `LoadModel.execute()` is satisfied by:
- `test_load_model_mock_returns_sentinel` (mock) — `MOCK_PATH_VERIFIED` marker
- `test_load_model_real_loads_zit_fixture` (real) — `REAL_PATH_VERIFIED` marker

## CI Impact

No CI changes required. The test file is already picked up by the existing `worker-linux-real` and `worker-windows-real` CI jobs (which run `pytest worker/tests/ -v -m real_mode`). The removed tests were real-mode-marked and will no longer be collected; the new test is real-mode-marked and will be collected. The total real-mode test count in this file changes from 9 to 7 (9 - 3 + 1), which is well above the >=4 minimum.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The change is purely Python-level dispatch logic with no `#[cfg(unix)]` / `#[cfg(windows)]` guards. The fixture path uses `Path(__file__).parent / "fixtures"` which handles platform path separators automatically.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_module("zit")` returns `None` if zit is not registered, causing `RuntimeError` at runtime. | Low | Medium | zit.py is imported and appended to `_REGISTERED_MODULES` in `arch/diffusion/__init__.py` (P20-B2). A test `test_get_module_returns_zit_for_matching_key` in test_arch_zit.py confirms this. The guard is defensive — it should never trigger. |
| The fixture path `zit_tiny.safetensors` may not contain keys that map correctly to the ZiTModel's state_dict, causing partial load or silent weight mismatches. | Low | Low | The fixture was built by P20-A1 specifically for this purpose. The `test_load_real_zit_fixture` test in test_arch_zit.py already exercises `zit.load()` against this same fixture and passes. Shape mismatches are logged as debug and skipped (not errors). |
| Removing the three NotImplementedError tests may leave a gap in cache-key verification. | Low | Low | The new `test_load_model_real_loads_zit_fixture` test implicitly verifies the cache key: it calls `execute(model_id=fixture_path)` which passes through `get_or_load(inputs["model_id"], ...)`. If the cache key were wrong, the fixture would not be found on subsequent calls (not tested here, but the direct call proves the path is correct). The `test_load_model_real_cache_key_format` was testing cache key format for LoadModel specifically — this is verified by the successful execution path. |
| The REAL_PATH_VERIFIED marker points at a test that doesn't exist yet (written in the same task). | Low | Medium | The marker is updated in the same file edit as the test is written. The acceptance check (`pytest --collect-only`) will confirm the test exists and is collectible before staging. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture -v` exits 0
- [ ] `grep -c "REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture" worker/nodes/loader.py` outputs `1`
- [ ] `grep -c "test_load_model_real_raises_not_implemented" worker/tests/test_nodes_loader.py` outputs `0`
- [ ] `grep -c "test_load_model_real_cache_key_format" worker/tests/test_nodes_loader.py` outputs `0`
- [ ] `grep -c "test_load_model_real_raises_no_diffusion_arch" worker/tests/test_nodes_loader.py` outputs `0`
