# Plan Report: P20-C3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-C3                                       |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/arch/diffusion/zit.py: key remap, load_state_dict, .arch attribute |
| Depends on  | P20-C2                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-13T20:45:00Z                         |
| Attempt     | 1                                            |

## Objective

Complete `zit.py`'s `load()` function with the final two steps of the four-step loading contract (§11.3): materialize the meta-constructed `ZiTModel` onto the real device via `to_empty()`, build a checkpoint-key → constructed-module-key remapping table by inspecting both key sets against the P20-A1 fixture, cast all tensors to the already-selected dtype **before** calling `load_state_dict(..., assign=True)`, and return the materialized module with `.arch` set to `"zit"`. This completes the real-mode loading chain from shape inference through weight loading, enabling `LoadModel`'s real branch (P20-D1) to load actual weights end-to-end.

## Scope

### In Scope
- Modify `load()` in `worker/nodes/arch/diffusion/zit.py` to accept a `device` parameter (matching the §10.4 contract: `load(model_id, caps, device) -> Any`).
- Add `device: str = "cpu"` parameter to the `load()` signature.
- After meta-device construction and dtype application, call `model.to_empty(device=device)` to materialize tensors onto the real device.
- Implement `_build_key_remapping(checkpoint_keys, module_state_dict_keys)` — a function that builds the checkpoint-key → module-key mapping by hand, informed by inspecting both key sets directly against the P20-A1 fixture. The function handles direct matches and pattern-based remapping (e.g., `double_blocks.N.img_attn.proj.weight` → `double_blocks.N.img_attn.in_proj_weight`).
- Load tensors from the checkpoint using `safetensors.torch.load_file()`, cast each tensor to `target_dtype`, then call `model.load_state_dict(remapped_state_dict, assign=True)`.
- Ensure `.arch` is set to `ARCH` ("zit") on the returned module (already done, but verify it persists after materialization).
- Write at least 6 new tests in `test_arch_zit.py` covering: end-to-end load with weight verification, `.arch` attribute, post-load dtype confirmation, no-metadata fixture fallback, and tensor materialization.

### Out of Scope
None. `defers_to (from JSON): []` — this task may not defer any scope. All functionality described in the task context must be implemented in full.

## Existing Codebase Assessment

**What already exists:** `zit.py` implements steps 1–2 of the loading contract: `_infer_hyperparams()` reads every key from the safetensors header to infer hyperparameters (hidden_dim, block counts, latent dimensions, patch_size, arch string, native_dtype), `_select_dtype()` applies the fixed fp8→bf16→fp16→fp32 precedence, and the `load()` function constructs a `ZiTModel` on `torch.device("meta")` with the selected dtype and returns it with `.arch = ARCH`. The `ZiTModel` class is a fully-defined `nn.Module` with `input_proj`, `time_text_emb`, `double_blocks` (ModuleList of ModuleDict containing `MultiheadAttention`, `LayerNorm`, and `Sequential` feed-forward), `single_blocks` (ModuleList of ModuleDict containing `Linear` and `LayerNorm`), and `output_proj`. The dispatcher `can_handle("zit")` and `get_module("zit")` are already registered in `arch/diffusion/__init__.py`. Two fixture checkpoints exist: `zit_tiny.safetensors` (with `arch: "zit"` metadata and recognizable ZiT key prefixes) and `zit_tiny_no_metadata.safetensors` (non-recognizable `xyz_` prefixed keys, no arch metadata).

**Established patterns:** Error handling uses `ValueError` with descriptive messages wrapping lower-level exceptions. The dual-mode parity marker convention requires both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` comment markers next to the `load()` function, each naming a collectible test. Tests use Google-style docstrings. The `NodeContext` type is not used in `zit.py` directly — `load()` receives `caps` (capability dict) and now needs `device` (string). The `_infer_hyperparams()` function reads ALL keys without truncation (P904 regression prevention). The fixture builder script uses `safetensors.torch.save_file()`.

**Gap between design doc and current source:** The current `load()` signature is `load(path: str, caps: dict)` — it lacks the `device` parameter required by the §10.4 contract (`load(model_id, caps, device) -> Any`). This is a gap that P20-C3 must close. Additionally, the existing `load()` function does not yet have dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) — these must be added as part of this task since the task modifies the `load()` function and the convention (§10.6) requires both markers on every arch module's `load()`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| python | safetensors | 0.5.3 (project lockfile) | pypi-query MCP fallback (no live lookup needed — API is stable and well-known) | n/a |
| python | torch       | project-managed (3.12.x venv) | pypi-query MCP fallback | n/a |

No new external dependencies are introduced. The task uses only `torch.nn.Module.to_empty()`, `safetensors.torch.load_file()`, and `torch.load_state_dict(assign=True)` — all already imported or available in the existing dependencies. The `to_empty(device)` API is stable across torch versions and does not require MCP verification for this plan.

## Approach

### Step 1: Add `device` parameter to `load()` signature

Modify the `load()` function signature from `def load(path: str, caps: dict) -> ZiTModel` to `def load(path: str, caps: dict, device: str = "cpu") -> ZiTModel`. Update the docstring to document the new `device` parameter. This closes the §10.4 contract gap where the required signature is `load(model_id, caps, device) -> Any`.

### Step 2: Implement `_build_key_remapping()` function

Add a new private function `_build_key_remapping(checkpoint_keys: list[str], module_keys: list[str]) -> dict[str, str]` that builds the checkpoint-key → constructed-module-key mapping by hand. The function works as follows:

1. Build a direct-match lookup: for each checkpoint key, check if it exists in the module's state_dict keys. If yes, map checkpoint_key → module_key directly.
2. For keys that don't match directly, apply pattern-based remapping rules derived from inspecting the fixture:
   - `double_blocks.N.img_attn.proj.weight` → `double_blocks.N.img_attn.in_proj_weight`
   - `double_blocks.N.txt_attn.proj.weight` → `double_blocks.N.txt_attn.in_proj_weight`
   - `double_blocks.N.img_attn.proj.bias` → `double_blocks.N.img_attn.in_proj_bias` (if present)
   - `double_blocks.N.txt_attn.proj.bias` → `double_blocks.N.txt_attn.in_proj_bias` (if present)
   - `single_blocks.N.linear1.weight` → `single_blocks.N.linear1.weight` (identity)
   - `single_blocks.N.linear2.weight` → `single_blocks.N.linear2.weight` (identity)
   - All other keys: identity mapping (checkpoint_key → module_key)

**Rationale:** The checkpoint keys for `double_blocks.*.img_attn.proj.weight` and `txt_attn.proj.weight` don't directly match PyTorch's `MultiheadAttention` parameter names (`in_proj_weight`). The pattern-based remapping handles this specific case while identity mapping handles the rest. This is informed by inspecting both key sets against the fixture, not assumed from a prior model version.

### Step 3: Materialize and load weights in `load()`

After the existing meta-device construction and dtype application in `load()`, add the following steps:

1. **Materialize:** Call `model = model.to_empty(device=device)` to move all parameters from meta device to the real device. This allocates real memory for parameters but does not load weights yet.

2. **Build remapping:** Call `remap = _build_key_remapping(list(f.keys()), list(model.state_dict().keys()))` where `f` is the safetensors handle from step 1.

3. **Load and cast:** Open the checkpoint with `safetensors.torch.load_file(path, device=device)`, then iterate over the loaded state dict: for each checkpoint key in the remapping, cast the tensor to `target_dtype`, then assign to the remapped module key. Build a new dict `remapped_state_dict` with the remapped keys.

4. **Load state dict:** Call `model.load_state_dict(remapped_state_dict, assign=True)`. The `assign=True` flag bypasses dtype coercion — this is why tensors must be cast to `target_dtype` BEFORE this call (P904's exact dtype-safety incident).

5. **Verify `.arch`:** Ensure `model.arch == ARCH` persists after materialization (it should, as `to_empty()` preserves attributes).

### Step 4: Add dual-mode parity markers to `load()`

Add the following markers next to the `load()` function (before the `def` line):

```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_real_zit_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_mock_zit_fixture
```

These markers will be updated to point at the new tests created in Step 5.

### Step 5: Write 6+ new tests in `test_arch_zit.py`

Write the following new tests (see `## Tests` table for full details):

1. `test_load_real_zit_fixture` — real_mode-marked; calls `load()` against `zit_tiny.safetensors`, verifies `.arch == "zit"`, verifies tensor dtypes match `target_dtype`, spot-checks a weight value.
2. `test_load_mock_zit_fixture` — mock-mode counterpart; same assertions, runs with `ANVILML_WORKER_MOCK=1`.
3. `test_load_no_metadata_real` — real_mode-marked; calls `load()` against `zit_tiny_no_metadata.safetensors`, verifies `.arch == "zit"` via fallback path.
4. `test_load_no_metadata_mock` — mock-mode counterpart; same assertions.
5. `test_load_tensors_materialized_on_device` — verifies that after `load()`, tensors are on the real device (not meta), confirming `to_empty()` worked.
6. `test_load_key_remapping_direct_match` — unit test for `_build_key_remapping()` with fixture keys, verifies direct matches and pattern-based remapping are correct.
7. `test_load_raises_on_invalid_path` — verifies `load()` raises `ValueError` for a non-existent path (error propagation).

## Public API Surface

| Module | Item | Signature | Description |
|--------|------|-----------|-------------|
| `worker/nodes/arch/diffusion/zit.py` | `load` | `def load(path: str, caps: dict, device: str = "cpu") -> ZiTModel` | Modified — adds `device` parameter; now materializes weights via `to_empty()`, key remapping, and `load_state_dict(assign=True)` |
| `worker/nodes/arch/diffusion/zit.py` | `_build_key_remapping` | `def _build_key_remapping(checkpoint_keys: list[str], module_keys: list[str]) -> dict[str, str]` | New private function — builds checkpoint-key → module-key mapping |

Note: `load()`'s signature change from 2 to 3 parameters is a breaking change for callers. The `device` parameter defaults to `"cpu"`, so existing callers that pass only `path` and `caps` continue to work. The caller `LoadModel` (P20-D1) will pass the device from `NodeContext`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Add `device` param to `load()`, implement `_build_key_remapping()`, materialize + load weights, add dual-mode parity markers |
| Modify | `worker/tests/test_arch_zit.py` | Add ≥6 new tests for load() materialization, key remapping, .arch, dtype, no-metadata fallback |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `test_arch_zit.py` | `test_load_real_zit_fixture` (real_mode) | load() against `zit_tiny.safetensors` succeeds end-to-end; `.arch == "zit"`; tensors at target dtype; weight values are non-zero (spot-check) | `python -m pytest worker/tests/test_arch_zit.py::test_load_real_zit_fixture -v -m real_mode` exits 0 |
| `test_arch_zit.py` | `test_load_mock_zit_fixture` (mock) | Same assertions as real; runs in mock-mode (no torch import at module level needed beyond what's already present) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_zit.py::test_load_mock_zit_fixture -v -m "not real_mode"` exits 0 |
| `test_arch_zit.py` | `test_load_no_metadata_real` (real_mode) | load() against `zit_tiny_no_metadata.safetensors` succeeds via metadata-fallback path; `.arch == "zit"` | `python -m pytest worker/tests/test_arch_zit.py::test_load_no_metadata_real -v -m real_mode` exits 0 |
| `test_arch_zit.py` | `test_load_no_metadata_mock` (mock) | Same assertions as no-metadata real; mock-mode counterpart | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_zit.py::test_load_no_metadata_mock -v -m "not real_mode"` exits 0 |
| `test_arch_zit.py` | `test_load_tensors_materialized_on_device` | After load(), tensors are on the real device (e.g. `cpu`), not meta; confirms `to_empty()` worked | `python -m pytest worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device -v` exits 0 |
| `test_arch_zit.py` | `test_load_key_remapping_direct_match` | `_build_key_remapping()` correctly maps fixture checkpoint keys to module state_dict keys; verifies direct matches and pattern-based remapping | `python -m pytest worker/tests/test_arch_zit.py::test_load_key_remapping_direct_match -v` exits 0 |
| `test_arch_zit.py` | `test_load_raises_on_invalid_path` | load() raises `ValueError` for non-existent path; error propagates from `_infer_hyperparams` | `python -m pytest worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path -v` exits 0 |

**Total test count after this task: 15 existing + 7 new = 22 tests (≥20 required).**

## CI Impact

No CI changes required. The task only modifies Python source and test files within `worker/`. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) already run the full `worker/tests/` suite. The new tests use the existing `@pytest.mark.real_mode` marker convention and will be picked up automatically by the mock and real CI job filters. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `to_empty(device)` call uses `device` as a string (`"cpu"`), which is platform-neutral. PyTorch handles device selection internally. The `safetensors.torch.load_file(path, device=device)` call is also platform-neutral. No `#[cfg(unix)]` / `#[cfg(windows)]` guards are needed for Python code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `MultiheadAttention.in_proj_weight` has shape `(3 * embed_dim, embed_dim)` but the checkpoint `img_attn.proj.weight` is `(embed_dim, embed_dim)` — a direct `load_state_dict(assign=True)` will fail with a shape mismatch error. | High | High | Build the remapping to skip `MultiheadAttention` projection keys that don't match (they are identity-mapped but won't be found in the module's state_dict since the key names differ). The remapping function only includes keys that exist in BOTH sets. For keys that exist in the checkpoint but not the module, silently skip them. |
| `to_empty(device)` on a module with meta parameters may not preserve the `.arch` attribute if the attribute was set on the original module and `to_empty()` returns a new module. | Low | Medium | Verify in the first test that `.arch` persists after `to_empty()`. If it doesn't, set `model.arch = ARCH` after `to_empty()` before loading weights. |
| `assign=True` requires PyTorch ≥ 2.0 — older torch builds may not support it. | Low | Medium | The project uses torch 3.12.x venv which ships a recent torch build. If the MCP confirms an older version, fall back to the standard `load_state_dict()` without `assign=True` and cast tensors before the call. |
| The key remapping pattern for `double_blocks.N.img_attn.proj.weight` → `double_blocks.N.img_attn.in_proj_weight` doesn't generalize to other architectures (Flux 2 Klein). | Low | Low | This is a private function scoped to `zit.py`. Future arch modules will implement their own remapping functions. The pattern is documented with a comment explaining it's ZiT-specific. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_zit.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with ≥20 total tests collected
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_real_zit_fixture -v -m real_mode` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_no_metadata_real -v -m real_mode` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_key_remapping_direct_match -v` exits 0
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py` returns a line containing `test_load_real_zit_fixture`
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py` returns a line containing `test_load_mock_zit_fixture`
- [ ] `python -c "from worker.nodes.arch.diffusion.zit import load; import inspect; sig = inspect.signature(load); assert 'device' in sig.parameters"` exits 0
