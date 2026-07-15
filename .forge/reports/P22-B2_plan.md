# Plan Report: P22-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-B2                                      |
| Phase       | 22 — Qwen3 CLIP Arch Module                 |
| Description | worker/nodes/arch/clip/qwen3.py: shape inference from safetensors header |
| Depends on  | P22-A1, P22-B1                              |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T11:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/arch/clip/qwen3.py` containing the `_infer_hyperparams(path: str) -> dict[str, Any]` function that implements step 1 of the four-step loading contract (ANVILML_DESIGN.md §11.3) for the Qwen3 CLIP text-encoder architecture. The function opens a safetensors checkpoint header-only, reads every tensor key's shape to infer hidden dimension, layer count, intermediate size, vocab size, architecture string, and native dtype — returning them as a typed dict. This is the first concrete arch-module file for the CLIP family and establishes the shape-inference discipline (read ALL keys, never truncated) that all subsequent arch modules follow.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/qwen3.py` with the `_infer_hyperparams(path: str) -> dict[str, Any]` function.
- The function reads ALL keys via `f.keys()` (never truncated to a sample) from the safetensors header using `safe_open(path, framework="np")`.
- Infer these hyperparameters from Qwen3 tensor key patterns:
  - `hidden_dim` — from `model.layers.N.self_attn.{q,k,v,o}_proj.weight` shape[0] or shape[1]
  - `num_hidden_layers` — count of unique `model.layers.N.*` indices
  - `intermediate_size` — from `model.layers.N.mlp.{gate,up}_proj.weight` shape[0]
  - `vocab_size` — from `model.embed_tokens.weight` shape[0]
  - `arch` — from safetensors metadata `"arch"` key (fallback: detect from key patterns)
  - `native_dtype` — from the first `.weight` tensor's dtype string, mapped to canonical form
- Wrap file-open errors (FileNotFoundError, SafetensorError, truncated header) into `ValueError` with descriptive messages.
- Create `worker/tests/test_arch_clip_qwen3.py` with ≥3 tests exercising the function.
- No `can_handle()`, no dispatch registration, no `load()` — these are P22-B3's scope.

### Out of Scope
- `can_handle(key: str) -> bool` — deferred to P22-B3 (explicitly in `defers_to`).
- Dispatch registration in `arch/clip/__init__.py` — deferred to P22-B3.
- Meta-device model construction — deferred to P22-C1.
- Dtype selection — deferred to P22-C1.
- Tokenizer loading — deferred to P22-C1.
- Key remapping and `load_state_dict` — deferred to P22-C2.
- The `.arch` attribute on a constructed model — deferred to P22-C2.

## Existing Codebase Assessment

**What already exists:**
- `worker/tests/fixtures/qwen3_tiny.safetensors` (363 KB) — the Qwen3-shaped fixture built by `build_qwen3_fixture.py` (P22-B1). It contains keys following the standard Qwen3 transformer key naming convention: `model.embed_tokens.weight`, `model.layers.N.self_attn.{q,k,v,o}_proj.weight`, `model.layers.N.mlp.{gate,up,down}_proj.weight`, `model.layers.N.{input,post_attention}_layernorm.weight`, `model.norm.weight`, with metadata `{"arch": "qwen3"}`. Hidden dimension is 64, 2 hidden layers, intermediate size 128, vocab size 128.
- `worker/nodes/arch/clip/__init__.py` — the dispatcher stub with `get_module(key)` and an empty `_REGISTERED_MODULES` list (P10-B2). No modules registered yet.
- `worker/nodes/arch/diffusion/zit.py` — the reference implementation for shape inference. Its `_infer_hyperparams()` function is the exact pattern to follow: `safe_open(path, framework="np")` for header-only reads, `f.keys()` without truncation (P904 regression prevention), metadata-fallback path for architecture detection, and `.weight` suffix iteration for native dtype detection.
- `worker/tests/test_arch_zit.py` — the reference test file with `test_infer_hyperparams_regular_fixture`, `test_infer_hyperparams_no_metadata_fixture`, `test_infer_hyperparams_nonexistent_path_raises`, and `test_infer_hyperparams_truncated_header_raises`.

**Established patterns to follow:**
- torch-guard: `try/except ImportError` at module level, setting `torch = None` — required because arch modules are imported eagerly by the dispatcher which is reachable from mock-mode test collection.
- Error handling: `ValueError` for all input validation failures, with descriptive messages including the file path.
- Documentation: Google-style docstrings with Args/Returns/Raises sections for every function.
- Test style: One test per function behavior, using `pytest.raises` for error cases, asserting specific dict key values.

**Gap between design doc and current source:**
- `qwen3.py` does not yet exist. This task creates it from scratch. The fixture exists and is structurally valid for the shape-inference formula this task will implement.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| python | safetensors | 0.5.x (from base.txt) | pypi-query MCP | n/a |

No new external dependencies are introduced. `safetensors` is already in `worker/requirements/base.txt`. The `safe_open(path, framework="np")` API used in zit.py is confirmed stable — it is the standard safetensors API for header-only reads.

## Approach

1. **Create `worker/nodes/arch/clip/qwen3.py`.**
   - Add module-level docstring describing the file's role: implements step 1 of the four-step loading contract (ANVILML_DESIGN.md §11.3) for Qwen3 CLIP text-encoders.
   - Guard torch imports with `try/except ImportError` (same pattern as zit.py): set `torch = None`, `nn = None` on failure. This keeps the module importable in mock-mode collection where torch is absent.
   - Define `ARCH: str = "qwen3"` as the canonical architecture identifier (mirrors zit.py's `ARCH`).
   - Define `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` helper — same as zit.py's implementation — mapping safetensors dtype strings ("F32", "BF16", "F8_E4M3", etc.) to canonical lowercase forms ("fp32", "bf16", "fp8").
   - Define `_infer_hyperparams(path: str) -> dict[str, Any]`:
     - Open the file with `safe_open(path, framework="np")` inside a try/except that converts `FileNotFoundError`, `OSError`, and generic `Exception` (SafetensorError) into `ValueError` with descriptive messages.
     - Call `_infer_hyperparams_inner(f, path)` inside the `with` block.
   - Define `_infer_hyperparams_inner(f, path) -> dict[str, Any]` (the inner logic, factored out so the try/except cleanly wraps `safe_open` without needing to re-raise from inside the `with` block — same pattern as zit.py):
     - **Read ALL keys** via `keys = f.keys()` — no truncation, no `[:30]` (P904 regression prevention).
     - **Detect native_dtype** — iterate over all keys, find the first key ending with `.weight`, call `f.get_slice(key).get_dtype()` to get the safetensors dtype string, map via `_safetensors_dtype_to_canonical()`. Default to `"fp32"` if no weight tensor found.
     - **Infer hidden_dim** — iterate over keys looking for `model.layers.N.self_attn.q_proj.weight` (or any of q/k/v/o_proj), extract `get_shape()[0]` (the first dimension = hidden_dim). Raise ValueError if not found.
     - **Count num_hidden_layers** — regex search for `model\.layers\.(\d+)` across all keys, collect unique indices into a set, `count = max(indices) + 1`. Raise ValueError if no layer keys found.
     - **Infer intermediate_size** — look for `model.layers.N.mlp.gate_proj.weight` or `mlp.up_proj.weight`, extract `get_shape()[0]` (the first dimension = intermediate_size). Raise ValueError if not found.
     - **Infer vocab_size** — look for `model.embed_tokens.weight`, extract `get_shape()[0]`. Raise ValueError if not found.
     - **Detect arch** — check `f.metadata().get("arch")` first. If absent, infer from key patterns: presence of `model.layers.N.self_attn.*_proj` or `model.layers.N.mlp.*_proj` indicates Qwen3 family. Set `arch = "qwen3"`. Raise ValueError if neither metadata nor key patterns match.
     - Return `{"hidden_dim": int, "num_hidden_layers": int, "intermediate_size": int, "vocab_size": int, "arch": str, "native_dtype": str}`.

2. **Create `worker/tests/test_arch_clip_qwen3.py`.**
   - Follow the exact same structure as `test_arch_zit.py`:
     - Guard torch import with `try/except ImportError`.
     - Import `_infer_hyperparams` from `worker.nodes.arch.clip.qwen3`.
     - Define `_FIXTURE_DIR = Path(__file__).parent / "fixtures"`.
   - **Test 1: `test_infer_hyperparams_qwen3_fixture`** — Call `_infer_hyperparams()` against `qwen3_tiny.safetensors`. Assert the returned dict contains all expected keys (`hidden_dim`, `num_hidden_layers`, `intermediate_size`, `vocab_size`, `arch`, `native_dtype`) and correct values: `hidden_dim=64`, `num_hidden_layers=2`, `intermediate_size=128`, `vocab_size=128`, `arch="qwen3"`, `native_dtype="fp32"` (random.randn defaults to float32).
   - **Test 2: `test_infer_hyperparams_nonexistent_path_raises`** — Call `_infer_hyperparams()` with a path that does not exist. Assert `ValueError` is raised with a message containing "No such file" or similar.
   - **Test 3: `test_infer_hyperparams_truncated_header_raises`** — Write a small binary blob (not a valid safetensors header) to a temp file, call `_infer_hyperparams()`, assert `ValueError` is raised. Clean up temp file in a `finally` block.

3. **Verify syntax** — Run `python -m py_compile worker/nodes/arch/clip/qwen3.py worker/tests/test_arch_clip_qwen3.py` to confirm no syntax errors before running tests.

## Public API Surface

| Module Path | Item | Signature | Description |
|-------------|------|-----------|-------------|
| `worker.nodes.arch.clip.qwen3` | `ARCH` | `str = "qwen3"` | Canonical architecture identifier string (module-level constant). |
| `worker.nodes.arch.clip.qwen3` | `_infer_hyperparams()` | `def _infer_hyperparams(path: str) -> dict[str, Any]` | Public function: open safetensors header, infer Qwen3 hyperparameters from tensor shapes. Returns dict with keys: `hidden_dim`, `num_hidden_layers`, `intermediate_size`, `vocab_size`, `arch`, `native_dtype`. Raises `ValueError` on invalid input. |
| `worker.nodes.arch.clip.qwen3` | `_infer_hyperparams_inner()` | `def _infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]` | Inner logic — runs inside the `safe_open` context. Factored out for clean try/except wrapping. |
| `worker.nodes.arch.clip.qwen3` | `_safetensors_dtype_to_canonical()` | `def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` | Helper mapping safetensors dtype strings to canonical lowercase forms. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/qwen3.py` | New file; `_infer_hyperparams()` for Qwen3 CLIP shape inference |
| CREATE | `worker/tests/test_arch_clip_qwen3.py` | New test file; ≥3 tests for `_infer_hyperparams()` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_infer_hyperparams_qwen3_fixture` | `_infer_hyperparams()` against `qwen3_tiny.safetensors` returns correct hyperparameter dict: hidden_dim=64, num_hidden_layers=2, intermediate_size=128, vocab_size=128, arch="qwen3", native_dtype="fp32" | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_qwen3_fixture -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_infer_hyperparams_nonexistent_path_raises` | `_infer_hyperparams()` raises `ValueError` for a non-existent file path, with a message containing "No such file" | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_infer_hyperparams_truncated_header_raises` | `_infer_hyperparams()` raises `ValueError` for a truncated/corrupted safetensors file (small binary blob) | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_truncated_header_raises -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_arch_clip_qwen3.py` is picked up automatically by the existing pytest discovery in `worker/tests/`. The mock-mode CI job (`worker-linux-mock`, `worker-windows-mock`) will collect this file (it has no unconditional torch imports at module level — `_infer_hyperparams` uses only `safetensors` with `framework="np"`). The real-mode CI job (`worker-linux-real`, `worker-windows-real`) will execute all three tests.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `safetensors` library is cross-platform, and the file path handling uses Python's standard `Path` which abstracts platform differences. No `# cfg` guards or path-separator handling needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The Qwen3 fixture key patterns may differ from what I infer — e.g. the actual fixture uses `model.layers.0.self_attn.q_proj.weight` but the regex or endswith logic doesn't match. | Low | Medium | Read the fixture builder (`build_qwen3_fixture.py`) which defines the exact keys. The keys are clearly named: `model.layers.N.self_attn.q_proj.weight`, etc. Use `endswith` checks like zit.py's pattern, not regex, for the primary inference path. |
| `safe_open(path, framework="np")` may not be available in the version of safetensors installed in the CI mock job (which only installs `requirements/base.txt`). | Very Low | High | `framework="np"` is the standard safetensors API for header-only reads — it has been stable since safetensors 0.1.x. If the MCP lookup shows a newer version, verify the API still exists. This is the exact same call used by zit.py which already ships in CI. |
| The fixture's `native_dtype` is `"fp32"` (torch.randn default), but the test may need to verify that a non-fp32 fixture would return the correct canonical dtype string. | Low | Low | The fixture builder uses `torch.randn()` which defaults to float32. The `native_dtype` assertion of `"fp32"` is correct for this fixture. A separate test for non-fp32 dtypes is not required by the acceptance criteria (≥3 tests). |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 with ≥3 tests collected
- [ ] `python -m py_compile worker/nodes/arch/clip/qwen3.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_clip_qwen3.py` exits 0
