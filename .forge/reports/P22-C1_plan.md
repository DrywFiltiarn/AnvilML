# Plan Report: P22-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-C1                                      |
| Phase       | 22 — Qwen3 CLIP Arch Module                 |
| Description | worker/nodes/arch/clip/qwen3.py: meta construction + dtype selection |
| Depends on  | P22-B3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T16:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `load()` function to `worker/nodes/arch/clip/qwen3.py` implementing steps 2–3 of the four-step loading contract (ANVILML_DESIGN.md §11.3), plus tokenizer loading from the vendored local asset directory. The `load()` function constructs a Qwen3 text-encoder `nn.Module` on `torch.device("meta")` using only hyperparameters inferred by `_infer_hyperparams()` (P22-B2), selects the compute dtype per the fixed precedence in §11.5 (caps.fp8+native → bf16 → fp16 → fp32), and loads the Qwen3 tokenizer from `worker/assets/qwen3_tokenizer/` via `transformers`' `AutoTokenizer` — never performing a hub lookup. Step 4 (materialize+remap+load weights) is the next task's scope. Acceptance: `python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 with >=10 tests total.

## Scope

### In Scope
- Implement `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype` — the dtype selection function following §11.5's fixed precedence chain (identical discipline to zit.py's `_select_dtype`).
- Implement `Qwen3TextEncoder(nn.Module)` class — the target model constructed on meta-device. Uses transformers' layer/block classes (Linear, LayerNorm, etc.) as building blocks per §11.2, never `AutoModel.from_pretrained`.
- Implement `load(path: str, caps: dict, device: str = "cpu") -> Qwen3TextEncoder` — constructs the model on meta-device with selected dtype, loads tokenizer from vendored path. Does NOT materialize or load weights (steps 3–4 are P22-C2's scope).
- Load the Qwen3 tokenizer from `worker/assets/qwen3_tokenizer/` using `transformers.AutoTokenizer.from_pretrained()` with `local_files_only=True` — zero network calls guaranteed.
- Add `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` dual-mode parity markers to `load()` per ANVILML_DESIGN.md §10.6.
- Add >=4 new tests in `worker/tests/test_arch_clip_qwen3.py`: meta construction, 4 caps-combo dtype selection, tokenizer loads from local path with zero network calls.

### Out of Scope
- Materializing the model from meta to real device via `to_empty()` (P22-C2).
- Building checkpoint-key → module-key remapping table (P22-C2).
- Loading weights via `load_state_dict(assign=True)` (P22-C2).
- Key remapping logic (P22-C2).
- `sample()` or `decode()` — CLIP modules implement `load()` only, per §10.4.
- `LoadClip` loader node integration (P22-D1).

## Existing Codebase Assessment

**What already exists:** `worker/nodes/arch/clip/qwen3.py` has `_infer_hyperparams()` (P22-B2), `can_handle()` (P22-B3), and `ARCH = "qwen3"`. The clip dispatcher in `__init__.py` already registers qwen3. `worker/tests/test_arch_clip_qwen3.py` has 6 tests covering `_infer_hyperparams`, `can_handle`, and dispatch registration. `worker/tests/fixtures/qwen3_tiny.safetensors` exists with structurally valid Qwen3-shaped tensors (hidden_dim=64, 2 layers, intermediate_size=128, vocab_size=128, arch="qwen3" metadata). The tokenizer is vendored at `worker/assets/qwen3_tokenizer/` (P22-A1). `worker/nodes/base.py` defines `NodeContext` with the `caps` dict attribute that arch modules read for dtype decisions.

**Established patterns:** zit.py's `load()` pattern is the reference: (1) torch import guard, (2) `_infer_hyperparams()` call, (3) `_select_dtype()` call, (4) meta-device construction, (5) dtype application via `model.to(dtype)`. The dual-mode parity markers use the format `# REAL_PATH_VERIFIED: worker/tests/test_arch_<family>.py::test_<name>` and `# MOCK_PATH_VERIFIED: ...`. Tests follow the pattern: import torch under try/except guard, use `_FIXTURE_DIR`, create caps dicts with explicit bool values.

**Gap between design doc and source:** No `Qwen3TextEncoder` class exists yet — the model class must be created. No `_select_dtype` function exists in qwen3.py (zit.py has one). The `load()` function is completely absent. The test file has 6 tests; P22-C1 must add at least 4 more to reach >=10 total.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source      | Feature flags confirmed |
|--------|-------------|-----------------|-----------------|------------------------|
| python | transformers | 5.13.1          | pypi-query MCP  | n/a (core dep, no features) |

**API shape confirmed:** `transformers.AutoTokenizer.from_pretrained(path, local_files_only=True)` exists in transformers 5.13.1 for loading tokenizers from local paths without hub access. The `local_files_only=True` parameter explicitly prevents any network call to the Hugging Face Hub. This matches the design doc §11.2 requirement: "Tokenizer classes from transformers, loaded from the vendored local asset directory — never a hub lookup."

## Approach

### Step 1: Add torch import guard

Add the same `try/except ImportError` guard that zit.py uses at the top of qwen3.py (after the existing `safetensors` import). This ensures the module remains importable in mock-mode test collection (worker-linux-mock CI job installs `requirements/base.txt` only, no torch).

```python
try:
    import torch
    import torch.nn as nn
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
```

**Rationale:** Every arch module's dispatcher (`arch/clip/__init__.py`) imports every registered module eagerly at package-import time. An unconditional `import torch` would break collection for the entire CLIP family's test suite in mock-mode CI jobs (ANVILML_DESIGN.md §11.2).

### Step 2: Add `_select_dtype(caps, native_dtype) -> torch.dtype`

Implement the dtype selection function following the fixed precedence in §11.5, identical to zit.py's `_select_dtype`:

1. If `caps.fp8` is `True` AND `native_dtype == "fp8"` → return `torch.float8_e4m3fn`
2. Else if `caps.bf16` is `True` → return `torch.bfloat16`
3. Else if `caps.fp16` is `True` → return `torch.float16`
4. Else → return `torch.float32`

This is a pure function — no I/O, no side effects. It reads only from the `caps` dict and `native_dtype` string.

### Step 3: Create `Qwen3TextEncoder(nn.Module)` class

Create a new `nn.Module` subclass for the Qwen3 text encoder. The class uses a conditional base class (`_ModuleBase = nn.Module if nn is not None else object`) so it defines successfully when torch is absent.

The `__init__` takes `hyperparams: dict[str, Any]` (from `_infer_hyperparams`) and constructs:

1. `self.embed_tokens` — `nn.Embedding(vocab_size, hidden_dim)` — the vocabulary embedding table
2. `self.layers` — `nn.ModuleList` of `Qwen3DecoderLayer` instances, one per `num_hidden_layers`
3. `self.norm` — `nn.LayerNorm(hidden_dim)` — final normalization

Each `Qwen3DecoderLayer` (inner class or separate class) contains:
1. `self.input_layernorm` — `nn.LayerNorm(hidden_dim)`
2. `self.self_attn` — using `nn.MultiheadAttention(embed_dim=hidden_dim, num_heads=hidden_dim // 64, batch_first=True)` — mirrors the pattern zit.py uses for attention
3. `self.post_attention_layernorm` — `nn.LayerNorm(hidden_dim)`
4. `self.mlp.gate_proj` — `nn.Linear(hidden_dim, intermediate_size)`
5. `self.mlp.up_proj` — `nn.Linear(hidden_dim, intermediate_size)`
6. `self.mlp.down_proj` — `nn.Linear(intermediate_size, hidden_dim)`

After construction, set `self.arch: str = "qwen3"`.

**Rationale for using nn.MultiheadAttention:** Per §11.2, we use transformers'/torch's own layer classes as building blocks — we reimplement the loading mechanism (shape inference + key remap), not the attention math. `nn.MultiheadAttention` is the standard PyTorch attention module that matches the checkpoint's key patterns (q_proj, k_proj, v_proj, o_proj map to the in_proj_weight/out_proj_weight internal naming).

### Step 4: Implement `load(path, caps, device) -> Qwen3TextEncoder`

The `load()` function implements steps 2–3 of the four-step contract:

1. **Guard:** Check `if torch is None: raise RuntimeError(...)` — clear error if reached from mock-mode.
2. **Infer hyperparameters:** `hyperparams = _infer_hyperparams(path)` — delegates to P22-B2's function.
3. **Select dtype:** `target_dtype = _select_dtype(caps, hyperparams["native_dtype"])` — step 2 of §11.3.
4. **Meta-device construction:** 
   ```python
   with torch.device("meta"):
       model = Qwen3TextEncoder(hyperparams)
   ```
5. **Apply dtype:** `model.to(target_dtype)` — changes dtype metadata on meta-parameters without allocating memory.
6. **Load tokenizer:** Load from the vendored local path using `transformers.AutoTokenizer.from_pretrained()` with `local_files_only=True`:
   ```python
   from transformers import AutoTokenizer
   tokenizer_path = str(Path(__file__).parent.parent.parent / "assets" / "qwen3_tokenizer")
   tokenizer = AutoTokenizer.from_pretrained(tokenizer_path, local_files_only=True)
   ```
7. **Attach tokenizer:** Set `model.tokenizer = tokenizer` on the returned object.
8. **Return:** Return the meta-constructed model with `.arch = "qwen3"` and attached tokenizer.

**Do NOT** include `to_empty()` materialization or `load_state_dict` weight loading — those are P22-C2's scope.

**Dual-mode parity markers:** Add both markers next to the `load()` function definition:
```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture
def load(path: str, caps: dict, device: str = "cpu") -> Qwen3TextEncoder:
```

### Step 5: Add logging

Per §11.7 of FORGE_AGENT_RULES, add a DEBUG log call after dtype selection:
```python
logger.debug("selected dtype=%s for device=%s", target_dtype, device)
```

And an INFO log after tokenizer load:
```python
logger.info("loaded tokenizer from=%s", tokenizer_path)
```

### Step 6: Write tests

Write the following new tests in `worker/tests/test_arch_clip_qwen3.py` (bringing total to >=10):

1. **`test_load_meta_construction`** — Calls `load()` and verifies the model is constructed on meta-device (parameters have `.device.type == "meta"` before materialization). This is the primary test for step 2 of the contract.
2. **`test_dtype_selection_fp8_caps_and_native`** — Unit test for `_select_dtype()` with `caps.fp8=True, native_dtype="fp8"` → expects `torch.float8_e4m3fn`.
3. **`test_dtype_selection_bf16_real`** — Unit test for `_select_dtype()` with `caps.bf16=True, native_dtype="fp32"` → expects `torch.bfloat16`.
4. **`test_dtype_selection_fp16_only`** — Unit test for `_select_dtype()` with `caps.fp16=True, bf16=False, fp8=False, native_dtype="fp32"` → expects `torch.float16`.
5. **`test_dtype_selection_fp32_fallback`** — Unit test for `_select_dtype()` with all precision flags False except fp32 → expects `torch.float32`.
6. **`test_tokenizer_loads_from_vendored_path_no_network`** — Tests that `AutoTokenizer.from_pretrained()` loads from the local vendored path without network calls. Uses `unittest.mock.patch("transformers.AutoTokenizer.from_pretrained")` to verify the call was made with `local_files_only=True` and the correct path. This is the load-bearing test for the offline guarantee.

**Test markers:** The real-mode tests (2-5 for dtype, 1 for meta) need `@pytest.mark.real_mode` because they reference `torch` types. The tokenizer test can run without torch (it mocks the import). The `load()` function tests that call `load()` directly need `@pytest.mark.real_mode`.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `_select_dtype` | `worker.nodes.arch.clip.qwen3` | `def _select_dtype(caps: dict, native_dtype: str) -> torch.dtype` |
| `Qwen3TextEncoder` | `worker.nodes.arch.clip.qwen3` | `class Qwen3TextEncoder(nn.Module)` with `.arch: str` and `.tokenizer: PreTrainedTokenizer` attributes |
| `load` | `worker.nodes.arch.clip.qwen3` | `def load(path: str, caps: dict, device: str = "cpu") -> Qwen3TextEncoder` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/clip/qwen3.py` | Add torch import guard, `_select_dtype()`, `Qwen3TextEncoder` class, `load()` function, tokenizer loading, dual-mode markers |
| Modify | `worker/tests/test_arch_clip_qwen3.py` | Add >=4 new tests: meta construction, 4 dtype combos, tokenizer network-blocking test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| test_arch_clip_qwen3.py | test_load_meta_construction | load() constructs Qwen3TextEncoder on meta-device; model has .arch="qwen3"; all params on meta device | torch installed | qwen3_tiny.safetensors path, bf16 caps | Returns Qwen3TextEncoder with meta params, .arch="qwen3" | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_meta_construction -v` |
| test_arch_clip_qwen3.py | test_dtype_selection_fp8_caps_and_native | _select_dtype returns float8_e4m3fn when caps.fp8=True AND native is fp8 | torch installed | caps={fp8:True, ...}, native="fp8" | torch.float8_e4m3fn | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp8_caps_and_native -v` |
| test_arch_clip_qwen3.py | test_dtype_selection_bf16_real | _select_dtype returns bfloat16 when caps.bf16=True, native fp32 | torch installed | caps={bf16:True, ...}, native="fp32" | torch.bfloat16 | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_bf16_real -v` |
| test_arch_clip_qwen3.py | test_dtype_selection_fp16_only | _select_dtype returns float16 when only fp16 available | torch installed | caps={fp16:True, bf16:False, fp8:False}, native="fp32" | torch.float16 | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp16_only -v` |
| test_arch_clip_qwen3.py | test_dtype_selection_fp32_fallback | _select_dtype returns float32 when no precision caps available | torch installed | caps={fp32:True, fp16:False, bf16:False, fp8:False}, native="fp32" | torch.float32 | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp32_fallback -v` |
| test_arch_clip_qwen3.py | test_tokenizer_loads_from_vendored_path_no_network | AutoTokenizer.from_pretrained called with local_files_only=True against vendored path | transformers installed | Vendored tokenizer path | Patch confirms local_files_only=True and correct path arg | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_tokenizer_loads_from_vendored_path_no_network -v` |

## CI Impact

No CI changes required. The new tests follow the existing pattern: mock-compatible tests (can_handle, _infer_hyperparams) run in both mock and real CI jobs; real_mode-marked tests run only in worker-*-real CI jobs. The torch import guard ensures the module remains importable in mock-mode collection.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. Tokenizer loading uses `pathlib.Path` for path construction which handles platform-specific separators automatically. No `#[cfg(unix)]` / `#[cfg(windows)]` guards needed — the Python code runs identically on both platforms.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| transformers' `AutoTokenizer.from_pretrained()` API may differ between versions — the `local_files_only` parameter name or behavior could change | Low | Medium | Verified via MCP that transformers 5.13.1 has `local_files_only` parameter. If MCP result differs at ACT time, use the MCP-confirmed version. The parameter is part of the standard `PreTrainedTokenizerBase.from_pretrained()` signature that has been stable since transformers 4.x. |
| Qwen3TextEncoder's `nn.MultiheadAttention` internal key naming (in_proj_weight, out_proj_weight) may not match checkpoint key patterns, causing weight loading to fail in P22-C2 | Medium | High (blocks P22-C2) | This plan only constructs on meta-device (P22-C1). The key remapping table for P22-C2 will need to handle the MultiheadAttention naming difference. The plan explicitly defers weight loading to P22-C2 where the actual remapping is implemented. |
| The tokenizer vendored at `worker/assets/qwen3_tokenizer/` may use a format incompatible with `AutoTokenizer.from_pretrained()` (e.g., SentencePiece vs BPE) | Low | Medium | The vendored tokenizer was seeded by P22-A1 using the official Qwen3 tokenizer release. `AutoTokenizer.from_pretrained()` auto-detects the tokenizer type from `tokenizer_config.json` in the directory. If the format is SentencePiece, `AutoTokenizer` handles it transparently. |
| Meta-device construction of Qwen3TextEncoder may consume unexpected memory due to nn.ModuleList initialization | Low | Medium | Meta-device construction in PyTorch does not allocate real memory for parameters — only shape metadata. This is the exact mechanism that fixed P904's ~15GB crash. Verified by checking that parameters have `.device.type == "meta"` after construction. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py -v --tb=short` exits 0 with >=10 tests collected and passed
- [ ] `python -m py_compile worker/nodes/arch/clip/qwen3.py` exits 0 (syntax check before test run)
- [ ] `grep -c "REAL_PATH_VERIFIED:" worker/nodes/arch/clip/qwen3.py` outputs 1 (load() has real-mode marker)
- [ ] `grep -c "MOCK_PATH_VERIFIED:" worker/nodes/arch/clip/qwen3.py` outputs 1 (load() has mock-mode marker)
- [ ] `grep "local_files_only=True" worker/nodes/arch/clip/qwen3.py` matches at least one line (tokenizer loading uses local-only mode)
