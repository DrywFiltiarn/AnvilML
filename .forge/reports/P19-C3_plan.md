# Plan Report: P19-C3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P19-C3                                      |
| Phase       | 019 — Model Loading Contract Groundwork     |
| Description | worker/nodes/loader.py: LoadVae, LoadClip node skeletons (mock-mode only) |
| Depends on  | P19-C2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T09:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add `LoadVae` and `LoadClip` node classes to `worker/nodes/loader.py`, completing the loader node trio alongside the existing `LoadModel`. Each node follows the identical mock/real-placeholder pattern established by `LoadModel` in P19-C1/P19-C2: mock-mode returns a sentinel dict with no real loading, and real-mode delegates to `pipeline_cache.get_or_load()` with a lambda that raises `NotImplementedError` (no concrete arch modules registered yet). Both nodes carry the mandatory `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker pair and are registered via `@register`. The acceptance criterion is `>=8 new tests` in `test_nodes_loader.py` covering both mock and real paths for each node, with both present in `NODE_REGISTRY`, and the full test file reaching `>=12 tests total` exiting 0.

## Scope

### In Scope
- Add `LoadVae` class to `worker/nodes/loader.py` with `NODE_TYPE="LoadVae"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]`, `OUTPUT_SLOTS=[SlotSpec("vae", "VAE")]`.
- Add `LoadClip` class to `worker/nodes/loader.py` with `NODE_TYPE="LoadClip"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]`, `OUTPUT_SLOTS=[SlotSpec("clip", "CLIP")]`.
- Both classes decorated with `@register` and both carry `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers.
- Each node's `execute()` branches on `ctx.mock`: mock returns sentinel dict; real calls `pipeline_cache.get_or_load()` with a lambda that raises `NotImplementedError("no diffusion arch module registered yet")`.
- Add `>=8 new tests` to `worker/tests/test_nodes_loader.py` covering mock and real-mode paths for both nodes, plus registry and cache-key tests.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No deferrals.

## Existing Codebase Assessment

The existing `LoadModel` class in `worker/nodes/loader.py` (82 lines) establishes the exact pattern this task replicates: a `@register`-decorated `BaseNode` subclass with `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`, and an `execute()` method that branches on `ctx.mock`. The mock branch returns a sentinel dict `{"mock": True, "model_id": inputs["model_id"]}`; the real branch calls `ctx.pipeline_cache.get_or_load()` with a generator-based lambda that throws `NotImplementedError`. Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers are present as module-level comments next to `execute()`.

The `NodeContext` class (in `base.py`) provides `mock`, `pipeline_cache`, `job_id`, `device`, `caps`, `cancel_flag`, and `emit` — all used identically across nodes. The `PipelineCache` class (in `pipeline_cache.py`) is already implemented (from P19-B1) with `get_or_load(key, loader_fn)` supporting LRU caching, exception-safe (exceptions do not populate the cache), and O(1) operations via `OrderedDict`.

The existing test file `test_nodes_loader.py` has 5 tests: `test_load_model_mock_returns_sentinel`, `test_load_model_real_raises_not_implemented`, `test_load_model_in_registry`, `test_load_model_real_cache_key_format`, and `test_load_model_real_raises_no_diffusion_arch`. A `_make_ctx()` helper constructs minimal `NodeContext` instances. Tests follow the established pattern: direct instantiation of the node, calling `execute()` with keyword args matching `INPUT_SLOTS`, and asserting on return values or raised exceptions.

No gap between design doc and current source affects this task — the design doc (§10.3) specifies the exact slot shapes, and the existing `LoadModel` pattern is already implemented as specified.

## Resolved Dependencies

None. This task introduces no new external crates or Python packages. All dependencies (`BaseNode`, `NodeContext`, `SlotSpec`, `register`, `PipelineCache`) are internal to the project's `worker/` package.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| None | — | — | — | — |

## Approach

### Step 1: Add `LoadVae` class to `worker/nodes/loader.py`

Append the `LoadVae` class immediately after `LoadModel` (after line 82). The class follows the identical structure:

```python
@register
class LoadVae(BaseNode):
    """Load a VAE from a standalone safetensors file.

    This node loads a Variational Autoencoder component. In mock mode
    it returns a sentinel dict; in real mode it raises NotImplementedError
    pending P20 which will implement actual safetensors reading and VAE
    arch dispatch.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Single input slot named "model_id" with type "STRING".
        OUTPUT_SLOTS: Single output slot named "vae" with type "VAE".
    """
    NODE_TYPE = "LoadVae"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load VAE"
    DESCRIPTION = "Loads a VAE from a standalone safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("vae", "VAE")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadVae node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        delegates to the pipeline cache and raises NotImplementedError.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the VAE to load.

        Returns:
            In mock mode: Dict with key "vae" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Raises NotImplementedError (deferred to P20).

        Raises:
            NotImplementedError: When ctx.mock is False — real VAE
                loading logic is deferred to P20.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"vae": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: delegate to the pipeline cache using a VAE-
            # specific cache key namespace ("vae:{model_id}"). The
            # loader_fn itself raises NotImplementedError because no
            # VAE arch module has been registered yet — real loading is
            # deferred to P20. The cache is not modified on exception
            # per the PipelineCache contract.
            return ctx.pipeline_cache.get_or_load(
                f"vae:{inputs['model_id']}",
                lambda: (_ for _ in ()).throw(
                    NotImplementedError(
                        "no diffusion arch module registered yet"
                    )
                ),
            )
```

Rationale for the `f"vae:{inputs['model_id']}"` cache key: each loader node uses a distinct cache-key namespace to prevent VAE/CLIP/model components from colliding in the same cache, per the design doc's principle that each loader has its own "cache-key namespace." The key format mirrors `LoadModel`'s raw `model_id` key but with a `vae:` prefix for namespace separation.

### Step 2: Add `LoadClip` class to `worker/nodes/loader.py`

Append the `LoadClip` class immediately after `LoadVae`. The structure is identical to `LoadVae` with different slot shapes and cache-key namespace:

```python
@register
class LoadClip(BaseNode):
    """Load a CLIP text encoder from a safetensors file.

    This node loads a text encoder component for prompt conditioning.
    In mock mode it returns a sentinel dict; in real mode it raises
    NotImplementedError pending P20 which will implement actual
    safetensors reading and CLIP arch dispatch.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: "model_id" (STRING, required) and "clip_type"
            (STRING, optional). clip_type is a dispatch hint (e.g.
            "qwen3") for architecture-specific loading.
        OUTPUT_SLOTS: Single output slot named "clip" with type "CLIP".
    """
    NODE_TYPE = "LoadClip"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load CLIP"
    DESCRIPTION = "Loads a CLIP text encoder from a safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]
    OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadClip node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        delegates to the pipeline cache and raises NotImplementedError.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the CLIP encoder to load. An optional
                "clip_type" key (e.g. "qwen3") may be provided as a
                dispatch hint for architecture-specific loading.

        Returns:
            In mock mode: Dict with key "clip" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Raises NotImplementedError (deferred to P20).

        Raises:
            NotImplementedError: When ctx.mock is False — real CLIP
                loading logic is deferred to P20.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"clip": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: delegate to the pipeline cache using a CLIP-
            # specific cache key namespace ("clip:{model_id}"). The
            # loader_fn itself raises NotImplementedError because no
            # CLIP arch module has been registered yet — real loading is
            # deferred to P20. The cache is not modified on exception
            # per the PipelineCache contract.
            return ctx.pipeline_cache.get_or_load(
                f"clip:{inputs['model_id']}",
                lambda: (_ for _ in ()).throw(
                    NotImplementedError(
                        "no diffusion arch module registered yet"
                    )
                ),
            )
```

Rationale for the `f"clip:{inputs['model_id']}"` cache key: same namespace principle as `LoadVae` — distinct prefix prevents CLIP components from colliding with VAE or model components in the shared cache. The `clip_type` input is accepted but not used in the cache key for Phase 19; it will be consumed by the real arch dispatch in Phase 20.

### Step 3: Add tests to `worker/tests/test_nodes_loader.py`

Append 10 new test functions to `test_nodes_loader.py`:

1. **`test_load_vae_mock_returns_sentinel`** — mock-mode: constructs `NodeContext(mock=True)`, calls `LoadVae().execute(ctx, model_id="test_vae")`, asserts `{"vae": {"mock": True, "model_id": "test_vae"}}`. Satisfies `MOCK_PATH_VERIFIED`.

2. **`test_load_vae_real_raises_not_implemented`** (marked `@pytest.mark.real_mode`) — real-mode: constructs `PipelineCache()`, `NodeContext(mock=False, pipeline_cache=cache)`, calls `LoadVae().execute(ctx, model_id="test_vae")`, asserts `NotImplementedError` raised. Satisfies `REAL_PATH_VERIFIED`.

3. **`test_load_clip_mock_returns_sentinel`** — mock-mode: constructs `NodeContext(mock=True)`, calls `LoadClip().execute(ctx, model_id="test_clip")`, asserts `{"clip": {"mock": True, "model_id": "test_clip"}}`. Satisfies `MOCK_PATH_VERIFIED`.

4. **`test_load_clip_real_raises_not_implemented`** (marked `@pytest.mark.real_mode`) — real-mode: constructs `PipelineCache()`, `NodeContext(mock=False, pipeline_cache=cache)`, calls `LoadClip().execute(ctx, model_id="test_clip")`, asserts `NotImplementedError` raised. Satisfies `REAL_PATH_VERIFIED`.

5. **`test_load_vae_in_registry`** — subprocess isolation: imports `worker.nodes.loader`, asserts `"LoadVae" in NODE_REGISTRY` and `NODE_REGISTRY["LoadVae"] is mod.LoadVae`.

6. **`test_load_clip_in_registry`** — subprocess isolation: imports `worker.nodes.loader`, asserts `"LoadClip" in NODE_REGISTRY` and `NODE_REGISTRY["LoadClip"] is mod.LoadClip`.

7. **`test_load_vae_real_cache_key_format`** (marked `@pytest.mark.real_mode`) — real-mode with mock cache: verifies `get_or_load` was called with key `"vae:test_model"` and cache remains empty after exception.

8. **`test_load_clip_real_cache_key_format`** (marked `@pytest.mark.real_mode`) — real-mode with mock cache: verifies `get_or_load` was called with key `"clip:test_clip"` and cache remains empty after exception.

9. **`test_load_vae_real_raises_no_diffusion_arch`** (marked `@pytest.mark.real_mode`) — canonical real-mode test with different model_id, asserts exact error message.

10. **`test_load_clip_real_raises_no_diffusion_arch`** (marked `@pytest.mark.real_mode`) — canonical real-mode test with different model_id, asserts exact error message.

Total tests after addition: 5 existing + 10 new = 15 tests (exceeds the >=12 requirement).

### Step 4: Verify NODE_REGISTRY contains both new node types

The `_import_nodes()` function in `worker/nodes/__init__.py` auto-imports all `.py` files under `nodes/` (excluding `__init__` and `base`). Since `loader.py` is already imported (it existed before this task), the `@register` decorators on the new classes will execute when `loader.py` is re-imported. However, since `_import_nodes()` is idempotent (guarded by `_imported` flag), the new classes will only be registered when `loader.py` is imported in a fresh Python process. This is exactly how the existing `test_load_model_in_registry` test works — it spawns a subprocess that imports `worker.nodes.loader` fresh, triggering `@register` for all classes in the module. The registry tests (items 5 and 6 above) verify this works for both new nodes.

## Public API Surface

| Item | Path | Signature/Description |
|------|------|----------------------|
| Class | `worker.nodes.loader.LoadVae` | `NODE_TYPE="LoadVae"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]`, `OUTPUT_SLOTS=[SlotSpec("vae", "VAE")]`, `execute(self, ctx: NodeContext, **inputs) -> dict` |
| Class | `worker.nodes.loader.LoadClip` | `NODE_TYPE="LoadClip"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]`, `OUTPUT_SLOTS=[SlotSpec("clip", "CLIP")]`, `execute(self, ctx: NodeContext, **inputs) -> dict` |

Both classes are registered in `NODE_REGISTRY` via the `@register` decorator (imported from `worker.nodes.base`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `LoadVae` and `LoadClip` classes after existing `LoadModel` |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add 10 new test functions for both nodes |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `test_nodes_loader.py` | `test_load_vae_mock_returns_sentinel` | LoadVae mock-mode returns sentinel dict `{"vae": {"mock": True, "model_id": "test_vae"}}` | `python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel -v` |
| `test_nodes_loader.py` | `test_load_vae_real_raises_not_implemented` (real) | LoadVae real-mode raises `NotImplementedError` with Phase-19 message; satisfies `REAL_PATH_VERIFIED` marker | `python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented -v -m real_mode` |
| `test_nodes_loader.py` | `test_load_clip_mock_returns_sentinel` | LoadClip mock-mode returns sentinel dict `{"clip": {"mock": True, "model_id": "test_clip"}}` | `python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel -v` |
| `test_nodes_loader.py` | `test_load_clip_real_raises_not_implemented` (real) | LoadClip real-mode raises `NotImplementedError` with Phase-19 message; satisfies `REAL_PATH_VERIFIED` marker | `python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented -v -m real_mode` |
| `test_nodes_loader.py` | `test_load_vae_in_registry` | LoadVae appears in NODE_REGISTRY after importing `worker.nodes.loader` | `python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_in_registry -v` |
| `test_nodes_loader.py` | `test_load_clip_in_registry` | LoadClip appears in NODE_REGISTRY after importing `worker.nodes.loader` | `python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_in_registry -v` |
| `test_nodes_loader.py` | `test_load_vae_real_cache_key_format` (real) | LoadVae real branch calls `get_or_load` with `"vae:test_model"` key; cache empty after exception | `python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format -v -m real_mode` |
| `test_nodes_loader.py` | `test_load_clip_real_cache_key_format` (real) | LoadClip real branch calls `get_or_load` with `"clip:test_clip"` key; cache empty after exception | `python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format -v -m real_mode` |
| `test_nodes_loader.py` | `test_load_vae_real_raises_no_diffusion_arch` (real) | LoadVae canonical real-mode test with different model_id, exact error message | `python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch -v -m real_mode` |
| `test_nodes_loader.py` | `test_load_clip_real_raises_no_diffusion_arch` (real) | LoadClip canonical real-mode test with different model_id, exact error message | `python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch -v -m real_mode` |

Acceptance command for the full file:
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> 15 tests, exits 0
```

## CI Impact

No CI changes required. The existing CI wiring from Phase 9 (P9-F1) already runs the full `worker/tests/` suite for both mock-mode (`-m "not real_mode"`) and real-mode (`-m real_mode`). The new tests in `test_nodes_loader.py` will be automatically collected by both CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) without any workflow file modifications.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. Both nodes use only Python built-ins (`dict`, `f-strings`, `lambda`) and project-internal imports. No `os.path` vs `pathlib` differences, no path-separator handling, no platform-specific behavior.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `loader.py` is already imported when `_import_nodes()` runs (idempotent guard), so new `@register` decorators won't fire in the same process. Tests that check `NODE_REGISTRY` directly (without subprocess isolation) may not see the new nodes. | Medium | High | All registry tests use subprocess isolation (matching `test_load_model_in_registry` pattern). Direct `NODE_REGISTRY` checks are only needed in subprocess-scoped tests, which is already the established convention. |
| The `f"vae:{inputs['model_id']}"` and `f"clip:{inputs['model_id']}"` cache key formats may conflict with future Phase 20 arch module expectations if they also use prefixed keys. | Low | Medium | The Phase 19 task spec explicitly states "different SlotType/cache-key namespace per loader" — the prefixed format is the correct design. Phase 20 arch modules will read this as an established convention. |
| `clip_type` optional slot in `LoadClip` may cause test failures if tests don't pass `clip_type` and the node code tries to access it. | Low | Medium | The mock and real branches only read `inputs["model_id"]`, never `inputs["clip_type"]`. The optional slot is declared for the node schema but not consumed in Phase 19's placeholder implementation. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with >=12 tests (expected 15)
- [ ] `grep -n "LoadVae\|LoadClip" worker/nodes/loader.py` returns lines for both new classes with correct `NODE_TYPE`, `INPUT_SLOTS`, and `OUTPUT_SLOTS`
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/loader.py` returns 4 lines (one per node's `execute()`)
- [ ] `grep "MOCK_PATH_VERIFIED:" worker/nodes/loader.py` returns 4 lines (one per node's `execute()`)
- [ ] `python -c "import subprocess, sys; r=subprocess.run([sys.executable,'-c','from worker.nodes.base import NODE_REGISTRY; assert \"LoadVae\" in NODE_REGISTRY and \"LoadClip\" in NODE_REGISTRY; print(\"OK\")'], capture_output=True, text=True, timeout=10); assert r.returncode==0 and \"OK\" in r.stdout"` exits 0
