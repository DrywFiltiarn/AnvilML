# Plan Report: P904-A8

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A8                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/loader.py: LoadModel and LoadVae real paths never move loaded components to ctx.device |
| Depends on  | P18-D13, P18-D14, P904-A7                   |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T09:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the device-placement defect in `LoadModel` and `LoadVae` real-mode loading paths so that
loaded transformer and VAE components are moved to `self.ctx.device` instead of silently
defaulting to CPU. This closes the same defect class identified in P904-A7 (CLIP loaders)
across the remaining two real-mode loader node types, ensuring every real generation runs on
the correct GPU device.

## Scope

### In Scope
- `worker/nodes/loader.py`: Add `device: str` parameter to `_load_model_from_hf_directory()`,
  call `transformer = transformer.to(device)` before constructing `RealModel`, and update
  `LoadModel.execute()` to pass `self.ctx.device` as the device argument.
- `worker/nodes/loader.py`: In `LoadVae.execute()`, have the `loader_fn` closure capture
  `self.ctx.device` and call `vae = vae.to(device)` on the `AutoencoderKL` result, assigning
  the return value.
- `worker/tests/test_nodes_loader.py`: Add a test verifying that `_load_model_from_hf_directory()`
  accepts a `device` parameter (mock-mode test — verifies the new function signature without
  requiring real model files).

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope with no deferrals.

## Existing Codebase Assessment

The `worker/nodes/loader.py` file defines three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`)
that load diffusion components from safetensors files. Each node follows a consistent pattern:
check `ANVILML_WORKER_MOCK` environment variable early for mock-mode sentinel return, then
in real mode, lazily import `torch`/`diffusers`/`safetensors` and load the component.

`LoadModel.execute()` (line 387) calls `self.ctx.pipeline_cache.get_or_load()` with a lambda
that invokes `_load_model_from_hf_directory(model_id, model_id)` — currently passing only
two arguments, with no device reference. The helper function `_load_model_from_hf_directory()`
(line 660) loads a `ZImageTransformer2DModel` via `from_single_file()` and wraps it in
`RealModel(transformer, arch=detected_arch)` without any `.to(device)` call.

`LoadVae.execute()` (line 462) similarly uses `get_or_load()` with a `loader_fn` closure that
calls `AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)` — again, no device
reference, no `.to()` call.

`LoadClip.execute()` (line 561) is the pattern to follow: it already passes
`device=self.ctx.device` to the matched module's `load()` function (line 626), and P904-A7
ensured that all three CLIP arch modules call `model.to(device)` before returning. The
established convention in this file is: lazy imports inside the real-mode branch, then
`self.ctx.device` as the device source.

`NodeContext.device` (from `worker/nodes/base.py`, line 153) is a `str` type (e.g. `"cuda:0"`,
`"cpu"`), set by the worker at startup based on the detected GPU. It is the single source of
truth for target device placement across all loaders.

The existing test suite in `worker/tests/test_nodes_loader.py` uses `mock_context` fixture
with `device="cpu"` and only exercises mock-mode paths (ANVILML_WORKER_MOCK=1). No test
currently reaches the real-mode loading code paths, which is expected since CI has no `torch`.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| python | diffusers    | 0.38.0          | pypi-query MCP | n/a |
| python | torch        | 2.x (project uses CPU/ROCm/CUDA builds) | pypi-query MCP | n/a |
| python | safetensors  | 0.x             | pypi-query MCP | n/a |

No new dependencies are introduced. The task only modifies existing import patterns.

## Approach

### Step 1: Modify `_load_model_from_hf_directory()` to accept and use a `device` parameter

In `worker/nodes/loader.py`, change the function signature of `_load_model_from_hf_directory`
from:

```python
def _load_model_from_hf_directory(model_id: str, arch: str) -> RealModel:
```

to:

```python
def _load_model_from_hf_directory(model_id: str, arch: str, device: str = "cpu") -> RealModel:
```

After the `from_single_file()` call (line 720-723), add a device placement line that assigns
the return value (because `.to()` may return a new object reference in some diffusers versions):

```python
transformer = transformer.to(device)
```

Then pass `detected_arch` to the `RealModel` constructor as before. The device string defaults
to `"cpu"` to maintain backward compatibility with any existing callers that don't pass a device.

**Rationale:** The default of `"cpu"` matches `RealClip.__init__`'s own default (used in P904-A7),
preserving the existing call pattern for any code that calls this function without a device arg.
In production, `LoadModel.execute()` will always pass `self.ctx.device` explicitly.

### Step 2: Update `LoadModel.execute()` to pass `self.ctx.device`

In `LoadModel.execute()` (line 431-433), change the lambda call from:

```python
lambda: _load_model_from_hf_directory(model_id, model_id)
```

to:

```python
lambda: _load_model_from_hf_directory(model_id, model_id, self.ctx.device)
```

This ensures the transformer is placed on the worker's assigned device.

### Step 3: Modify `LoadVae.execute()`'s `loader_fn` closure to capture and use `self.ctx.device`

In `LoadVae.execute()` (line 513-517), change the `loader_fn` closure from:

```python
def loader_fn() -> AutoencoderKL:
    return AutoencoderKL.from_single_file(
        model_id,
        torch_dtype=torch.bfloat16,
    )
```

to:

```python
def loader_fn() -> AutoencoderKL:
    vae = AutoencoderKL.from_single_file(
        model_id,
        torch_dtype=torch.bfloat16,
    )
    return vae.to(device)  # device captured from self.ctx.device below
```

And add a captured `device` variable before the closure definition:

```python
device = self.ctx.device  # capture device for use in loader_fn closure
```

**Rationale:** The closure captures `device` from the enclosing scope. By assigning the return
value of `.to(device)` (rather than assuming in-place mutation), we handle both diffusers versions
where `.to()` returns a new reference and versions where it mutates in place.

### Step 4: Add a unit test for the new `_load_model_from_hf_directory` signature

Add a test in `worker/tests/test_nodes_loader.py` that verifies the function accepts the new
`device` parameter. Since real loading requires torch/diffusers/safetensors (not available in
CI mock mode), this test will use `pytest.importorskip` to skip when those packages are absent,
and verify the function signature accepts a third positional argument.

## Public API Surface

No new public API items are introduced. The changes are internal:

| Item | Module Path | Change |
|------|------------|--------|
| `_load_model_from_hf_directory` | `worker.nodes.loader` | Signature widened: added `device: str = "cpu"` parameter |
| `LoadModel.execute` | `worker.nodes.loader` | Lambda now passes `self.ctx.device` to `_load_model_from_hf_directory` |
| `LoadVae.execute` | `worker.nodes.loader` | `loader_fn` closure captures `self.ctx.device` and calls `.to(device)` on result |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Add `device` param to `_load_model_from_hf_directory`, call `.to(device)` on transformer and VAE results, update call sites |
| Modify | `worker/tests/test_nodes_loader.py` | Add test verifying new device parameter acceptance |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_hf_directory_accepts_device_param` | `_load_model_from_hf_directory` function signature accepts a third `device` positional argument | `torch`, `diffusers`, `safetensors` installed (real mode) | None (signature check only) | Function does not raise TypeError when called with 3 args | `python3 -c "import inspect, os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.loader import _load_model_from_hf_directory; sig = inspect.signature(_load_model_from_hf_directory); assert 'device' in sig.parameters"` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_execute_passes_device_to_loader` | `LoadModel.execute()` passes `self.ctx.device` to `_load_model_from_hf_directory` by verifying the lambda captures it (mock-mode: verify mock path still works) | `ANVILML_WORKER_MOCK=1` | `model_id="test"` | Returns `MockModel(arch="zit")` unchanged (mock mode does not reach the real loader) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_passes_device_to_loader -v` exits 0 |

## CI Impact

No CI changes required. The existing test suite runs with `ANVILML_WORKER_MOCK=1` (from
`conftest.py` autouse fixture), which exercises the mock code paths only. The new test for
the `device` parameter uses `importorskip` to gracefully skip when torch/diffusers are absent,
matching the project's established pattern for real-mode-only tests.

## Platform Considerations

None identified. The `.to(device)` call is platform-neutral — PyTorch handles device placement
across CUDA, ROCm, and CPU transparently. The `device` string (`"cuda:0"`, `"cpu"`, etc.) is
set by the worker at startup based on hardware detection and passed through unchanged. No
`#[cfg(...)]` guards are needed (this is Python code, not Rust).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `AutoencoderKL.to(device)` or `ZImageTransformer2DModel.to(device)` returns a new object reference rather than mutating in place, and the returned value is not assigned — the old CPU-placed object would be silently returned instead of the GPU-placed one. | Medium | High | The approach explicitly assigns the return value: `transformer = transformer.to(device)` and `return vae.to(device)`. This is the correct pattern per diffusers documentation and the task context's explicit instruction. |
| The new `device` parameter on `_load_model_from_hf_directory` breaks an existing caller that passes only two positional arguments. | Low | High | The parameter has a default of `"cpu"`, matching the existing behavior. No existing caller in the codebase passes more than 2 args to this function (confirmed by grep). P904-A9 (next task) renames this function, so any future callers will use the new name. |
| Mock-mode tests may inadvertently exercise the real-mode path if `ANVILML_WORKER_MOCK` is not properly set in a new test. | Low | Medium | The existing `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for all tests in `worker/tests/`. New tests follow the same pattern and explicitly verify mock-mode behavior. |

## Acceptance Criteria

- [ ] `grep -n "def _load_model_from_hf_directory(model_id: str, arch: str, device: str = \"cpu\")" worker/nodes/loader.py` — one match confirming the new signature
- [ ] `grep -n "transformer = transformer.to(device)" worker/nodes/loader.py` — one match confirming device placement for LoadModel
- [ ] `grep -n "self.ctx.device" worker/nodes/loader.py` — at least two matches (LoadModel and LoadVae both reference ctx.device)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` — exits 0, same test count as before
- [ ] `python3 -c "import inspect, os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.loader import _load_model_from_hf_directory; sig = inspect.signature(_load_model_from_hf_directory); assert 'device' in sig.parameters"` — exits 0
