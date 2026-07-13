# Plan Report: P20-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-C2                                      |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/arch/diffusion/zit.py: dtype selection per InferenceCaps |
| Depends on  | P20-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T17:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Add dtype selection to `zit.py`'s `load()` function, implementing the fixed precedence chain defined in ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND checkpoint native dtype is fp8) -> bf16 -> fp16 -> fp32. The selected dtype is applied to the meta-device construction from P20-C1. The function signature changes from `load(path: str)` to `load(path: str, caps: dict)` to accept the worker's capability dict. This produces a ZiTModel whose meta-parameters carry the correct dtype metadata, with four new tests exercising each precedence branch.

## Scope

### In Scope
- Modify `load()` signature in `worker/nodes/arch/diffusion/zit.py` to accept a `caps: dict` parameter (the Python-side `InferenceCaps` dict with keys `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`).
- Implement `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype` helper function that encodes the §11.5 fixed precedence.
- Detect the checkpoint's native dtype from the safetensors header (read from the first weight tensor's dtype info via `safe_open`).
- Apply the selected dtype to the meta-device construction in `load()` so meta-parameters carry the correct dtype.
- Update the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` dual-mode parity markers on `load()` to point at the new dtype test names.
- Add 4 new tests in `worker/tests/test_arch_zit.py`: one per precedence branch (fp8-capable, bf16-only, fp16-only, fp32-fallback).
- Add 1 test verifying that when caps have multiple capabilities, the highest-precedence branch wins (fp8 beats bf16).

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals. The weight materialization and `load_state_dict()` step is P20-C3. The `sample()` and `compute_latent_shape()` methods are out of scope for Phase 20 entirely.

## Existing Codebase Assessment

The codebase at this point has `zit.py` with `_infer_hyperparams()` (P20-B1), `can_handle()` (P20-B2), and `load()` (P20-C1). The `load()` function currently:
1. Calls `_infer_hyperparams(path)` to get hyperparameters from the safetensors header.
2. Constructs `ZiTModel(hyperparams)` inside `with torch.device("meta"):` — all parameters are meta-tensors with default dtype (`torch.float32`).
3. Sets `model.arch = ARCH`.

The `NodeContext` in `base.py` carries `caps` as a dict (not a Rust `InferenceCaps` struct — the Python side uses a plain dict with keys matching the Rust struct field names). The `probe_capabilities()` function in `capability.py` returns exactly this dict shape: `{"fp32": bool, "fp16": bool, "bf16": bool, "fp8": bool, "fp4": bool, "flash_attention": bool}`.

The existing 12 tests in `test_arch_zit.py` exercise hyperparameter inference, dispatch registration, and meta-device construction. The dual-mode parity markers on `load()` currently point at `test_load_meta_construction_real` and `test_load_meta_construction_mock` — these will need updating.

No new external dependencies are needed — only `torch` (already imported) and `safetensors` (already imported).

## Resolved Dependencies

None. This task uses only `torch` and `safetensors`, both already imported in `zit.py` and already in the project's requirements. No new crates, packages, or feature flags are introduced.

## Approach

1. **Read checkpoint native dtype in `_infer_hyperparams_inner()`.**  
   The `_infer_hyperparams_inner()` function already has access to the `safe_open` handle `f`. Add a native dtype detection step: iterate over `f.keys()`, call `f.get_tensor_info(key).dtype` on each key, and collect the dtype from the first weight tensor (not bias or other metadata). The safetensors header stores dtype as strings like `"F32"`, `"F16"`, `"BF16"`, `"F8_E4M3"`, `"I32"`, etc. Map these to a canonical string for the dtype selection function. Return the native dtype string alongside the existing hyperparams dict (as a new key `"native_dtype"`).  
   *Rationale: We need the checkpoint's native dtype to decide whether fp8 is viable (step 1 of the precedence requires both caps.fp8 AND native_dtype == fp8).*

2. **Implement `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype`.**  
   Add a new module-level function in `zit.py` that implements the §11.5 precedence:
   - If `caps.get("fp8", False)` AND `native_dtype` maps to an FP8 format -> return `torch.float8_e4m3fn`.
   - Else if `caps.get("bf16", False)` -> return `torch.bfloat16`.
   - Else if `caps.get("fp16", False)` -> return `torch.float16`.
   - Else -> return `torch.float32`.
   
   The native dtype to canonical string mapping: `"F32"` -> `"fp32"`, `"F16"` -> `"fp16"`, `"BF16"` -> `"bf16"`, `"F8_E4M3"` or `"F8_E5M2"` -> `"fp8"`.  
   *Rationale: The native dtype string from safetensors uses uppercase abbreviations; the mapping normalises them for comparison.*

3. **Modify `load()` signature and body.**  
   Change from `def load(path: str) -> ZiTModel:` to `def load(path: str, caps: dict) -> ZiTModel:`.  
   Inside `load()`:
   - Call `_infer_hyperparams(path)` which now also returns `"native_dtype"`.
   - Call `_select_dtype(caps, hyperparams["native_dtype"])` to get `target_dtype`.
   - Apply the selected dtype to the meta-constructed module: construct on meta-device, then call `model.to(target_dtype)` which changes the dtype metadata on all meta parameters without allocating real memory.
   - Set `model.arch = ARCH`.
   - Return model.
   
   *Rationale: `model.to(dtype)` on a module with meta-device parameters changes their dtype metadata without allocating real memory. This is the standard PyTorch idiom for dtype selection before weight loading.*

4. **Add inline documentation comments.**  
   Per ANVILML_DESIGN.md §10 and ENVIRONMENT.md §10, every non-trivial decision point in `load()` must have an inline `#` comment explaining the *why*. The dtype selection chain is a non-trivial decision point — add a comment explaining which `caps` field drove each branch, following the example in ENVIRONMENT.md §10 (Python section, dtype/capability branch example).

5. **Update dual-mode parity markers on `load()`.**  
   The existing markers:
   ```
   # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_real
   # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_mock
   ```
   Update to point at the new dtype tests:
   ```
   # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real
   # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock
   ```
   *Rationale: The construction-only tests no longer pass the `caps` parameter, so they would break. The new dtype tests exercise the full load() path with caps, making them the appropriate parity markers.*

6. **Write 4 new tests for each precedence branch in `test_arch_zit.py`.**  
   Each test constructs a `caps` dict that exercises exactly one branch of the precedence, calls `load(fixture_path, caps)`, and verifies the returned model's parameters have the expected dtype metadata (via `next(model.parameters()).dtype`).
   
   - `test_dtype_selection_fp8_caps_and_native` — caps with `fp8=True` and fixture native dtype is FP8 -> `torch.float8_e4m3fn`
   - `test_dtype_selection_bf16_real` (real) and `test_dtype_selection_bf16_mock` (mock) — caps with `bf16=True, fp16=True, fp8=False` -> `torch.bfloat16`
   - `test_dtype_selection_fp16_only` — caps with `fp16=True, bf16=False, fp8=False` -> `torch.float16`
   - `test_dtype_selection_fp32_fallback` — caps with all precision flags `False` -> `torch.float32`
   
   *Rationale: Each test isolates one branch of the precedence chain. The bf16 test serves as the dual-mode parity marker tests (mock + real), satisfying Gate 4.*

7. **Write 1 additional test for precedence priority.**  
   `test_dtype_selection_fp8_beats_bf16` — caps with `fp8=True, bf16=True` and native dtype is fp8 -> `torch.float8_e4m3fn` (verifies fp8 takes precedence over bf16 when both caps are available).

8. **Remove old construction-only tests that no longer apply.**  
   Remove `test_load_meta_construction_real`, `test_load_meta_construction_mock`, `test_load_meta_device_zero_real_memory`, and `test_load_meta_construction_no_metadata_variant` since `load()` now requires a `caps` parameter and these tests do not pass it. The dtype tests cover the same invariants (model class, .arch attribute, meta device placement) while also verifying dtype selection.

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `load()` | `worker/nodes/arch/diffusion/zit.py` | `def load(path: str, caps: dict) -> ZiTModel:` — **signature changed** (added `caps: dict` parameter) |
| `_select_dtype()` | `worker/nodes/arch/diffusion/zit.py` | `def _select_dtype(caps: dict, native_dtype: str) -> torch.dtype:` — **new function** |
| `_infer_hyperparams()` return dict | `worker/nodes/arch/diffusion/zit.py` | Gains `"native_dtype": str` key — **return type extended** |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `caps` param to `load()`, add `_select_dtype()`, update `_infer_hyperparams_inner()` to detect native dtype, update parity markers, remove old tests from docstring |
| MODIFY | `worker/tests/test_arch_zit.py` | Remove 4 old construction-only tests, add 5 new dtype selection tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_arch_zit.py` | `test_dtype_selection_fp8_caps_and_native` | caps.fp8=True AND checkpoint native dtype is FP8 -> model params at `torch.float8_e4m3fn` | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native -v` |
| `test_arch_zit.py` | `test_dtype_selection_bf16_real` (real) | caps.bf16=True, fp8=False -> model params at `torch.bfloat16` | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real -v` |
| `test_arch_zit.py` | `test_dtype_selection_bf16_mock` (mock) | Same as above, mock-mode parity marker | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock -v` |
| `test_arch_zit.py` | `test_dtype_selection_fp16_only` | caps.fp16=True, bf16=False -> model params at `torch.float16` | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only -v` |
| `test_arch_zit.py` | `test_dtype_selection_fp32_fallback` | All caps False -> model params at `torch.float32` | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback -v` |
| `test_arch_zit.py` | `test_dtype_selection_fp8_beats_bf16` | caps.fp8=True, bf16=True, native fp8 -> `torch.float8_e4m3fn` (fp8 priority) | `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 -v` |

## CI Impact

No CI changes required. The new tests are collected by the existing `pytest worker/tests/` invocations in both mock-mode (`-m "not real_mode"`) and real-mode (`-m real_mode`) CI jobs. The bf16 test's mock variant runs in the mock CI job; the bf16 test's real variant runs in the real CI job. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The dtype selection logic is pure Python with no platform-specific branches. `torch.bfloat16`, `torch.float16`, `torch.float32`, and `torch.float8_e4m3fn` are all available on torch CPU builds. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The fixture checkpoint's native dtype may not be FP8, so the fp8 branch test cannot exercise the full fp8 condition (caps.fp8=True AND native_dtype is fp8). | Medium | High | Test `_select_dtype()` as a pure function with controlled inputs (mock caps dict, arbitrary native_dtype string) rather than relying on fixture properties. The fixture test only needs to verify that the full load() path works end-to-end with a specific caps combination. |
| `model.to(dtype)` on meta-device parameters may not change the dtype metadata as expected on older PyTorch versions. | Low | Medium | Verify the behavior with a quick `torch.device("meta")` + `.to(dtype)` test before writing the main tests. If it doesn't work, use `torch.set_default_dtype()` inside the meta context as an alternative. |
| Updating `load()`'s signature to require `caps` will break the existing construction-only tests that call `load(path)` without caps. | High | Medium | Remove the old construction-only tests (`test_load_meta_construction_real`, `test_load_meta_construction_mock`, `test_load_meta_device_zero_real_memory`, `test_load_meta_construction_no_metadata_variant`) and replace them with the new dtype-aware tests. The parity markers will be updated to point at the new tests. |
| The fixture's native dtype from safetensors may not match what the test expects for the fp8 branch. | Medium | Medium | Test `_select_dtype()` as a pure function with controlled inputs rather than relying on fixture properties. The fixture test only needs to verify full load() path works with a specific caps combination. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_zit.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with >=14 tests total
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py` finds a valid test name that passes `pytest --collect-only`
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py` finds a valid test name that passes `pytest --collect-only`
