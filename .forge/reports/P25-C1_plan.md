# Plan Report: P25-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P25-C1                                      |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description | worker/nodes/arch/diffusion/flux2klein.py: meta construction + dtype (4B) |
| Depends on  | P25-B2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-22T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `worker/nodes/arch/diffusion/flux2klein.py` with `load()`'s steps 2–3 of the four-step loading contract (ANVILML_DESIGN.md §11.3): construct the Flux 2 Klein diffusion transformer on `torch.device("meta")` using only P25-B1's inferred hyperparameters, select the compute dtype per §11.5's precedence chain reading `ctx.caps`, materialize onto the target device, and verify construction succeeds against both the regular and no-metadata fixtures. Step 4 (key remap + weight load) is deferred to P25-C2. This completes the meta-construction and dtype-selection half of `load()`, enabling the next task to focus exclusively on the checkpoint-key remapping logic.

## Scope

### In Scope
- Define `Flux2KleinModel(nn.Module)` — a torch.nn.Module subclass mirroring Flux 2 Klein's architecture (double_blocks with modulated cross-attention, single_blocks, final_layer with adaLN_modulation) constructed on meta-device from hyperparameters.
- Implement `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype` — dtype selection per §11.5 precedence chain (fp8 → bf16 → fp16 → fp32).
- Implement `load(path: str, caps: dict, device: str = "cpu") -> Flux2KleinModel` — steps 2–3 of the four-step contract: meta construction with selected dtype, materialize via `to_empty()`, zero-initialize parameters, verify `.arch` attribute.
- Add `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers to `load()` per §10.6 (with §17.2 exception: MOCK_PATH_VERIFIED names a collection-safety test since load() has no mock branch).
- Write ≥4 new tests in `test_arch_flux2klein.py`: meta construction against regular fixture, dtype selection for 4 caps combinations, meta construction against no-metadata fixture.
- Total test count in file reaches ≥11 (from current 8, adding ≥4 new tests).

### Out of Scope
- Step 4 of load(): checkpoint-key remapping and `load_state_dict(assign=True)` weight loading (P25-C2).
- `compute_latent_shape()` and `sample()` (P25-D1).
- `.arch` attribute verification against actual loaded weights (only construction verified here; P25-C2 verifies loaded weights).
- Pipeline assembly and scheduler wiring (P25-D1).
- Forward pass implementation for Flux2KleinModel (deferred to P25-D1).

## Existing Codebase Assessment

**What already exists:** `flux2klein.py` (411 lines) has `_infer_hyperparams()`, `_infer_hyperparams_inner()`, `_safetensors_dtype_to_canonical()`, and `can_handle()` — all torch-free, importable without torch installed. The `_SAFETENSORS_DTYPE_MAP` and `_safetensors_dtype_to_canonical()` already exist in flux2klein.py, so the dtype string normalization layer is in place. The two fixture files (`flux2klein4b_tiny.safetensors` and `flux2klein4b_tiny_no_metadata.safetensors`) exist from P25-A1. The test file has 8 tests covering `_infer_hyperparams()` (4 tests) and `can_handle()`/dispatch (4 tests).

**Established patterns to follow:**
- `zit.py` is the exact reference: it uses `torch.nn` primitives (Linear, LayerNorm, MultiheadAttention, Sequential, GELU) for model construction — not diffusers' layer classes. The model class inherits from a conditional base (`_ModuleBase = nn.Module if nn is not None else object`) for import safety.
- `_select_dtype()` in zit.py follows the exact §11.5 precedence: fp8 (caps.fp8 AND native==fp8) → bf16 → fp16 → fp32, returning `torch.float8_e4m3fn`, `torch.bfloat16`, `torch.float16`, or `torch.float32`.
- `load()` pattern: meta construction → `.to(dtype)` → `.to_empty(device)` → zero-initialize → verify `.arch` → (deferred: remap + load).
- Import guard: torch imports are wrapped in `try/except ImportError` at module level. `load()` checks `torch is None` at the top and raises `RuntimeError`.
- Dual-mode markers: `REAL_PATH_VERIFIED` names a real-mode test; `MOCK_PATH_VERIFIED` names a collection-safety test (per §17.2 exception for arch-module `load()`).

**Gap between design doc and current source:** `flux2klein.py` does not yet have a model class (`Flux2KleinModel`), `_select_dtype()`, or `load()`. The Flux 2 Klein architecture differs from ZiT — it uses adaptive LayerNorm modulation (`img_mod`, `txt_mod`) in double blocks (ZiT does not), and has a `final_layer` with `adaLN_modulation` (ZiT has `output_proj` instead). The model class must be built from scratch, not copied from zit.py.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| python | torch      | (project env)   | n/a            | n/a — standard PyTorch, no special features |
| python | diffusers  | (project env)   | n/a            | n/a — not used for model construction in this task; zit.py uses torch.nn primitives only |
| python | safetensors| (project env)   | n/a            | n/a — `safe_open` and `load_file` already imported via guard |

No new external dependencies are introduced. The task uses only torch.nn primitives (Linear, LayerNorm, MultiheadAttention, Sequential, GELU) and `torch.device("meta")`, all of which are part of the standard torch package already in the project's requirements.

## Approach

1. **Add `Flux2KleinModel` class** to `flux2klein.py`, following `ZiTModel`'s pattern:
   - Set `_ModuleBase = nn.Module if nn is not None else object` (already guarded via the existing try/except block at module level — confirm `nn` is set in the guard, then define the class).
   - `__init__(self, hyperparams: dict[str, Any]) -> None`: construct the model on meta-device from inferred hyperparameters. The Flux 2 Klein architecture consists of:
     - `input_proj`: `nn.Linear(latent_dim, hidden_dim)` where `latent_dim = latent_channels * patch_size²`.
     - `time_text_emb`: `nn.Linear(hidden_dim, hidden_dim)` (time-step + text embedding projection).
     - `double_blocks`: `nn.ModuleList` of modulated cross-attention blocks. Each block contains:
       - `img_mod`: `nn.Linear(hidden_dim, hidden_dim * 6)` — generates modulation parameters for 6 LayerNorm layers in the double block (2 per attention sub-layer × 3 sub-layers).
       - `txt_mod`: `nn.Linear(hidden_dim, hidden_dim * 6)` — same for text path.
       - `img_attn`: `nn.MultiheadAttention(embed_dim=hidden_dim, num_heads=hidden_dim // 64, batch_first=True)`.
       - `txt_attn`: `nn.MultiheadAttention(embed_dim=hidden_dim, num_heads=hidden_dim // 64, batch_first=True)`.
       - `img_norm1`, `img_norm2`, `txt_norm1`, `txt_norm2`: `nn.LayerNorm(hidden_dim)`.
       - `img_mlp`: `nn.Sequential(nn.Linear(hidden_dim, hidden_dim * 4), nn.GELU(), nn.Linear(hidden_dim * 4, hidden_dim))` — SwiGLU-style MLP.
       - `txt_mlp`: same structure as `img_mlp`.
     - `single_blocks`: `nn.ModuleList` of linear transformation blocks. Each contains:
       - `linear1`: `nn.Linear(hidden_dim, hidden_dim * 4)`.
       - `linear2`: `nn.Linear(hidden_dim * 4, hidden_dim)`.
       - `norm`: `nn.LayerNorm(hidden_dim)`.
     - `final_layer.adaLN_modulation`: `nn.Linear(hidden_dim, hidden_dim * 2)` — modulation for final LayerNorm.
     - `final_layer.linear`: `nn.Linear(hidden_dim, latent_dim)`.
   - Set `self.arch: str = "flux2klein"` after construction.
   - Add a `forward()` method stub that accepts `(x, timestep, conditioning)` and returns a tensor — this is the interface `sample()` will call in P25-D1. For now, implement a minimal pass-through (project → blocks → output) that is structurally correct but does not implement the full modulation math (that math is deferred to P25-D1 when `sample()` is implemented).

2. **Implement `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype`**:
   - Copy the exact logic from `zit.py`'s `_select_dtype()`: fp8 (caps.fp8 AND native==fp8) → bf16 → fp16 → fp32.
   - Return `torch.float8_e4m3fn`, `torch.bfloat16`, `torch.float16`, or `torch.float32`.
   - Document the precedence chain with inline comments explaining each branch's rationale.

3. **Implement `load(path: str, caps: dict, device: str = "cpu") -> Flux2KleinModel`**:
   - Guard: check `torch is None` at top, raise `RuntimeError` with clear message.
   - Call `_infer_hyperparams(path)` to get hyperparameters (step 1, already exists).
   - Call `_select_dtype(caps, hyperparams["native_dtype"])` to select compute dtype.
   - Log DEBUG: dtype selection result and the caps field that drove it.
   - Construct `Flux2KleinModel(hyperparams)` inside `with torch.device("meta"):` context.
   - Apply dtype: `model.to(target_dtype)`.
   - Log DEBUG: materialization parameters (hidden_dim, block counts, device).
   - Materialize: `model = model.to_empty(device=device)`.
   - Zero-initialize: iterate `model.parameters()` and `model.buffers()`, call `.data.zero_()` on each.
   - Verify `.arch`: assert `model.arch == ARCH`, re-set if not (safety net per zit.py pattern).
   - Return `model` — do NOT implement key remapping or weight loading (that's P25-C2).
   - Add `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers above the function.

4. **Write tests in `test_arch_flux2klein.py`** (≥4 new tests, bringing total to ≥11):
   - `test_load_meta_construction_regular_fixture`: Call `load()` against the regular 4B fixture, verify the returned model is a `Flux2KleinModel` instance, has `.arch == "flux2klein"`, and all parameters are on the target device.
   - `test_load_dtype_fp8_caps`: Mock `caps={"fp8": True, "bf16": True, "fp16": True}` with a fixture whose native_dtype is "fp8" (or construct a test that verifies `_select_dtype` returns `torch.float8_e4m3fn`).
   - `test_load_dtype_bf16_caps`: Mock `caps={"fp8": False, "bf16": True, "fp16": True}` — verify `_select_dtype` returns `torch.bfloat16`.
   - `test_load_dtype_fp16_caps`: Mock `caps={"fp8": False, "bf16": False, "fp16": True}` — verify `_select_dtype` returns `torch.float16`.
   - `test_load_dtype_fp32_caps`: Mock `caps={"fp8": False, "bf16": False, "fp16": False}` — verify `_select_dtype` returns `torch.float32`.
   - `test_load_meta_construction_no_metadata_fixture`: Call `load()` against the no-metadata fixture, verify construction succeeds (same checks as regular fixture).
   - `test_collection_safety_load_import`: Subprocess-based test that imports `flux2klein` with `ANVILML_WORKER_MOCK=1` and no torch, confirming the module imports without error (for MOCK_PATH_VERIFIED marker).

5. **Add logging** per §11.7:
   - DEBUG: dtype selection result and driving caps field in `_select_dtype`.
   - DEBUG: materialization parameters in `load()`.
   - INFO: model loaded (deferred to P25-C2 when weight loading is added; for now, no INFO log point is needed since step 3 is incomplete).

## Public API Surface

| Item | Module Path | Signature | Description |
|------|-------------|-----------|-------------|
| Class | `worker.nodes.arch.diffusion.flux2klein` | `class Flux2KleinModel(nn.Module)` | Flux 2 Klein diffusion transformer constructed on meta-device from hyperparameters. Has `.arch = "flux2klein"`, `input_proj`, `time_text_emb`, `double_blocks`, `single_blocks`, `final_layer`, and `forward()` method. |
| Function | `worker.nodes.arch.diffusion.flux2klein` | `def _select_dtype(caps: dict, native_dtype: str) -> torch.dtype` | Select compute dtype per §11.5 precedence chain. Private helper. |
| Function | `worker.nodes.arch.diffusion.flux2klein` | `def load(path: str, caps: dict, device: str = "cpu") -> Flux2KleinModel` | Steps 2–3 of four-step loading contract: meta construction + dtype selection + materialization. Returns model with `.arch` set. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/flux2klein.py` | Add `Flux2KleinModel` class, `_select_dtype()`, `load()` function with markers and logging |
| MODIFY | `worker/tests/test_arch_flux2klein.py` | Add ≥4 new tests: meta construction, dtype selection (4 caps combos), no-metadata fixture, collection safety |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_flux2klein.py` | `test_load_meta_construction_regular_fixture` (real) | `load()` against regular 4B fixture returns Flux2KleinModel with .arch=="flux2klein", parameters on target device | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_meta_construction_regular_fixture -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_load_dtype_fp8_caps` (real) | `_select_dtype` with fp8 caps + fp8 native → torch.float8_e4m3fn | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_dtype_fp8_caps -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_load_dtype_bf16_caps` (real) | `_select_dtype` with bf16 caps → torch.bfloat16 | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_dtype_bf16_caps -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_load_dtype_fp16_caps` (real) | `_select_dtype` with fp16 caps (no bf16) → torch.float16 | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_dtype_fp16_caps -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_load_dtype_fp32_caps` (real) | `_select_dtype` with no fp8/bf16/fp16 caps → torch.float32 | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_dtype_fp32_caps -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_load_meta_construction_no_metadata_fixture` (real) | `load()` against no-metadata fixture returns Flux2KleinModel with .arch=="flux2klein" | `python -m pytest worker/tests/test_arch_flux2klein.py::test_load_meta_construction_no_metadata_fixture -v` |
| `worker/tests/test_arch_flux2klein.py` | `test_collection_safety_load_import` (mock) | Module imports successfully with ANVILML_WORKER_MOCK=1 and no torch (MOCK_PATH_VERIFIED for load()) | `python -m pytest worker/tests/test_arch_flux2klein.py::test_collection_safety_load_import -v` |

## CI Impact

No CI job changes required. The new tests in `test_arch_flux2klein.py` are collected by the existing pytest runs:
- `worker-linux-mock` CI job: collects and runs mock-mode tests (the collection-safety test runs here).
- `worker-linux-real` CI job: runs real-mode tests including all load() and dtype tests.
- No new pytest markers, no new CI jobs, no changes to `pyproject.toml` or `pytest.ini`.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `torch.device("meta")` context manager, `torch.nn` primitives, and `to_empty()` are all platform-neutral PyTorch operations. The fixture files are cross-platform safetensors binaries.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Flux 2 Klein's actual architecture key patterns in the fixture may not match the expected `double_blocks.N.img_attn` / `single_blocks.N.linear1` naming — the fixture was built by P25-A1 with its own key convention, and the model class must match those keys for the later `load_state_dict` step. | Medium | High | Read the fixture's actual keys (via `safe_open(..., framework="np").keys()`) before writing the model class. Align the constructed module's `state_dict()` keys with the fixture's key naming convention — this is the same approach zit.py uses where the fixture's simplified keys are handled by shape filtering in the remapping step. |
| The Flux 2 Klein architecture uses adaptive LayerNorm modulation (img_mod, txt_mod) that produces per-token modulation parameters — implementing the full modulation math in `forward()` may be complex. However, `forward()` is only called by `sample()` in P25-D1, not by this task. | Low | Medium | Implement a minimal `forward()` that passes data through the layers without modulation for this task. P25-D1 will implement the full modulation math when `sample()` is added. Document the stub with an inline comment. |
| `_select_dtype` in flux2klein.py duplicates zit.py's implementation — if zit.py's logic changes, flux2klein.py may diverge. | Low | Low | The precedence chain is a fixed design contract (§11.5), not an implementation detail. Duplication is intentional and expected per the architecture module pattern (each module owns its own dtype selection). A shared utility would introduce a cross-module dependency that the design doc's module isolation principle discourages. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py --collect-only -q 2>/dev/null | grep "tests collected" | grep -q "11"` — file collects ≥11 tests (from current 8, adding ≥4 new)
- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py -v 2>&1; echo "EXIT:$?"` — exits 0
- [ ] `grep -c "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py` — ≥1 (load() has marker)
- [ ] `grep -c "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py` — ≥1 (load() has marker)
- [ ] `grep "def load(" worker/nodes/arch/diffusion/flux2klein.py` — function exists with signature `def load(path: str, caps: dict, device: str = "cpu") -> Flux2KleinModel`
- [ ] `grep "def _select_dtype(" worker/nodes/arch/diffusion/flux2klein.py` — function exists
- [ ] `grep "class Flux2KleinModel" worker/nodes/arch/diffusion/flux2klein.py` — class exists
