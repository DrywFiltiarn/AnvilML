# Plan Report: P904-A7

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-A7                                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/nodes/arch/clip/{qwen3,clip_l,t5}.py: text encoder models never moved to ctx.device, always run on CPU |
| Depends on  | P18-D9, P18-D10, P18-D11, P904-A6b                         |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-24T08:05:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Fix the silent device-placement defect in all three CLIP text-encoder architecture modules (`qwen3.py`, `clip_l.py`, `t5.py`) and in `LoadClip.execute()` so that loaded text encoders are placed on the worker's assigned device (`ctx.device`, typically `cuda:0`) instead of silently running on CPU. The fix widens each `load()` function's signature to accept a `device` parameter (defaulting to `"cpu"` for backward compatibility with existing mock-mode tests), calls `model.to(device)` before constructing `RealClip`, and has `LoadClip.execute()` pass `device=self.ctx.device` explicitly in real mode.

## Scope

### In Scope
- `worker/nodes/arch/clip/qwen3.py`: widen `load(model_id, torch_dtype)` to `load(model_id, torch_dtype, device: str = "cpu")`; call `model.to(device)` after `load_state_dict`; pass `device=device` to `RealClip()` constructor in both mock and real return paths.
- `worker/nodes/arch/clip/clip_l.py`: identical change.
- `worker/nodes/arch/clip/t5.py`: identical change.
- `worker/nodes/loader.py`: `LoadClip.execute()` passes `device=self.ctx.device` as a keyword argument to `module.load(...)` in real mode.
- All three existing test files (`test_arch_clip_qwen3.py`, `test_arch_clip_l.py`, `test_arch_clip_t5.py`) pass without modification — the default `device="cpu"` preserves the positional `load("/fake/path", None)` call pattern.

### Out of Scope
None. `defers_to (from JSON): absent`. This task implements its full scope without deferring any functionality to another task.

## Existing Codebase Assessment

Three things were found during codebase inspection:

**(a) What already exists:** `RealClip.__init__` in `loader.py` already accepts a `device: str = "cpu"` parameter and stores it as `self._device`. The parameter is documented and used by `RealClip.encode()` to move input tensors to the correct device. However, no caller ever passes a non-default device — all three `load()` functions construct `RealClip(tokenizer, model)` without the `device` argument, and none of them call `.to(device)` on the model after loading. `LoadClip.execute()` calls `module.load(model_id, torch_dtype=torch.bfloat16)` without passing `device`.

**(b) Established patterns:** All three arch modules follow an identical structure: mock-mode early return → lazy imports → construct model from config → `load_state_dict()` → return `RealClip(tokenizer, model)`. The mock path constructs `RealClip(MockTokenizer(), MockTextEncoder())` without a device argument (relying on the default). Existing tests call `load("/fake/path", None)` positionally. No module calls `.to()` on any model. The design doc at §10.4a already declares the target signature `load(model_id: str, torch_dtype: Any, device: str) -> RealClip`.

**(c) Gap between design doc and source:** The design doc specifies `device: str` as a required parameter (no default shown), while the task context and `RealClip.__init__` both use `"cpu"` as the default. The source files have zero references to `.to(` or `device=` in any `load()` function. This gap is the defect being fixed.

## Resolved Dependencies

No new external dependencies are introduced. The task uses only `torch.Tensor.to()` (standard PyTorch API, already a transitive dependency via the existing `torch` import in these modules' real-mode paths) and the existing `device` parameter of `RealClip.__init__`.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

1. **Modify `worker/nodes/arch/clip/qwen3.py`**:
   - Change the `load()` signature from `def load(model_id: str, torch_dtype: Any) -> RealClip` to `def load(model_id: str, torch_dtype: Any, device: str = "cpu") -> RealClip`.
   - Update the docstring to document the new `device` argument.
   - In the mock-mode path (line ~86), change `return RealClip(MockTokenizer(), MockTextEncoder())` to `return RealClip(MockTokenizer(), MockTextEncoder(), device=device)`.
   - In the real-mode path, after `model.load_state_dict(safetensors_load_file(model_id))` (line ~133), insert `model = model.to(device)` before the return. This assigns the return value because `.to()` returns a new reference for some module types.
   - Change the final return from `return RealClip(tokenizer, model)` to `return RealClip(tokenizer, model, device=device)`.

2. **Modify `worker/nodes/arch/clip/clip_l.py`**:
   - Apply the identical four changes as step 1: widen signature with `device: str = "cpu"` default, update docstring, pass `device=device` to `RealClip()` in both mock and real return paths, insert `model = model.to(device)` after `load_state_dict()`.

3. **Modify `worker/nodes/arch/clip/t5.py`**:
   - Apply the identical four changes as step 1.

4. **Modify `worker/nodes/loader.py`** (only `LoadClip.execute()`):
   - In the real-mode branch of `LoadClip.execute()` (line ~623), change:
     ```python
     return module.load(model_id, torch_dtype=torch.bfloat16)
     ```
     to:
     ```python
     return module.load(model_id, torch_dtype=torch.bfloat16, device=self.ctx.device)
     ```
   - This passes the worker's assigned device explicitly so that the text encoder is placed on the correct GPU/CPU.

5. **Verify**: Run the existing mock-mode tests for all three CLIP arch modules and `test_nodes_loader.py` — they must pass without modification because:
   - Mock-mode tests call `load("/fake/path", None)` positionally (2 args), which is compatible with the new 3rd parameter having a default.
   - Mock-mode returns `RealClip(..., device=device)` where `device="cpu"` (the default), which is identical to the current behavior of `RealClip(...)` relying on its own default.
   - The `test_load_mock_no_torch_import` tests remain unaffected because no top-level imports change.

No new tests are added because the existing mock-mode test suite already exercises the `load()` function with the positional call pattern that must continue to work. The device-placement fix is a real-mode-only change that is verified by the real-mode test suite (P904-B3) in a later task.

## Public API Surface

Every `load()` function in the clip arch modules is a module-level public function (listed in `__all__`). The signatures change as follows:

| Module | Before | After |
|--------|--------|-------|
| `worker.nodes.arch.clip.qwen3.load` | `def load(model_id: str, torch_dtype: Any) -> RealClip` | `def load(model_id: str, torch_dtype: Any, device: str = "cpu") -> RealClip` |
| `worker.nodes.arch.clip.clip_l.load` | `def load(model_id: str, torch_dtype: Any) -> RealClip` | `def load(model_id: str, torch_dtype: Any, device: str = "cpu") -> RealClip` |
| `worker.nodes.arch.clip.t5.load` | `def load(model_id: str, torch_dtype: Any) -> RealClip` | `def load(model_id: str, torch_dtype: Any, device: str = "cpu") -> RealClip` |

The `RealClip.__init__` signature is unchanged — it already accepts `device: str = "cpu"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Widen `load()` signature with `device` param; call `model.to(device)`; pass device to `RealClip()` |
| MODIFY | `worker/nodes/arch/clip/clip_l.py` | Identical changes to qwen3.py |
| MODIFY | `worker/nodes/arch/clip/t5.py` | Identical changes to qwen3.py |
| MODIFY | `worker/nodes/loader.py` | `LoadClip.execute()` passes `device=self.ctx.device` to `module.load()` in real mode |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_returns_realclip` | `load()` returns `RealClip` with sentinels in mock mode, compatible with new 3-arg signature | `ANVILML_WORKER_MOCK=1` (from conftest.py autouse fixture) | `load("/fake/path", None)` (2 positional args) | `isinstance(result, RealClip)`, tokenizer is `MockTokenizer`, text_encoder is `MockTextEncoder` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_returns_realclip` | Same as above for CLIP-L | `ANVILML_WORKER_MOCK=1` | `load("/fake/path", None)` | Same assertions | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py -v` exits 0 |
| `worker/tests/test_arch_clip_t5.py` | `test_load_mock_returns_realclip` | Same as above for T5 | `ANVILML_WORKER_MOCK=1` | `load("/fake/path", None)` | Same assertions | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_no_torch_import` | No torch import at module level in mock mode | `ANVILML_WORKER_MOCK=1` | Module re-import after torch removal from sys.modules | `"torch" not in sys.modules`, `can_handle` and `load` callable | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_no_torch_import` | Same import isolation for CLIP-L | `ANVILML_WORKER_MOCK=1` | Same | Same | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import -v` exits 0 |
| `worker/tests/test_arch_clip_t5.py` | `test_load_mock_no_torch_import` | Same import isolation for T5 | `ANVILML_WORKER_MOCK=1` | Same | Same | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py::test_load_mock_no_torch_import -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | (all existing tests) | No regression in `LoadClip.execute()` mock-mode path | `ANVILML_WORKER_MOCK=1` | Existing test inputs | All existing assertions pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |

## CI Impact

No CI changes required. The task modifies only Python source files in the worker module. CI already runs `ANVILML_WORKER_MOCK=1 pytest worker/tests/` which will pick up the modified files. No new test files, no new CI gates, no new file types.

## Platform Considerations

None identified. The `.to(device)` API is cross-platform (works identically on Linux, Windows, and macOS). The device string `"cpu"` or `"cuda:0"` is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `model.to(device)` returns a new reference for some PyTorch module types — assigning the return value (`model = model.to(device)`) is required; failing to assign would leave the original CPU model in place. | Low | High | Always assign the return value: `model = model.to(device)`. This is the established PyTorch pattern and is explicitly documented in the approach step. |
| Adding `device=device` to the mock-mode `RealClip()` call could be a breaking change if `RealClip.__init__` doesn't accept a 3rd positional argument. | Very Low | Medium | Confirmed by reading `loader.py` line 140: `RealClip.__init__` already has `device: str = "cpu"` as the third parameter. The mock path already constructs `RealClip(MockTokenizer(), MockTextEncoder())` — passing `device=device` is a positional argument that matches the existing signature. |
| The `load()` function signature change could break external code that imports and calls `load()` with positional arguments beyond two. | Very Low | Medium | The task context explicitly states that existing mock-mode tests call `load("/fake/path", None)` positionally with two arguments, and the default `device="cpu"` preserves this. Only code that passes three or more positional arguments would be affected, and no such code exists in the repository. |
| `LoadClip.execute()` accesses `self.ctx.device` — if `self.ctx` is not yet initialized or `device` is missing, this would raise an `AttributeError`. | Low | High | `self.ctx` is a `NodeContext` that is always initialized before `execute()` is called (established by the node system). The design doc §10.4a and the `NodeContext` contract both guarantee `device` is present. Confirmed by reading `worker/nodes/base.py` where `NodeContext.device` is defined. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py worker/tests/test_arch_clip_t5.py worker/tests/test_nodes_loader.py -v` exits 0
- [ ] `for f in qwen3 clip_l t5; do grep -n "\.to(device)" worker/nodes/arch/clip/$f.py || exit 1; done` exits 0 (all three files contain a `.to(device)` call)
- [ ] `grep -n "device=self.ctx.device" worker/nodes/loader.py` exits 0 (at least one match inside `LoadClip.execute()`)
- [ ] `grep -n "def load(model_id: str, torch_dtype: Any, device: str = \"cpu\")" worker/nodes/arch/clip/qwen3.py worker/nodes/arch/clip/clip_l.py worker/nodes/arch/clip/t5.py` exits 0 (all three files have the widened signature)
