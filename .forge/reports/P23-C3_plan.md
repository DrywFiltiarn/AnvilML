# Plan Report: P23-C3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-C3                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/nodes/arch/vae/zit_vae.py: key remap, load_state_dict, .arch attribute |
| Depends on  | P23-C1, P23-C2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T12:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `load()` function in `worker/nodes/arch/vae/zit_vae.py` by implementing steps 3–4 of the four-step loading contract (§11.3): materialize the meta-constructed `ZiTVaeModel` via `to_empty()`, build a checkpoint-key → module-key remapping table against the ZiT VAE fixture, cast tensors to the selected dtype BEFORE `load_state_dict(assign=True)`, and set the `.arch` attribute to `"zit_vae"`. Add dual-mode parity markers (`REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED`) to the `load()` function. Deliver at least 6 new tests in `test_arch_vae_zit.py` covering weight loading, dtype casting, `.arch` verification, and the no-metadata fixture fallback, bringing the total to ≥20.

## Scope

### In Scope
- Complete `load()` in `worker/nodes/arch/vae/zit_vae.py`: materialize via `to_empty()`, build key remapping, cast tensors, call `load_state_dict(assign=True)`, set `.arch`.
- Add `_build_key_remapping()` function specific to the ZiT VAE key namespace (never copied from `zit.py`).
- Add dual-mode parity markers (`REAL_PATH_VERIFIED`, `MOCK_PATH_VERIFIED`) to the `load()` function.
- Add at least 6 new tests in `worker/tests/test_arch_vae_zit.py`:
  - `test_load_weights_loaded_regular_fixture` — real-mode: weights actually loaded, shapes match, values spot-checked.
  - `test_load_weights_loaded_no_metadata_fixture` — real-mode: no-metadata fixture loads correctly.
  - `test_load_arch_attribute_set` — real-mode: `.arch == "zit_vae"` after load().
  - `test_load_dtype_applied_to_loaded_tensors` — real_mode: tensors at selected dtype after load().
  - `test_load_mock_returns_sentinel` — mock-mode: sentinel tensor shapes returned without torch.
  - `test_load_real_zit_vae_fixture` — real_mode: end-to-end load with full fixture.
- Update docstrings and add decision-point inline comments for the new code paths.

### Out of Scope
- `decode()` — implemented in P23-D1.
- `LoadVae` loader node integration — implemented in P23-E1.
- E2E generation chain (LoadModel → Sampler → decode) — implemented in P23-F1.
- Any changes to `worker/nodes/loader.py` or other loader files.
- Any Rust-side changes.
- Adding new fixture files (fixtures already exist from P23-A1).

defers_to (from JSON): absent

## Existing Codebase Assessment

**What already exists:** The `zit_vae.py` module has three of the four loading-contract steps implemented: `_infer_hyperparams()` (step 1, shape inference from safetensors header), `_select_dtype()` (step 2, dtype precedence), and a partial `load()` that does meta-construction and dtype application but returns early before materialization and weight loading. The `ZiTVaeModel` class is fully constructed with `encoder.block_N`, `mid_block`, and `decoder.block_N` submodules containing `Conv2d` and `GroupNorm` parameters. The fixture files (`zit_vae_tiny.safetensors` and `zit_vae_tiny_no_metadata.safetensors`) contain tensor keys in the format `encoder.blocks.N.conv.weight`, `decoder.blocks.N.conv.weight`, `mid_block.conv.weight`, etc. — note the checkpoint uses `blocks.N` (plural) while the module uses `block_N` (singular, no 's'). Fifteen tests already exist in `test_arch_vae_zit.py` covering `_infer_hyperparams()`, `can_handle()`, `get_module()`, and partial `load()` (meta construction + dtype selection only).

**Established patterns:** The `zit.py` diffusion module is the reference implementation for the loading contract. Its `load()` function follows the exact ordering: meta-construction → `.to(dtype)` → `to_empty(device)` → `.arch` check → `load_file()` → `_build_key_remapping()` → cast-to-target-dtype → shape-filter → `load_state_dict(assign=True, strict=False)` → log. The key remapping function iterates checkpoint keys, checks for direct matches in the module's state_dict, then applies regex-based pattern remapping. Dual-mode parity markers are placed as two comment lines immediately above the function signature. Tests use `@pytest.mark.real_mode` for torch-requiring tests. The fixture builder creates tensors with `torch.randn()` (defaulting to fp32).

**Gap between design doc and current source:** The current `load()` function has a `defers_to: P23-C3` comment at line 530-531 and returns the meta-constructed module before steps 3–4. The `.arch` attribute is set in `ZiTVaeModel.__init__()` (line 336) but the existing tests assert `hasattr(model, "arch")` on the meta-constructed module — after materialization, we need to verify `.arch` persists through `to_empty()`. The key remapping table must be built from scratch for the VAE namespace (different key patterns from zit.py's diffusion keys).

## Resolved Dependencies

None. This task uses only existing Python packages already imported in `zit_vae.py`: `torch`, `torch.nn`, `safetensors.torch.load_file`. No new external dependencies are introduced.

| Type   | Name          | Version verified | MCP source     | Feature flags confirmed |
|--------|---------------|-----------------|----------------|------------------------|
| python | torch         | (existing)      | (project lock) | n/a                    |
| python | safetensors   | (existing)      | (project lock) | n/a                    |

## Approach

1. **Read the existing `load()` function and the `ZiTVaeModel` state_dict keys.** The current `load()` returns after `model.to(target_dtype)` (line 567) — we need to continue from there. Inspect the actual `state_dict()` keys that `ZiTVaeModel` produces by examining the constructor: `encoder.block_N.conv.weight`, `encoder.block_N.norm.weight`, `mid_block.conv.weight`, `mid_block.norm.weight`, `decoder.block_N.conv.weight`, `decoder.block_N.norm.weight` (plus corresponding `.bias` parameters from Conv2d).

2. **Implement `_build_key_remapping()` for the ZiT VAE key namespace.** This function takes `checkpoint_keys` (from `load_file()`) and `module_keys` (from `model.state_dict().keys()`) and returns a `dict[str, str]` mapping checkpoint keys to module keys. The VAE remapping rules are:
   - **Direct match:** if a checkpoint key exists verbatim in the module's state_dict, map it identically (e.g. `mid_block.conv.weight` → `mid_block.conv.weight`).
   - **Pattern-based remapping for encoder blocks:** `encoder.blocks.N.conv.weight` → `encoder.block_N.conv.weight` (and corresponding `.norm.weight`). The pattern strips the 's' from `blocks` and replaces `.` with `_` between `block` and the index.
   - **Pattern-based remapping for decoder blocks:** same logic as encoder — `decoder.blocks.N.conv.weight` → `decoder.block_N.conv.weight`.
   - **Skip non-weight keys:** the `latents` key from the checkpoint is not a parameter in the module's state_dict — it will be silently excluded because it won't match any module key (direct or remapped).

   This function is built by inspecting both key sets directly against the fixture, never assumed from `zit.py`'s diffusion key mapping (per §11.4's independence requirement).

3. **Complete the `load()` function.** After the existing code at line 567 (`model.to(target_dtype)`), add:
   a. Log the materialization step (DEBUG level, per §11.5 log conventions).
   b. Call `model = model.to_empty(device=device)` to materialize parameters from meta to the target device.
   c. Verify `.arch` persists after materialization — if `hasattr(model, "arch")` is False or `model.arch != ARCH`, explicitly re-set it. This mirrors the safety net in `zit.py` (line 541-542).
   d. Load checkpoint tensors: `state_dict = load_file(path, device=device)`.
   e. Build the remapping: `remap = _build_key_remapping(list(state_dict.keys()), list(model.state_dict().keys()))`.
   f. Cast each tensor to `target_dtype` BEFORE calling `load_state_dict` — `assign=True` bypasses dtype coercion, so tensors must already be correct. Also filter by shape match (skip tensors with shape mismatches, logging them at DEBUG level).
   g. Call `info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)`.
   h. Log the load result at INFO level: number of loaded tensors, missing keys, unexpected keys, device.
   i. Return `model`.

4. **Add dual-mode parity markers.** Place these two comment lines immediately above the `def load()` signature:
   ```python
   # REAL_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_load_real_zit_vae_fixture
   # MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_load_mock_returns_sentinel
   ```
   The real-mode test loads the actual fixture and verifies weights. The mock-mode test uses a sentinel approach (mock `load_file` returns a dict of tensors with known sentinel values) to verify the remapping and `load_state_dict` path without requiring a real checkpoint.

5. **Write the key remapping unit test.** Test `_build_key_remapping()` directly with controlled input: pass lists of checkpoint keys and module keys that exercise both direct match and pattern-based remapping paths. Assert the returned dict contains the expected mappings and excludes non-weight keys like `latents`.

6. **Write the new real-mode tests** (steps 7-10 in the test table below).

7. **Update the existing partial-load tests.** The current `test_load_meta_construction_succeeds` and `test_load_dtype_selection_applied` assert `.arch` on the meta-constructed module. After this task's changes, `load()` will return a fully-loaded module. These tests should continue to pass (they already check `.arch`, dtype, and meta-device — after materialization, the device check changes from `"meta"` to the target device). Update the meta-device assertions to check the target device instead.

8. **Pre-test syntax check.** Run `worker/.venv/bin/python -m py_compile worker/nodes/arch/vae/zit_vae.py` to confirm no syntax errors before running the full test suite.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `def _build_key_remapping(checkpoint_keys: list[str], module_keys: list[str]) -> dict[str, str]` | `worker.nodes.arch.vae.zit_vae` | Private helper; builds checkpoint→module key mapping for VAE. Not pub but tested directly. |
| `def load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel` | `worker.nodes.arch.vae.zit_vae` | Modified — now completes steps 3-4 of the loading contract. Signature unchanged. |

No new public items are introduced. The `_build_key_remapping()` function is private (underscore-prefixed) and is tested directly in unit tests.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/vae/zit_vae.py` | Complete `load()` with materialization, key remapping, load_state_dict; add `.arch` verification; add `_build_key_remapping()`; add dual-mode parity markers; add logging and inline comments. |
| MODIFY | `worker/tests/test_arch_vae_zit.py` | Add ≥6 new tests for weight loading, dtype casting, `.arch` attribute, mock-mode, real-mode; update existing partial-load tests to reflect full load(). |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `test_arch_vae_zit.py` | `test_build_key_remapping_direct_match` | `_build_key_remapping()` returns identity mapping for keys that exist in both checkpoint and module (e.g. `mid_block.conv.weight`) | None | Checkpoint keys: `["mid_block.conv.weight", "latents"]`; Module keys: `["mid_block.conv.weight", ...]` | `{"mid_block.conv.weight": "mid_block.conv.weight"}` — `latents` excluded | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_build_key_remapping_direct_match -v` |
| `test_arch_vae_zit.py` | `test_build_key_remapping_pattern_match` | `_build_key_remapping()` remaps `encoder.blocks.N.*` → `encoder.block_N.*` (and decoder equivalent) | None | Checkpoint keys with `encoder.blocks.0.conv.weight`; Module keys with `encoder.block_0.conv.weight` | `{"encoder.blocks.0.conv.weight": "encoder.block_0.conv.weight"}` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_build_key_remapping_pattern_match -v` |
| `test_arch_vae_zit.py` | `test_load_weights_loaded_regular_fixture` | Full `load()` against `zit_vae_tiny.safetensors`: weights actually loaded, shapes match module expectations, at least one tensor value is non-zero (proves data flowed through) | torch installed | Regular fixture path, caps with bf16=True | Model with parameters on device (not meta), non-zero tensor values, `.arch == "zit_vae"` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_weights_loaded_regular_fixture -v -m real_mode` |
| `test_arch_vae_zit.py` | `test_load_weights_loaded_no_metadata_fixture` | Full `load()` against `zit_vae_tiny_no_metadata.safetensors`: remapping handles xyz_ prefixed keys (via direct match since no pattern remapping needed for xyz_ keys — they don't match VAE patterns and are silently skipped) | torch installed | No-metadata fixture path | Model loads, parameters on device, `.arch == "zit_vae"` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_weights_loaded_no_metadata_fixture -v -m real_mode` |
| `test_arch_vae_zit.py` | `test_load_arch_attribute_set` | `.arch` attribute is `"zit_vae"` after `load()` returns (verifies step 4 of the loading contract) | torch installed | Any fixture path | `model.arch == "zit_vae"` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_arch_attribute_set -v -m real_mode` |
| `test_arch_vae_zit.py` | `test_load_dtype_applied_to_loaded_tensors` | Tensors are cast to the selected dtype (e.g. fp32 when all caps are False) BEFORE `load_state_dict(assign=True)` — verified by checking tensor dtype after load | torch installed | Regular fixture, caps with all precisions False → fp32 fallback | `param.dtype == torch.float32` for all params | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_applied_to_loaded_tensors -v -m real_mode` |
| `test_arch_vae_zit.py` | `test_load_mock_returns_sentinel` (mock) | Mock-mode: `load_file` patched to return sentinel tensors; verifies remapping logic and `load_state_dict` path execute without requiring a real checkpoint | None | Patched `load_file` returning `{}` or minimal sentinel dict | No exception raised, remapping logic exercised | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_mock_returns_sentinel -v` |
| `test_arch_vae_zit.py` | `test_load_real_zit_vae_fixture` (real) | End-to-end: full load pipeline against `zit_vae_tiny.safetensors`, verifying all steps execute correctly and model is usable | torch installed | Regular fixture path, bf16 caps | Model with loaded weights on cpu device | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_load_real_zit_vae_fixture -v -m real_mode` |

## CI Impact

No CI job changes required. The new tests use existing markers (`real_mode` for torch-requiring tests, no marker for mock-compatible tests). The `worker-linux-mock` and `worker-windows-mock` CI jobs will collect and run the mock-mode test. The `worker-linux-real` and `worker-windows-real` CI jobs will run the real-mode tests. No new file types or test modules are introduced — everything goes into the existing `test_arch_vae_zit.py`.

## Platform Considerations

None identified. The `load()` function uses `torch.device(device)` and `model.to_empty(device=device)` which are platform-neutral. The key remapping is pure string manipulation. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The VAE fixture key patterns (`encoder.blocks.N.conv.weight`) don't match the module's state_dict keys (`encoder.block_N.conv.weight`) in the expected way — the pattern-based remapping regex may not capture all keys, resulting in few or no tensors being loaded. | Medium | High | Inspect the actual `state_dict().keys()` output from the constructed `ZiTVaeModel` against the fixture keys before writing the remapping. Write a unit test that exercises the remapping with exact fixture key strings. |
| `load_state_dict(assign=True, strict=False)` may raise if the remapped state dict contains tensors with shapes that don't match the module's parameters exactly. | Medium | High | Filter tensors by shape match before building the remapped state dict (same pattern as `zit.py` line 574). Log skipped tensors at DEBUG level. |
| The no-metadata fixture uses `xyz_` prefixed keys that don't match any VAE pattern — they will be silently skipped by the remapping, resulting in an empty loaded state dict. This is correct behavior (no matching weights to load), but the test must not assert that weights were loaded. | Low | Medium | The no-metadata test asserts model structure and `.arch` but does NOT assert non-zero weights (since no keys match). Document this explicitly in the test docstring. |
| `to_empty()` may not preserve the `.arch` attribute on some PyTorch versions. | Low | Medium | Add the safety-net check from `zit.py` (line 541-542): if `model.arch != ARCH` after materialization, re-set it. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/vae/zit_vae.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_vae_zit.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 with ≥20 tests collected
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v -m "not real_mode"` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v -m real_mode` exits 0
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/arch/vae/zit_vae.py` returns one match pointing to `test_load_real_zit_vae_fixture`
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/arch/vae/zit_vae.py` returns one match pointing to `test_load_mock_returns_sentinel`
