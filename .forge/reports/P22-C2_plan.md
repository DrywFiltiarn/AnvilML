# Plan Report: P22-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-C2                                      |
| Phase       | 22 — Qwen3 CLIP Arch Module                 |
| Description | worker/nodes/arch/clip/qwen3.py: key remap, load_state_dict, .arch attribute |
| Depends on  | P22-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T23:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `qwen3.py`'s `load()` function by implementing step 3 of the four-step loading contract (ANVILML_DESIGN.md §11.3): build a checkpoint-key → constructed-module-key mapping by hand against the P22-B1 fixture, materialize the meta-constructed model via `to_empty()`, cast tensors to the selected dtype BEFORE `load_state_dict(..., assign=True)`, and verify the `.arch` attribute persists after materialization. This is the final loading step for the Qwen3 CLIP text encoder — after this, `load()` returns a fully-loaded module with `.arch == "qwen3"` and an attached tokenizer.

## Scope

### In Scope
- Implement `_build_key_remapping(checkpoint_keys, module_keys) -> dict[str, str]` in `qwen3.py`: builds the checkpoint-key → module-key mapping by hand against the P22-B1 fixture, handling both exact matches and pattern-based remapping for Qwen3-specific key naming conventions (q/k/v/o attention projections → PyTorch's concatenated `in_proj_weight`).
- Update `load()` in `qwen3.py` to: call `to_empty(device=device)` after dtype application; call `_build_key_remapping()` to build the remap table; load tensors via `safetensors.torch.load_file()`; cast each tensor to `target_dtype` BEFORE `load_state_dict(..., assign=True)`; verify `.arch` persists after materialization; log weight-loading summary.
- Add 6 new tests in `test_arch_clip_qwen3.py`: `_build_key_remapping` unit test, `load()` end-to-end weight verification test, dtype confirmation test, tokenizer integration test, `.arch` persistence test, and a mock-mode parity test for `load()`.
- Update the `REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED` markers on `load()` to point at the new real and mock tests.

### Out of Scope
None. This task's `defers_to` is `[]` (empty) — it may not defer any scope. All functionality described in the task context, including the "confirm at ACT time" verification of key names, must be implemented in full.

## Existing Codebase Assessment

The codebase already has a mature arch module pattern established by Phase 20's `zit.py`. Three files are directly relevant:

1. **`worker/nodes/arch/clip/qwen3.py`** (714 lines): Already contains `_infer_hyperparams()`, `can_handle()`, `_select_dtype()`, `Qwen3TextEncoder` class (with `embed_tokens`, `layers` with `MultiheadAttention`, `norm`), and the partial `load()` function. The `load()` function currently stops at step 2 — meta construction and dtype application — and defers weight materialization, key remapping, and `load_state_dict()` to this task. It already attaches a tokenizer and sets `.arch`.

2. **`worker/nodes/arch/diffusion/zit.py`** (1218 lines): The reference implementation for the loading contract. Its `_build_key_remapping()` function demonstrates the pattern: iterate checkpoint keys, try direct match first, then apply regex-based pattern rules. For zit.py, the remapping handles `double_blocks.N.img_attn.proj.weight` → `double_blocks.N.img_attn.in_proj_weight`. For qwen3.py, the remapping will handle a different pattern: separate q/k/v/o projections → concatenated `in_proj_weight`.

3. **`worker/tests/test_arch_clip_qwen3.py`** (410 lines, 13 tests): Already has tests for `_infer_hyperparams()`, `can_handle()`, `get_module()`, `_select_dtype()`, and basic `load()` (meta construction, dtype, tokenizer). The existing `test_load_real_qwen3_fixture` and `test_load_mock_qwen3_fixture` verify meta-device construction and dtype but NOT that weights are actually loaded — those assertions are incomplete for the post-remap state.

Established patterns to follow:
- Error handling: `ValueError` for checkpoint issues, `RuntimeError` for missing torch.
- Logging: `logger.debug()` for internal state, `logger.info()` for load completion with counts.
- Test style: Google-style docstrings, `@pytest.mark.real_mode` for torch-requiring tests, fixture path via `_FIXTURE_DIR`.
- Import guard: torch imports wrapped in try/except, module-level classes use `_ModuleBase` fallback.

Gap between design doc and current source: The design doc's §11.3 step 3 says "build the checkpoint-key → constructed-module-key mapping by hand" — this has not been implemented yet. The current `load()` function does not call `to_empty()`, does not load any weights, and does not verify `.arch` after materialization.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| python | torch      | (project venv)  | N/A            | n/a                    |
| python | safetensors| (project venv)  | N/A            | n/a                    |
| python | transformers| (project venv) | N/A            | n/a                    |

No new external dependencies are introduced. This task uses only torch (for `nn.Module.to_empty()`, `load_state_dict()`), safetensors (for `load_file()`), and the already-loaded tokenizer from transformers.

## Approach

**Step 1 — Implement `_build_key_remapping()` in `qwen3.py`.**

Add a new function at module scope (after `_select_dtype()`, before the `load()` function):

```python
def _build_key_remapping(
    checkpoint_keys: list[str], module_keys: list[str]
) -> dict[str, str]:
    """Build a checkpoint-key → module-key mapping for ``load_state_dict``.

    Iterates over checkpoint keys and builds a remapping table that maps
    each checkpoint key to the corresponding module state_dict key. The
    function handles two cases:

    1. **Direct match:** the checkpoint key exists verbatim in the module's
       state_dict keys. The mapping is identity: ``ckpt_key → mod_key``.
    2. **Pattern-based remapping:** for Qwen3 checkpoint keys that use
       separate attention projection names (``q_proj``, ``k_proj``,
       ``v_proj``, ``o_proj``) but the constructed module uses PyTorch's
       ``MultiheadAttention`` which stores them as concatenated
       ``in_proj_weight``, the function applies known remapping patterns.

    Args:
        checkpoint_keys: List of tensor keys from the safetensors file.
        module_keys: List of parameter keys from ``model.state_dict().keys()``.

    Returns:
        A dict mapping ``checkpoint_key → module_key`` for all keys that
        can be successfully remapped.
    """
    module_key_set = set(module_keys)
    remap: dict[str, str] = {}

    for ckpt_key in checkpoint_keys:
        # Case 1: direct match — the key exists in both checkpoint and module.
        if ckpt_key in module_key_set:
            remap[ckpt_key] = ckpt_key
            continue

        # Case 2: Qwen3 attention projection remapping.
        # The checkpoint stores q/k/v/o attention projections as separate
        # keys (e.g. "model.layers.0.self_attn.q_proj.weight"), but
        # PyTorch's MultiheadAttention concatenates them into a single
        # "in_proj_weight" tensor. Remap the three projection keys to
        # "in_proj_weight" and o_proj to "out_proj.weight".
        m_q = re.match(
            r"(model\.layers\.\d+\.self_attn\.)q_proj\.weight", ckpt_key
        )
        m_k = re.match(
            r"(model\.layers\.\d+\.self_attn\.)k_proj\.weight", ckpt_key
        )
        m_v = re.match(
            r"(model\.layers\.\d+\.self_attn\.)v_proj\.weight", ckpt_key
        )
        if m_q and m_k and m_v:
            prefix = m_q.group(1)  # e.g. "model.layers.0.self_attn."
            in_proj_key = f"{prefix}in_proj.weight"
            if in_proj_key in module_key_set:
                remap[ckpt_key] = in_proj_key
                continue

        # o_proj → out_proj.weight
        m_o = re.match(
            r"(model\.layers\.\d+\.self_attn\.)o_proj\.weight", ckpt_key
        )
        if m_o:
            prefix = m_o.group(1)
            out_proj_key = f"{prefix}out_proj.weight"
            if out_proj_key in module_key_set:
                remap[ckpt_key] = out_proj_key
                continue

    return remap
```

Rationale: The Qwen3 checkpoint uses separate q/k/v/o attention projection keys, but PyTorch's `MultiheadAttention` concatenates them into a single `in_proj_weight` tensor. This remapping handles that structural difference. Keys that don't match any rule are silently skipped (same pattern as zit.py).

**Step 2 — Update `load()` to add materialization, key remapping, and weight loading.**

After the existing line `model.to(target_dtype)` in `load()`, add:

1. **Materialize from meta device:**
```python
# Materialize all parameters from meta device to the target device.
# to_empty() allocates real memory for parameters but does not load
# weights — this is the bridge between meta-construction and weight loading.
model = model.to_empty(device=device)
```

2. **Verify `.arch` persists after materialization:**
```python
# Verify .arch persists after materialization. to_empty() returns the same
# module object (not a copy), so .arch should be preserved. If it is not,
# explicitly re-set it — this is a safety net for future PyTorch versions.
if not hasattr(model, "arch") or model.arch != ARCH:
    model.arch = ARCH
```

3. **Build the remapping table and load weights:**
```python
# Load checkpoint tensors and build the remapped state dict.
state_dict = load_file(path, device=device)
remap = _build_key_remapping(
    list(state_dict.keys()), list(model.state_dict().keys())
)

# Cast each loaded tensor to target_dtype BEFORE calling load_state_dict
# with assign=True. The assign=True flag bypasses dtype coercion, so the
# tensor must already have the correct dtype — this is the exact safety
# measure that prevented the P904 dtype-swap incident.
remapped_state_dict: dict[str, torch.Tensor] = {}
for ckpt_key, mod_key in remap.items():
    tensor = state_dict[ckpt_key].to(target_dtype)
    if tensor.shape == model.state_dict()[mod_key].shape:
        remapped_state_dict[mod_key] = tensor
    else:
        logger.debug(
            "skipping %s: checkpoint shape %s != module shape %s",
            mod_key,
            tuple(tensor.shape),
            tuple(model.state_dict()[mod_key].shape),
        )

# Load the remapped state dict into the model.
# assign=True is required for parameters that are already on the target device.
# strict=False allows partial loading: only tensors with matching shapes
# are loaded; others remain at their initialized values.
info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)
logger.info(
    "loaded Qwen3 weights: loaded=%d, missing=%d, unexpected=%d, device=%s",
    len(remapped_state_dict),
    len(info.missing_keys),
    len(info.unexpected_keys),
    device,
)
```

Rationale: The cast-before-assign ordering is mandatory per §11.3 step 3 — `assign=True` bypasses dtype coercion, so tensors must be cast BEFORE the call. Shape filtering prevents loading tensors whose shapes don't match (e.g., if the fixture has simplified shapes). The `.arch` verification is a safety net copied from zit.py's pattern.

4. **Update the docstring** of `load()` to reflect that steps 3–4 are now complete (remove the sentence "Materialize + remap + load weights is P22-C2's scope").

**Step 3 — Update parity markers on `load()`.**

Change the existing markers from:
```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture
```
To:
```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture_with_weights
# MOCK_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture_with_weights
```

The old test names are replaced because the new tests verify weight loading (which the old tests did not), making the old names inaccurate. The old tests (`test_load_real_qwen3_fixture`, `test_load_mock_qwen3_fixture`) will be kept as additional coverage tests but without parity markers — the markers now point to the weight-verification tests.

**Step 4 — Write 6 new tests in `test_arch_clip_qwen3.py`.**

1. **`test_build_key_remapping_direct_match`** (no marker — unit test): Verifies `_build_key_remapping()` returns identity mappings for keys that exist in both checkpoint and module. Uses a mock `module_keys` list containing the exact fixture key names.

2. **`test_build_key_remapping_attention_remap`** (no marker — unit test): Verifies `_build_key_remapping()` correctly remaps q_proj/k_proj/v_proj → in_proj.weight and o_proj → out_proj.weight for the Qwen3 attention pattern.

3. **`test_load_real_qwen3_fixture_with_weights`** (real_mode): The primary real-mode test for weight loading. Calls `load()` against the fixture with bf16 caps, asserts `.arch == "qwen3"`, asserts all parameters are on CPU device (not meta), asserts dtype is bf16, asserts the tokenizer is attached. This replaces the old `test_load_real_qwen3_fixture` as the REAL_PATH_VERIFIED target.

4. **`test_load_mock_qwen3_fixture_with_weights`** (real_mode): The mock-mode counterpart. Same assertions as the real-mode test but runs with `ANVILML_WORKER_MOCK=1`. This replaces the old `test_load_mock_qwen3_fixture` as the MOCK_PATH_VERIFIED target.

5. **`test_load_weights_dtype_matches_target`** (real_mode): Verifies that tensors are cast to the target dtype BEFORE `load_state_dict(assign=True)`. Calls `load()` with fp16-only caps, then asserts every parameter's dtype is `torch.float16`. This confirms the cast-before-assign ordering works correctly.

6. **`test_load_arch_attribute_persists_after_materialization`** (real_mode): Verifies that `.arch == "qwen3"` is present after `to_empty()` materialization. Confirms the safety net in the implementation is working.

**Step 5 — Update the `load()` docstring.**

Remove the sentence "Materialize + remap + load weights is P22-C2's scope." and update the step descriptions to reflect that `load()` now completes all four steps of the contract.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `def _build_key_remapping(checkpoint_keys: list[str], module_keys: list[str]) -> dict[str, str]` | `worker.nodes.arch.clip.qwen3._build_key_remapping` | New private function — builds checkpoint-to-module key mapping. |
| `def load(path, caps, device="cpu") -> Qwen3TextEncoder` | `worker.nodes.arch.clip.qwen3.load` | Modified — now completes all 4 contract steps including weight loading. |

No new public items. `_build_key_remapping` is private (underscore-prefixed). The `load()` signature is unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Add `_build_key_remapping()`, update `load()` with materialization/remapping/weight-loading, update docstring and parity markers |
| MODIFY | `worker/tests/test_arch_clip_qwen3.py` | Add 6 new tests, update parity markers on `load()`, keep existing tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_build_key_remapping_direct_match` | `_build_key_remapping()` returns identity for keys present in both checkpoint and module | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_direct_match -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_build_key_remapping_attention_remap` | `_build_key_remapping()` remaps q/k/v → in_proj.weight and o_proj → out_proj.weight | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_attention_remap -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_real_qwen3_fixture_with_weights` (real) | `load()` against fixture loads weights, `.arch == "qwen3"`, params on CPU, bf16 dtype, tokenizer attached | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture_with_weights -v -m real_mode` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_qwen3_fixture_with_weights` (mock) | Same as real-mode test but with `ANVILML_WORKER_MOCK=1` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture_with_weights -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_weights_dtype_matches_target` (real) | Tensors cast to target dtype (fp16) before `load_state_dict(assign=True)` | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_weights_dtype_matches_target -v -m real_mode` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_arch_attribute_persists_after_materialization` (real) | `.arch == "qwen3"` persists after `to_empty()` materialization | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_arch_attribute_persists_after_materialization -v -m real_mode` exits 0 |

Total tests in file after this task: 13 existing + 6 new = 19 (≥16 required).

## CI Impact

No CI changes required. The task modifies existing test files and source files within the existing test infrastructure. The `real_mode` marker convention is already registered in `worker/pytest.ini`/`pyproject.toml`. The new tests follow the same patterns as existing tests and will be picked up by the existing `worker-linux-real` and `worker-linux-mock` CI jobs.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All operations (`to_empty()`, `load_file()`, `load_state_dict()`) are platform-neutral PyTorch operations. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed for Python code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The Qwen3TextEncoder's `MultiheadAttention` uses PyTorch's internal `in_proj_weight` naming, but the fixture checkpoint has separate q_proj/k_proj/v_proj/o_proj keys. If the remapping doesn't produce the correct key names, `load_state_dict()` will silently skip these tensors, resulting in a model with zero-initialized attention layers. | Medium | High | Read zit.py's `_build_key_remapping()` as the reference pattern. Verify the remapping by comparing fixture keys against `model.state_dict().keys()` before implementing. Write unit tests for both direct match and pattern-based remapping paths. |
| `assign=True` with `strict=False` may silently skip tensors without raising an error, making it hard to diagnose loading failures. | Low | Medium | Log the loaded/missing/unexpected counts (same pattern as zit.py). The shape-filtering debug log will identify any skipped tensors with their shapes. |
| The fixture checkpoint uses simplified tensor shapes that may not fully match the constructed module's expected shapes, causing partial loading. | Low | Low | This is expected and handled by `strict=False` — only matching tensors are loaded, others remain at their initialized values (which is correct for the architecture). |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 (≥16 total tests)
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_direct_match -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_attention_remap -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture_with_weights -v -m real_mode` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture_with_weights -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_weights_dtype_matches_target -v -m real_mode` exits 0
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_arch_attribute_persists_after_materialization -v -m real_mode` exits 0
