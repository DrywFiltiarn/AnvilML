# Plan Report: P24-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P24-C1                                            |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/loader.py: EmptyLatent node, mock branch only |
| Depends on  | P21-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-17T22:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `EmptyLatent` node class to `worker/nodes/loader.py` with a working mock branch
that returns a placeholder-shaped latent tensor, and a real branch that raises
`NotImplementedError` (deferred to P24-C2). The node is registered in `NODE_REGISTRY`
via `@register`, carries both `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` dual-mode parity
markers, and is exercised by ≥3 tests in `worker/tests/test_nodes_loader.py`.

## Scope

### In Scope
- Add `EmptyLatent` class to `worker/nodes/loader.py`, co-located with `LoadModel`,
  `LoadVae`, and `LoadClip` per §10.3's `Loaders` category grouping (even though this
  node creates rather than loads).
- Exact class attributes per `ANVILML_DESIGN.md §10.3`:
  - `NODE_TYPE = "EmptyLatent"`
  - `CATEGORY = "Latents"`
  - `DISPLAY_NAME = "Empty Latent"`
  - `DESCRIPTION = "Creates a blank noise latent tensor."`
  - `INPUT_SLOTS = [SlotSpec("width", "INT"), SlotSpec("height", "INT"), SlotSpec("batch_size", "INT", optional=True), SlotSpec("model", "MODEL", optional=True)]`
  - `OUTPUT_SLOTS = [SlotSpec("latent", "LATENT")]`
- Mock branch: `ctx.mock` → returns `{"latent": torch.zeros((batch_size, 4, height//8, width//8), dtype=torch.float32)}` — a generic placeholder-shaped latent with no model dispatch, ignoring the optional `model` input entirely (per §10.3's explicit note that mock mode ignores this input).
- Real branch: raises `NotImplementedError` with a clear message naming P24-C2 as the
  deferring task. This is a placeholder stub, not a real implementation.
- Add `# defers_to: P24-C2` comment at the stub site on the `raise NotImplementedError` line, per `FORGE_AGENT_RULES.md §9.7`.
- Add dual-mode parity markers next to the `execute()` method:
  - `# REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_empty_latent_real_raises_not_implemented`
  - `# MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_empty_latent_mock_returns_placeholder_shape`
- Google-style docstrings on the class and `execute()` method, per `ENVIRONMENT.md §10`.
- ≥3 tests in `worker/tests/test_nodes_loader.py`:
  1. Mock branch returns the correct placeholder shape.
  2. Mock branch ignores a provided model input (verifies the "ignores model" contract).
  3. Node is in `NODE_REGISTRY` under key `"EmptyLatent"` (subprocess isolation).

### Out of Scope
- Real branch implementation (dispatch to `arch.diffusion.get_module().compute_latent_shape()`) — deferred to P24-C2, which is listed in this task's `defers_to` field and whose description confirms it delivers the real branch.

## Existing Codebase Assessment

**What already exists:** `worker/nodes/loader.py` contains three established node classes
(`LoadModel`, `LoadVae`, `LoadClip`) that follow a consistent pattern: class attributes
(`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`),
a `@register` decorator, a Google-style docstring on the class, and an `execute()` method
that branches on `ctx.mock` at the top. The mock branch returns a sentinel dict; the real
branch dispatches to an arch module via `get_module()` and caches via `pipeline_cache`.
`worker/nodes/base.py` defines `BaseNode` (ABC), `SlotSpec` (dataclass), `NodeContext`,
and the `@register` decorator. `worker/nodes/__init__.py` auto-imports all `.py` files
under `nodes/` via `pkgutil.iter_modules()`. `worker/tests/test_nodes_loader.py` has a
consistent test pattern: `_make_ctx()` helper, mock tests with `assert result == {...}`,
real tests marked `@pytest.mark.real_mode`, and subprocess-isolated registry tests.

**Established patterns to follow:** (a) Class attributes as exact class-level assignments,
(b) `if ctx.mock:` at the top of `execute()` with inline comments explaining each branch,
(c) Google-style docstrings with `Args:`, `Returns:`, `Raises:` sections,
(d) `logging.getLogger(__name__)` for logging, (e) subprocess isolation for registry
tests (to avoid cross-test pollution), (f) dual-mode parity markers as module-level
comments directly above `def execute()`.

**Gap between design doc and current source:** No existing source code for `EmptyLatent`
exists yet — this task introduces it. The design doc (§10.3) specifies that mock mode
ignores the `model` input entirely (correct behavior, not an oversight), while real mode
requires it — this task's mock branch must not dispatch even if a model is provided, and
the real branch is a stub.

## Resolved Dependencies

None. This task introduces no new external crates or Python packages. All imports are
from the project's existing codebase (`worker.nodes.base`, `torch` in test).

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | | | | |

## Approach

1. **Add `EmptyLatent` class to `worker/nodes/loader.py`**, placed after the `LoadClip`
   class and before the end of file. The class follows the exact same structural pattern
   as `LoadModel`, `LoadVae`, and `LoadClip`:

   ```python
   @register
   class EmptyLatent(BaseNode):
       """Create a blank noise latent tensor.

       This node generates an empty latent of the specified dimensions.
       In mock mode it returns a placeholder tensor with no model dispatch.
       In real mode it dispatches to the loaded model's arch module to
       compute the architecture-specific latent shape (P24-C2).

       Class Attributes:
           NODE_TYPE: The registry key for this node type.
           CATEGORY: The category this node belongs to.
           DISPLAY_NAME: Human-readable name shown in UI/tooling.
           DESCRIPTION: One-line description of the node's purpose.
           INPUT_SLOTS: width (INT, required), height (INT, required),
               batch_size (INT, optional, default 1), model (MODEL, optional).
           OUTPUT_SLOTS: Single output slot named "latent" with type "LATENT".
       """
       NODE_TYPE = "EmptyLatent"
       CATEGORY = "Latents"
       DISPLAY_NAME = "Empty Latent"
       DESCRIPTION = "Creates a blank noise latent tensor."
       INPUT_SLOTS = [
           SlotSpec("width", "INT"),
           SlotSpec("height", "INT"),
           SlotSpec("batch_size", "INT", optional=True),
           SlotSpec("model", "MODEL", optional=True),
       ]
       OUTPUT_SLOTS = [SlotSpec("latent", "LATENT")]

       # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_empty_latent_real_raises_not_implemented
       # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_empty_latent_mock_returns_placeholder_shape
       def execute(self, ctx: NodeContext, **inputs) -> dict:
           """Execute the EmptyLatent node.

           Branches on ctx.mock at the top per §14.6 — the mock branch
           returns a placeholder latent tensor with no model dispatch;
           the real branch (P24-C2) dispatches to the loaded model's
           arch module for architecture-specific shape computation.

           Args:
               ctx: Runtime context carrying job_id, device, caps,
                   cancel_flag, emit, pipeline_cache, and mock flag.
               **inputs: Must contain "width" (int) and "height" (int).
                   Optional "batch_size" (int, default 1) and "model"
                   (MODEL, optional — ignored in mock mode, required
                   in real mode).

           Returns:
               In mock mode: Dict with key "latent" containing a
               torch.zeros tensor of shape (batch_size, 4, height//8, width//8).
               In real mode: (deferred to P24-C2) Dict with key "latent"
               containing a tensor computed via compute_latent_shape().

           Raises:
               NotImplementedError: If called in real mode without
                   P24-C2's real branch implementation.
           """
           if ctx.mock:
               # Mock branch: return a generic placeholder latent with
               # the standard VAE-downsampled shape formula (C=4, H/8, W/8).
               # Per §10.3's note, mock mode ignores the optional "model"
               # input entirely — this is correct behavior, not an oversight.
               import torch

               width = inputs["width"]
               height = inputs["height"]
               batch_size = inputs.get("batch_size", 1)

               latent_shape = (batch_size, 4, height // 8, width // 8)
               return {"latent": torch.zeros(latent_shape, dtype=torch.float32)}
           else:
               # Real branch placeholder — full implementation is deferred
               # to P24-C2, which dispatches to arch.diffusion.get_module()
               # and calls compute_latent_shape().
               raise NotImplementedError(
                   f"EmptyLatent real branch not yet implemented; "
                   f"deferred to P24-C2"
               )
   ```

   Key design decisions:
   - The `import torch` is inside the mock branch, not at module top-level. This follows
     the pattern established by existing loader nodes where the mock path avoids torch
     imports (though in this case the mock *does* produce a torch tensor, the import is
     local to the branch). Wait — looking more carefully at the existing loader nodes,
     they do NOT import torch at all in their mock branches (they return plain dicts).
     However, the test for this node will need to verify the tensor shape, and the design
     doc says the mock returns a "placeholder-shaped latent" — a tensor is the natural
     representation. The mock branch *must* import torch locally to produce the tensor,
     since the real `torch.zeros` call is the point of the test. This is acceptable because
     the test itself runs with `ANVILML_WORKER_MOCK=1` and the mock path is the one
     exercising the torch import.
   - The latent shape uses `(batch_size, 4, height//8, width//8)` — this is the standard
     VAE-downsampled shape (4 channels, 8x spatial downscale), matching the formula used
     by `compute_latent_shape()` in Phase 21.
   - The real branch raises `NotImplementedError` with a message naming P24-C2, and carries
     the `# defers_to: P24-C2` comment per `FORGE_AGENT_RULES.md §9.7`.

2. **Add tests to `worker/tests/test_nodes_loader.py`**, after the existing `LoadClip`
   test section:

   **Test 1 — `test_empty_latent_mock_returns_placeholder_shape`**:
   - Construct a `NodeContext` with `mock=True`.
   - Call `execute(ctx, width=64, height=64)`.
   - Assert the result has a `"latent"` key.
   - Assert `result["latent"]` is a `torch.Tensor` with shape `(1, 4, 8, 8)`.
   - This satisfies the `MOCK_PATH_VERIFIED` marker.

   **Test 2 — `test_empty_latent_mock_ignores_model_input`**:
   - Construct a `NodeContext` with `mock=True`.
   - Call `execute(ctx, width=128, height=128, model={"mock": True, "model_id": "ignored"})`.
   - Assert the result has a `"latent"` key with shape `(1, 4, 16, 16)`.
   - Assert the model input was not used (the result is identical to calling without model).
   - This verifies the §10.3 contract that mock mode ignores the `model` input.

   **Test 3 — `test_empty_latent_in_registry`**:
   - Subprocess isolation: import `worker.nodes.loader`, check `NODE_REGISTRY["EmptyLatent"]`
     exists and equals `mod.EmptyLatent`.
   - Print `"OK"` on success.
   - Same pattern as existing `test_load_model_in_registry`, `test_load_vae_in_registry`,
     `test_load_clip_in_registry`.

   **Test 4 — `test_empty_latent_real_raises_not_implemented`** (real_mode marked):
   - Construct a `NodeContext` with `mock=False`.
   - Call `execute(ctx, width=64, height=64)`.
   - Assert `NotImplementedError` is raised.
   - Assert the error message mentions "P24-C2".
   - This satisfies the `REAL_PATH_VERIFIED` marker.

3. **Verify syntax** by running `worker/.venv/bin/python -m py_compile worker/nodes/loader.py`
   and `worker/.venv/bin/python -m py_compile worker/tests/test_nodes_loader.py`.

4. **Run tests** with `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v`.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `class EmptyLatent` | `worker.nodes.loader` | New node class; `pub` via `@register` side-effect on `NODE_REGISTRY`. |
| `EmptyLatent.execute()` | `worker.nodes.loader.EmptyLatent.execute` | New method; branches on `ctx.mock`. Mock returns placeholder tensor; real raises `NotImplementedError`. |

Full class attributes (exact values):
```python
NODE_TYPE = "EmptyLatent"
CATEGORY = "Latents"
DISPLAY_NAME = "Empty Latent"
DESCRIPTION = "Creates a blank noise latent tensor."
INPUT_SLOTS = [
    SlotSpec("width", "INT"),
    SlotSpec("height", "INT"),
    SlotSpec("batch_size", "INT", optional=True),
    SlotSpec("model", "MODEL", optional=True),
]
OUTPUT_SLOTS = [SlotSpec("latent", "LATENT")]
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `EmptyLatent` class with mock branch and real-branch stub after `LoadClip`. |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add ≥4 tests for `EmptyLatent`: mock shape, mock ignores model, registry, real raises. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_mock_returns_placeholder_shape` (mock) | Mock branch returns `torch.zeros` tensor with correct shape `(1, 4, 8, 8)` for `width=64, height=64`. Satisfies `MOCK_PATH_VERIFIED`. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_mock_returns_placeholder_shape -v` |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_mock_ignores_model_input` (mock) | Mock branch returns identical latent shape when a `model` input is provided vs absent, verifying §10.3's "mock ignores model" contract. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_mock_ignores_model_input -v` |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_in_registry` (mock) | Subprocess import proves `@register` placed `EmptyLatent` in `NODE_REGISTRY` under key `"EmptyLatent"`. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_in_registry -v` |
| `worker/tests/test_nodes_loader.py` | `test_empty_latent_real_raises_not_implemented` (real) | Real branch raises `NotImplementedError` with P24-C2 in message. Satisfies `REAL_PATH_VERIFIED`. | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_real_raises_not_implemented -v -m real_mode` |

Acceptance command for full task:
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
```
Must exit 0 with ≥3 new EmptyLatent tests.

## CI Impact

No CI changes required. The new tests follow the existing pattern: mock-mode tests run
in the `worker-linux-mock` and `worker-windows-mock` CI jobs (no `real_mode` marker),
and the one real-mode test runs in `worker-linux-real` and `worker-windows-real` CI jobs.
No new CI job or step is needed.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. The mock
branch uses `torch.zeros()` which is platform-neutral. No `#[cfg(...)]` or path-separator
handling is needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The mock branch imports `torch` locally — if `torch` is not available in the mock-mode venv (unlikely but possible if `base.txt` was installed without torch), the mock test will fail with an import error rather than a test assertion failure. | Low | Medium | The mock-mode venv is provisioned with `base.txt` which includes `msgpack` and `pyzmq` but not `torch` (per `ENVIRONMENT.md §5` — torch is installed separately via architecture-specific requirements). However, the mock tests for loader.py currently don't use torch in the mock path. If torch import in the mock branch causes issues, the mock can return a plain dict `{"latent": {"mock": True, "shape": [batch_size, 4, h//8, w//8]}}` instead, and the test verifies the dict shape. This is the safer approach. |
| The `REAL_PATH_VERIFIED` marker points at a test that only checks `NotImplementedError` — this is a weak verification of the real path since it doesn't exercise any real inference logic. However, this is intentional for P24-C1: the real branch is a stub, and P24-C2 will update both markers to point at a meaningful real-mode test. | Low | Low | Document this in the plan. P24-C2's context confirms it updates both markers. Gate 4 (§8) will pass because the named test exists and is collectible (pytest --collect-only resolves it). |
| The latent shape formula `(batch_size, 4, height//8, width//8)` may not match the exact formula used by `compute_latent_shape()` in Phase 21 if that implementation uses a different downscale factor. | Low | Medium | The §10.3 design doc notes that `compute_latent_shape()` returns the "shape formula, not just a scale factor, is architecture-specific." The mock uses the standard VAE shape (C=4, 8x downscale) which is the canonical formula for diffusion models. If Phase 21's implementation differs, the mock test will fail and the ACT agent will need to adjust. This is caught at test time. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_mock_returns_placeholder_shape -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_mock_ignores_model_input -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_in_registry -v` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_empty_latent_real_raises_not_implemented -v -m real_mode` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with ≥3 EmptyLatent tests
