# Plan Report: P23-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-E1                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/nodes/loader.py: LoadVae real branch calls zit_vae.py via dispatch |
| Depends on  | P23-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T16:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace LoadVae's `NotImplementedError` placeholder (introduced by P19-C3) with a real code path that dispatches to `arch.vae.get_module("zit_vae")` and calls `.load(path, ctx.caps)` via `pipeline_cache.get_or_load()`, mirroring the established pattern of LoadModel (Phase 20) and LoadClip (Phase 22). Remove the stale `NotImplementedError`-asserting tests, update the `REAL_PATH_VERIFIED` marker to point at the new passing test, and add sufficient real-mode tests so `worker/tests/test_nodes_loader.py` has >=4 real_mode tests that all pass. After this task, all three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`) are fully real, closing the gap Phase 19 opened.

## Scope

### In Scope
- Update `LoadVae.execute()` real branch in `worker/nodes/loader.py` to call `arch.vae.get_module("zit_vae").load(path, ctx.caps)` via `pipeline_cache.get_or_load()`, following the LoadModel/LoadClip pattern exactly.
- Update the `REAL_PATH_VERIFIED` marker on `LoadVae.execute()` to point at the new passing test (remove reference to the old NotImplementedError-asserting test).
- Remove `test_load_vae_real_raises_not_implemented` from `worker/tests/test_nodes_loader.py` (it tests the old placeholder).
- Remove `test_load_vae_real_raises_no_diffusion_arch` from `worker/tests/test_nodes_loader.py` (it tests the old placeholder).
- Remove `test_load_vae_real_cache_key_format` from `worker/tests/test_nodes_loader.py` (it asserts NotImplementedError behavior and cache emptiness — both obsolete after the real branch is implemented).
- Add `test_load_vae_real_loads_zit_vae_fixture` — a real-mode test that calls `LoadVae.execute()` against the P23-A1 fixture (`zit_vae_tiny.safetensors`) and verifies the returned VAE is a `torch.nn.Module` with `.arch == "zit_vae"` and parameters on CPU.
- Add `test_load_vae_real_cache_returns_cached_instance` — a real-mode test that calls `LoadVae.execute()` twice with the same fixture path and verifies the second call returns the same cached object (LRU cache hit).

### Out of Scope
None. This task has an empty `defers_to` field and implements its full scope. No stubs, no placeholders, no deferred functionality.

## Existing Codebase Assessment

**What exists:** The `LoadVae` class in `worker/nodes/loader.py` (lines 102–170) follows the same structural pattern as `LoadModel` and `LoadClip`: a `@register`-decorated `BaseNode` subclass with `execute()` branching on `ctx.mock`. The mock branch (line 155) already returns the sentinel dict `{"vae": {"mock": True, "model_id": inputs["model_id"]}}`. The real branch (lines 156–170) calls `ctx.pipeline_cache.get_or_load()` with a `"vae:{model_id}"` key and a lambda that raises `NotImplementedError("no diffusion arch module registered yet")`.

The `arch/vae/__init__.py` dispatcher already registers `zit_vae` (line 20–23 of `__init__.py`), so `get_module("zit_vae")` will return the module. The `zit_vae.py` `load()` function (line 657) has the signature `load(path: str, caps: dict, device: str = "cpu")` — matching the same two-argument call pattern used by LoadModel and LoadClip (they pass `path` and `caps`; `device` defaults to `"cpu"`).

The fixture `worker/tests/fixtures/zit_vae_tiny.safetensors` exists from P23-A1. The `PipelineCache` class is a simple `OrderedDict`-based LRU cache with `get_or_load(key, loader_fn)` — exceptions in `loader_fn` do not populate the cache.

**Established patterns:**
- Real branch: `from worker.nodes.arch.{family} import get_module` → `module = get_module(key)` → guard `None` → `return {slot: ctx.pipeline_cache.get_or_load(key, lambda: module.load(path, ctx.caps))}`
- Logger: `logger.debug()` at the dispatch call site.
- Marker discipline: `REAL_PATH_VERIFIED` points at a real-mode test; `MOCK_PATH_VERIFIED` points at a mock-mode test. Both markers are updated together when either test changes.
- Test style: real-mode tests use `@pytest.mark.real_mode`, import torch, create a `PipelineCache` instance, and assert the returned object's type, `.arch` attribute, and device placement.

**Gap between design doc and source:** The design doc (§11.3) describes `load()` as taking `(model_id, caps, device)` with three arguments, but the actual `zit_vae.py` implementation has `device` defaulting to `"cpu"`, making it a two-argument call for loader nodes — identical to `zit.py` and `qwen3.py`. The approach follows the actual source, not the design doc's three-argument description.

## Resolved Dependencies

None. This task only modifies existing Python files that already import `torch`, `safetensors`, and `diffusers` at runtime. No new external packages or version pins are introduced. All imports (`worker.nodes.arch.vae.get_module`, `worker.pipeline_cache.PipelineCache`) are to internal modules already present in the codebase.

## Approach

**Step 1 — Replace LoadVae's real branch in `worker/nodes/loader.py`.**

In the `LoadVae.execute()` method, replace lines 156–170 (the `else:` branch that raises `NotImplementedError`) with a real dispatch pattern identical to `LoadModel`'s and `LoadClip`'s:

```python
        else:
            # Real branch: dispatch to the registered VAE arch module.
            # The arch key "zit_vae" matches zit_vae.py's can_handle()
            # contract. get_or_load provides caching: if the same
            # model_id is loaded again, the cached ZiTVaeModel is returned
            # without re-loading. The cache is not modified on exception
            # per the PipelineCache contract.
            from worker.nodes.arch.vae import get_module

            module = get_module("zit_vae")
            if module is None:
                # Defensive guard — zit_vae is imported and appended to
                # _REGISTERED_MODULES in vae/__init__.py (P23-B2),
                # so this should never trigger in normal operation.
                raise RuntimeError(
                    f"no VAE arch module registered for 'zit_vae'; "
                    f"cannot load VAE '{inputs['model_id']}'"
                )

            logger.debug("LoadVae: requesting model_id=%s", inputs["model_id"])
            return {
                "vae": ctx.pipeline_cache.get_or_load(
                    f"vae:{inputs['model_id']}",
                    lambda: module.load(inputs["model_id"], ctx.caps),
                )
            }
```

Also update the docstring for `LoadVae` (line 107): change "raises NotImplementedError pending P20" to "dispatches to the registered VAE architecture module (currently 'zit_vae') via `arch.vae.get_module()` and `pipeline_cache.get_or_load()`."

Update the `execute()` docstring's Returns section: change "In real mode: Raises NotImplementedError" to "In real mode: Dict with key 'vae' containing a `torch.nn.Module` (the loaded ZiTVaeModel)."

Remove the `Raises: NotImplementedError` section from the docstring.

**Step 2 — Update the REAL_PATH_VERIFIED marker.**

Change line 126 from:
```python
    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented
```
to:
```python
    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_real_loads_zit_vae_fixture
```

The `MOCK_PATH_VERIFIED` marker on line 127 stays unchanged.

**Step 3 — Remove obsolete real_mode tests from `worker/tests/test_nodes_loader.py`.**

Delete these three tests (lines 154–258):
- `test_load_vae_real_raises_not_implemented` (lines 154–175)
- `test_load_vae_real_cache_key_format` (lines 207–234)
- `test_load_vae_real_raises_no_diffusion_arch` (lines 237–257)

These tests all assert `NotImplementedError`, which will no longer be raised after step 1.

**Step 4 — Add new real_mode tests.**

Add two new `@pytest.mark.real_mode` tests after the existing `test_load_vae_in_registry` test (after line 204):

**Test 4a — `test_load_vae_real_loads_zit_vae_fixture`:**

```python
@pytest.mark.real_mode
def test_load_vae_real_loads_zit_vae_fixture() -> None:
    """LoadVae.execute() loads the ZiT VAE fixture checkpoint via the real branch.

    Calls LoadVae.execute() with mock=False against the P23-A1 fixture
    path (zit_vae_tiny.safetensors). Verifies the return dict has a 'vae'
    key containing a ZiTVaeModel (torch.nn.Module with .arch == 'zit_vae'),
    confirming the full real loading chain works end-to-end.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"vae": ZiTVaeModel(...)} is returned, not an exception.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "zit_vae_tiny.safetensors"
    )

    node = LoadVae()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(ctx, model_id=fixture_path)

    # Verify the return dict has the expected VAE slot.
    assert "vae" in result
    vae = result["vae"]

    # Verify the returned object is a torch.nn.Module (loaded VAE).
    assert isinstance(vae, torch.nn.Module)

    # Verify the architecture identifier is set correctly.
    assert vae.arch == "zit_vae"

    # Verify parameters are on the real device (not meta).
    for param in vae.parameters():
        assert param.device.type == "cpu"
```

**Test 4b — `test_load_vae_real_cache_returns_cached_instance`:**

```python
@pytest.mark.real_mode
def test_load_vae_real_cache_returns_cached_instance() -> None:
    """LoadVae.execute() returns the cached VAE on a second call with the same model_id.

    Calls LoadVae.execute() twice with mock=False and the same fixture path.
    Verifies that both calls return the same object (the PipelineCache LRU
    cache returned the cached value on the second call rather than reloading).

    This test exercises the real code path and confirms the cache integration
    works correctly for VAE loading.

    Expected outcome: both execute() calls return the identical VAE object.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "zit_vae_tiny.safetensors"
    )

    node = LoadVae()
    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)

    result1 = node.execute(ctx, model_id=fixture_path)
    vae1 = result1["vae"]

    result2 = node.execute(ctx, model_id=fixture_path)
    vae2 = result2["vae"]

    # Both calls must return the same cached object.
    assert vae1 is vae2

    # Verify the cached object is still a valid torch.nn.Module.
    assert isinstance(vae1, torch.nn.Module)
    assert vae1.arch == "zit_vae"
```

**Step 5 — Update the docstring for LoadVae class.**

Change the class docstring (lines 104–110) from:
```
    This node loads a Variational Autoencoder component. In mock mode
    it returns a sentinel dict; in real mode it raises NotImplementedError
    pending P20 which will implement actual safetensors reading and VAE
    arch dispatch.
```
to:
```
    This node loads a Variational Autoencoder component. In mock mode
    it returns a sentinel dict; in real mode it dispatches to the
    registered VAE architecture module (currently "zit_vae") via
    ``arch.vae.get_module()`` and caches the result via
    ``pipeline_cache.get_or_load()``.
```

## Public API Surface

No new public API items are introduced. The only change is to an existing method's implementation:

| Module | Item | Change |
|--------|------|--------|
| `worker.nodes.loader.LoadVae.execute()` | `def execute(self, ctx: NodeContext, **inputs) -> dict` | Real branch: replaces `NotImplementedError` with `arch.vae.get_module("zit_vae").load(path, ctx.caps)` dispatch. Signature unchanged. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Replace LoadVae's real branch NotImplementedError with arch.vae.get_module dispatch; update docstrings; update REAL_PATH_VERIFIED marker |
| Modify | `worker/tests/test_nodes_loader.py` | Remove 3 obsolete NotImplementedError tests; add 2 new real_mode tests (load fixture + cache hit) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_loader.py` | `test_load_vae_real_loads_zit_vae_fixture (real)` | LoadVae.execute() loads `zit_vae_tiny.safetensors` successfully, returns `torch.nn.Module` with `.arch == "zit_vae"` and params on CPU | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode --collect-only -q` (collects), then `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_vae_real_cache_returns_cached_instance (real)` | Second LoadVae.execute() call with same model_id returns cached object (is comparison) | Same as above |
| `worker/tests/test_nodes_loader.py` | `test_load_model_real_loads_zit_fixture (real)` | Existing LoadModel real test still passes (regression check) | Same as above |
| `worker/tests/test_nodes_loader.py` | `test_load_clip_real_loads_qwen3_fixture (real)` | Existing LoadClip real test still passes (regression check) | Same as above |

## CI Impact

No CI changes required. The task only modifies existing test files and source files. The `real_mode` marker is already registered in `worker/pyproject.toml` / `worker/pytest.ini`. The existing CI jobs (`worker-linux-real`, `worker-windows-real`) run `pytest worker/tests/ -v -m real_mode` which will automatically pick up the new tests. No new CI jobs, gates, or file types are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The change is a pure Python logic change with no `#[cfg(unix)]`/`#[cfg(windows)]` guards, no path-separator handling, and no line-ending differences. The `arch.vae.get_module("zit_vae")` import and the `load()` call are platform-neutral (torch CPU path used in CI).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The fixture `zit_vae_tiny.safetensors` may not load cleanly — shape mismatches between the fixture tensors and the constructed `ZiTVaeModel` could cause `load_state_dict` to skip all keys or raise. | Low | High | The fixture was built by P23-A1 specifically to be structurally valid for `zit_vae.py`'s shape inference. The existing `test_load_real_zit_vae_fixture` in `test_arch_vae_zit.py` already exercises the full load path against this fixture — if it passes there, it will pass here too. |
| The `get_module("zit_vae")` call could return `None` if the VAE dispatcher's registration is broken (e.g., zit_vae module fails to import). | Low | Medium | This is a defensive guard identical to LoadModel and LoadClip — it raises a clear `RuntimeError` rather than silently failing. The guard was already verified to be unreachable because `test_arch_vae_init.py` confirms zit_vae is registered in the VAE dispatcher. |
| Removing the 3 old NotImplementedError tests reduces test count below the >=4 real_mode threshold. | Low | Medium | Two new real_mode tests are added (fixture load + cache hit), replacing the three removed tests. Combined with the two existing loader real tests (LoadModel, LoadClip), this yields 4 real_mode tests. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode` exits 0
- [ ] `grep -c "@pytest.mark.real_mode" worker/tests/test_nodes_loader.py` returns >= 4 (count of real_mode tests in the file)
- [ ] `grep "NotImplementedError" worker/nodes/loader.py` returns no matches (the NotImplementedError placeholder is fully removed)
- [ ] `grep "REAL_PATH_VERIFIED:.*test_load_vae_real_loads_zit_vae_fixture" worker/nodes/loader.py` returns one match (marker updated to point at new test)
- [ ] `grep -c "test_load_vae_real_raises" worker/tests/test_nodes_loader.py` returns 0 (old tests removed)
