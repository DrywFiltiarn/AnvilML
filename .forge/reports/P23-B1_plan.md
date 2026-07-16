# Plan Report: P23-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-B1                                        |
| Phase       | 023 — ZiT VAE Arch Module                     |
| Description | worker/nodes/arch/vae/zit_vae.py: shape inference from safetensors header |
| Depends on  | P23-A1                                          |
| Project     | anvilml                                         |
| Planned at  | 2026-07-16T19:45:00Z                            |
| Attempt     | 1                                               |

## Objective

Create `worker/nodes/arch/vae/zit_vae.py` containing `_infer_hyperparams(path: str) -> dict[str, Any]` — the first step of the four-step loading contract (ANVILML_DESIGN.md §11.3) for the ZiT-compatible VAE architecture family. This function reads only the safetensors header of a ZiT-VAE checkpoint, infers encoder/decoder channel counts, latent channel count, architecture string, and native dtype, and returns them as a dict. It must work with torch absent (importable in mock-mode collection), follow the same all-keys discipline as zit.py and qwen3.py, and be genuinely independent of zit.py's diffusion shape-inference logic per §11.4.

## Scope

### In Scope
- Create `worker/nodes/arch/vae/zit_vae.py` with:
  - Torch import guard (try/except ImportError → None fallback), same pattern as zit.py and qwen3.py.
  - `_infer_hyperparams(path: str) -> dict[str, Any]` — public function, opens safetensors header-only with `framework="np"`, reads ALL keys via `f.keys()`, infers hyperparameters, returns dict.
  - `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]` — inner logic helper (factored out for clean try/except wrapping, same pattern as zit.py/qwen3.py).
  - `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` — maps safetensors dtype strings to canonical lowercase forms.
- Create `worker/tests/test_arch_vae_zit.py` with >=4 tests exercising `_infer_hyperparams` against both fixtures and malformed input.

### Out of Scope
- `can_handle(key) -> bool` — deferred to P23-B2 (confirmed: P23-B2's description explicitly covers "can_handle(key) -> bool" and "Register zit_vae.py into arch/vae/__init__.py's _REGISTERED_MODULES list").
- Dispatch registration in `arch/vae/__init__.py` — deferred to P23-B2.
- `load()` function — deferred to later tasks in Group C.
- `decode()` function — deferred to P23-D1.
- Any other function beyond `_infer_hyperparams`, its inner helper, and the dtype mapper.

## Existing Codebase Assessment

**What already exists:** Phase 23's fixtures (P23-A1) are already built: `zit_vae_tiny.safetensors` with `arch: "zit_vae"` metadata and recognizable VAE key prefixes (`encoder.blocks.N.conv.weight`, `decoder.blocks.N.conv.weight`, `mid_block.conv.weight`, `latents`), plus `zit_vae_tiny_no_metadata.safetensors` with `xyz_` prefixed keys and no `arch` metadata. The VAE dispatcher (`arch/vae/__init__.py`) exists with an empty `_REGISTERED_MODULES` list. zit.py and qwen3.py already implement the same four-step contract for their respective families, establishing the established patterns.

**Established patterns to follow:**
- Torch import guard: `try: import torch ... except ImportError: torch = None`, with `_ModuleBase = nn.Module if nn is not None else object` (though this task only creates a pure function, the guard is needed at module level).
- `framework="np"` in `safe_open()` — the function only reads `.keys()`, `.get_slice().get_shape()`, and `.get_slice().get_dtype()`, never tensor data; using `"pt"` would require torch at import time and break mock-mode collection.
- P904 regression prevention: read ALL keys via `f.keys()` without truncation — never `list(f.keys())[:N]`.
- Inner function pattern: `_infer_hyperparams_inner(f, path)` factored out so the public function's try/except cleanly wraps `safe_open()` without re-raising from inside a `with` block.
- Error handling: convert `FileNotFoundError`/`OSError` to `ValueError("cannot open safetensors file: ...")`, catch-all for other exceptions (SafetensorError) as `ValueError("cannot parse safetensors header: ...")`.
- Native dtype detection: iterate keys for first `.weight`-suffixed key, read its dtype via `get_slice(key).get_dtype()`, map via `_safetensors_dtype_to_canonical()`, default to `"fp32"` if no weight tensor found.
- Architecture detection: primary path reads `f.metadata().get("arch")`; fallback checks key naming patterns.
- Google-style docstrings on all public functions.

**Gap between design doc and current source:** `zit_vae.py` does not yet exist. The fixture shapes differ from diffusion/CLIP — VAE uses convolutional block keys (`encoder.blocks.N.conv.weight`, `decoder.blocks.N.conv.weight`, `mid_block.conv.weight`) and a `latents` tensor, rather than the projection/attention keys used by zit.py and qwen3.py. The shape-inference formula must be written independently for this key namespace.

## Resolved Dependencies

None. This task uses only `safetensors` (already in `requirements/base.txt`) and Python stdlib (`re`, `math`). No new external packages are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (stdlib) | safetensors | already in base.txt | pypi-query MCP | n/a |

No new dependencies. The `safetensors` package is already listed in `requirements/base.txt`.

## Approach

1. **Create `worker/nodes/arch/vae/zit_vae.py`** with the module docstring describing this file's role (step 1 of the four-step loading contract for ZiT VAE, ANVILML_DESIGN.md §11.3), the torch import guard (try/except ImportError pattern identical to zit.py/qwen3.py), and the `ARCH` constant (`"zit_vae"`).

2. **Implement `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str`** — maps safetensors dtype strings (`"F32"`, `"F16"`, `"BF16"`, `"F8_E4M3"`, `"F8_E5M2"`) to canonical lowercase forms (`"fp32"`, `"fp16"`, `"bf16"`, `"fp8"`), falling through to `"fp32"` for unknown strings. This is the same function as zit.py/qwen3.py but placed in this file per the independence rule (§11.4).

3. **Implement `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]`** — the inner logic that runs inside the `safe_open` context:
   - Read ALL keys: `keys = f.keys()` (P904 regression prevention — no truncation).
   - Detect native dtype: iterate keys for first `.weight`-suffixed key, call `f.get_slice(key).get_dtype()`, map via `_safetensors_dtype_to_canonical()`, default to `"fp32"`.
   - Infer encoder channel count: find the first key matching `encoder.blocks.N.conv.weight`, extract shape[0] (out_channels). If no such key, look for `xyz_encoder_block*conv` pattern and extract shape[0].
   - Infer decoder channel count: find the first key matching `decoder.blocks.N.conv.weight`, extract shape[0]. If no such key, look for `xyz_decoder_block*conv` pattern and extract shape[0].
   - Infer latent channel count: find the key ending with `latents` (or `xyz_latents` for no-metadata fixture), extract shape[1] from its shape.
   - Detect architecture string: primary path reads `f.metadata().get("arch")`; fallback checks for recognizable VAE key patterns (`encoder.blocks`, `decoder.blocks`, `mid_block`) and sets `arch = "zit_vae"` if found.
   - Return dict with keys: `encoder_channels`, `decoder_channels`, `latent_channels`, `arch`, `native_dtype`.

4. **Implement `_infer_hyperparams(path: str) -> dict[str, Any]`** — the public entry point:
   - Wrap `safe_open(path, framework="np")` in a `with` block, call `_infer_hyperparams_inner(f, path)` inside.
   - Catch `FileNotFoundError`/`OSError` → `ValueError("cannot open safetensors file: ...")`.
   - Catch-all → `ValueError("cannot parse safetensors header: ...")`.
   - Include comprehensive Google-style docstring documenting the contract, return dict keys, and raised exceptions.

5. **Create `worker/tests/test_arch_vae_zit.py`** with >=4 tests:
   - `test_infer_hyperparams_regular_fixture` — calls `_infer_hyperparams()` against `zit_vae_tiny.safetensors`, asserts returned dict has expected keys and correct values (encoder_channels=16, decoder_channels=16, latent_channels=4, arch="zit_vae", native_dtype="fp32" since tensors are created with `torch.randn()` defaulting to float32).
   - `test_infer_hyperparams_no_metadata_fixture` — calls against `zit_vae_tiny_no_metadata.safetensors`, asserts the metadata-fallback path succeeds (arch="zit_vae" from key patterns, same channel values).
   - `test_infer_hyperparams_nonexistent_path_raises` — calls with a non-existent path, asserts `ValueError` with "No such file" message.
   - `test_infer_hyperparams_truncated_header_raises` — writes a small invalid binary blob to a temp file, asserts `ValueError` is raised.

6. **Verify the test file passes**: `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0.

## Public API Surface

| Item | Module Path | Signature | Description |
|------|-------------|-----------|-------------|
| `_infer_hyperparams` | `worker.nodes.arch.vae.zit_vae` | `def _infer_hyperparams(path: str) -> dict[str, Any]` | Public entry point — opens safetensors header, infers hyperparameters. |
| `_infer_hyperparams_inner` | `worker.nodes.arch.vae.zit_vae` | `def _infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]` | Inner logic helper — runs inside `safe_open` context. |
| `_safetensors_dtype_to_canonical` | `worker.nodes.arch.vae.zit_vae` | `def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` | Maps safetensors dtype strings to canonical lowercase forms. |

Return dict from `_infer_hyperparams`:
- `encoder_channels` (int): Channel count of the first encoder conv block.
- `decoder_channels` (int): Channel count of the first decoder conv block.
- `latent_channels` (int): Channel count from the `latents` tensor.
- `arch` (str): Architecture string — `"zit_vae"` from metadata or key-pattern fallback.
- `native_dtype` (str): Canonical native dtype string inferred from the first weight tensor.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/vae/zit_vae.py` | New file — `_infer_hyperparams()` and helpers for ZiT VAE shape inference. |
| CREATE | `worker/tests/test_arch_vae_zit.py` | New file — tests for `_infer_hyperparams()` against both fixtures and malformed input. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_arch_vae_zit.py` | `test_infer_hyperparams_regular_fixture` | `_infer_hyperparams()` against `zit_vae_tiny.safetensors` returns correct dict: encoder_channels=16, decoder_channels=16, latent_channels=4, arch="zit_vae", native_dtype="fp32". | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture -v` exits 0 |
| `worker/tests/test_arch_vae_zit.py` | `test_infer_hyperparams_no_metadata_fixture` | `_infer_hyperparams()` against `zit_vae_tiny_no_metadata.safetensors` succeeds via metadata-fallback path (arch inferred from key patterns). | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture -v` exits 0 |
| `worker/tests/test_arch_vae_zit.py` | `test_infer_hyperparams_nonexistent_path_raises` | `_infer_hyperparams()` with non-existent path raises `ValueError` containing "No such file". | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0 |
| `worker/tests/test_arch_vae_zit.py` | `test_infer_hyperparams_truncated_header_raises` | `_infer_hyperparams()` with invalid binary data raises `ValueError` (SafetensorError converted to ValueError). | `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the established naming convention (`test_arch_vae_zit.py` mirrors `test_arch_zit.py`, `test_arch_clip_qwen3.py`) and is automatically discovered by `pytest worker/tests/`. The mock-mode CI jobs (`worker-linux-mock`, `worker-windows-mock`) collect this file at import time (torch is guarded, so collection succeeds without torch). The real-mode CI jobs (`worker-linux-real`, `worker-windows-real`) execute the tests against the fixture files.

## Platform Considerations

None identified. The function operates only on file paths and safetensors metadata — no platform-specific code, no path-separator handling beyond what `str(path)` provides (fixture paths are passed as strings, and Python's `open()` handles path separators natively). The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| VAE key patterns differ from diffusion/CLIP — the fixture uses `encoder.blocks.N.conv.weight` and `decoder.blocks.N.conv.weight` patterns, not projection-key patterns. If the shape-inference formula doesn't match the actual fixture keys, inference fails. | Low | High | Read the fixture builder script (`build_zit_vae_fixture.py`) to confirm exact key names before writing inference logic. The fixture keys are: `encoder.blocks.0.conv.weight`, `decoder.blocks.0.conv.weight`, `mid_block.conv.weight`, `latents`. |
| The no-metadata fixture uses `xyz_encoder_block0_conv` keys (no `.weight` suffix). The native_dtype detection iterates for `.weight`-suffixed keys and won't find any, defaulting to `"fp32"`. This is correct behavior but must not raise an error. | Low | Medium | The native_dtype loop already defaults to `"fp32"` when no `.weight` key is found — same pattern as zit.py/qwen3.py. Verify the no-metadata fixture's keys don't end with `.weight`. |
| `safetensors` dtype mapping for `xyz_` prefixed keys: since no weight tensor is found, native_dtype defaults to `"fp32"`. The fixture tensors are `torch.randn()` which defaults to `float32`, so this is correct. | Low | Low | Confirmed by reading the fixture builder — all tensors use `torch.randn()` without dtype argument, defaulting to float32. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 with >=4 tests
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/vae/zit_vae.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_vae_zit.py` exits 0
- [ ] The function `_infer_hyperparams` is importable from `worker.nodes.arch.vae.zit_vae` without torch installed (mock-mode collection safety)
