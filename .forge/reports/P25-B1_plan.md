# Plan Report: P25-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P25-B1                                        |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE   |
| Description | worker/nodes/arch/diffusion/flux2klein.py: shape inference from header (4B) |
| Depends on  | P25-A1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-07-22T08:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `worker/nodes/arch/diffusion/flux2klein.py` implementing `_infer_hyperparams(path: str) -> dict[str, Any]` — step 1 of the four-step loading contract (ANVILML_DESIGN.md §11.3) for the Flux 2 Klein 4B diffusion architecture. The function opens the checkpoint header-only, reads ALL keys (no truncation, per the P904 regression-prevention discipline established in zit.py), and returns a dict of inferred hyperparameters. Accompanying test file `worker/tests/test_arch_flux2klein.py` provides >= 3 tests exercising correct inference against the P25-A1 4B fixture, the no-metadata fallback path, and malformed/truncated input error handling.

## Scope

### In Scope
- `worker/nodes/arch/diffusion/flux2klein.py` — new file containing:
  - Module-level docstring describing the four-step loading contract.
  - Guarded import block (torch optional at import time, per §11.2).
  - `ARCH: str = "flux2klein"` — canonical architecture identifier.
  - `logger = logging.getLogger(__name__)` — module logger.
  - `_infer_hyperparams(path: str) -> dict[str, Any]` — public function implementing step 1 of the loading contract.
  - `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]` — inner logic helper (factored out from zit.py for clean exception wrapping).
  - `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` — dtype string normalisation helper.
- `worker/tests/test_arch_flux2klein.py` — new test file with >= 3 tests:
  - `test_infer_hyperparams_regular_fixture` — validates all expected keys and values against `flux2klein4b_tiny.safetensors`.
  - `test_infer_hyperparams_no_metadata_fixture` — validates the metadata-fallback path against `flux2klein4b_tiny_no_metadata.safetensors`.
  - `test_infer_hyperparams_nonexistent_path_raises` — validates ValueError on missing file.
  - `test_infer_hyperparams_truncated_header_raises` — validates ValueError on corrupted data.

### Out of Scope
- `can_handle(key) -> bool` — deferred to P25-B2 (the task's own `defers_to` field names it).
- Registration into `arch/diffusion/__init__.py`'s dispatcher — deferred to P25-B2.
- `load()`, `sample()`, `compute_latent_shape()` — deferred to P25-C1, P25-C2, P25-D1 respectively.
- `ZiTModel` / `Flux2KleinModel` class definition — deferred to P25-C1 (meta-device construction).
- Dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) — not applicable to private functions (`_infer_hyperparams`). The §10.6 marker convention applies to `load()`, `sample()`, `compute_latent_shape()` (ANVILML_DESIGN.md §10.4's fixed method names), none of which this task implements.

## Existing Codebase Assessment

**What already exists:** The P25-A1 fixture files (`flux2klein4b_tiny.safetensors` and `flux2klein4b_tiny_no_metadata.safetensors`) are already present in `worker/tests/fixtures/`, built by `build_flux2klein_fixture.py`. The ZiT module (`zit.py`, 1300 lines) provides the exact reference pattern for `_infer_hyperparams()`: guarded torch imports, `safe_open(..., framework="np")`, reading ALL keys without truncation, dtype normalisation, and metadata-fallback architecture detection. The `arch/diffusion/__init__.py` dispatcher already imports and registers `zit`.

**Established patterns:**
- **Import guarding:** torch is imported under `try/except ImportError` with sentinel `None` assignments, keeping `_infer_hyperparams()` importable without torch (mock-mode collection safety).
- **Exception wrapping:** `_infer_hyperparams()` wraps the `safe_open` context in try/except, converting `FileNotFoundError`/`OSError` → `ValueError("cannot open safetensors file: ...")` and all other exceptions → `ValueError("cannot parse safetensors header: ...")`.
- **Key reading discipline:** `keys = f.keys()` — never truncated, never sliced. P904 regression prevention documented inline.
- **Dtype mapping:** `_safetensors_dtype_to_canonical()` maps `"F32"`→`"fp32"`, `"F16"`→`"fp16"`, `"BF16"`→`"bf16"`, `"F8_E4M3"`/`"F8_E5M2"`→`"fp8"`, unknown→`"fp32"`.
- **Test style:** Tests import only the public interface, use `_FIXTURE_DIR = Path(__file__).parent / "fixtures"`, and follow Google-style docstrings describing what, how, and expected outcome.

**Gap between design doc and source:** The design doc (§11.3 step 1) specifies `framework="pt"` but zit.py uses `framework="np"` to keep `_infer_hyperparams()` importable without torch installed. This is the established pattern, not a gap — the design doc describes the general contract, and the implementation adapts it for mock-mode compatibility (documented in zit.py's inline comment).

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| python | safetensors | 0.5.x (from base.txt) | pypi-query MCP | n/a |

No new dependencies are introduced. `safetensors` is already a dependency of the Python worker (in `worker/requirements/base.txt`). The `framework="np"` argument to `safe_open()` is part of the safetensors API — confirmed to exist in all recent versions.

## Approach

**Step 1 — Create `worker/nodes/arch/diffusion/flux2klein.py`.**

1. Write the module docstring describing: (a) the four-step loading contract (§11.3), (b) Flux 2 Klein's architecture (modulated cross-attention blocks with `img_mod`/`txt_mod` adaptive LN, `img_attn`/`txt_attn` attention, `img_mlp`/`txt_mlp` SwiGLU MLPs, and `single_blocks` with linear layers, `final_layer` with adaptive LN modulation).

2. Add guarded import block (same pattern as zit.py lines 35-57):
   ```python
   try:
       import torch
       import torch.nn as nn
       from safetensors.torch import load_file
   except ImportError:
       torch = None  # type: ignore[assignment]
       nn = None  # type: ignore[assignment]
       load_file = None  # type: ignore[assignment]
   ```
   This keeps `_infer_hyperparams()` importable without torch (mock-mode test collection safety, per §11.2).

3. Define `ARCH: str = "flux2klein"` — the canonical architecture identifier that `can_handle()` (P25-B2) will compare against.

4. Define `logger = logging.getLogger(__name__)` — module-level logger for debug/info output.

5. Implement `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str`:
   - Same mapping as zit.py: `"F32"`→`"fp32"`, `"F16"`→`"fp16"`, `"BF16"`→`"bf16"`, `"F8_E4M3"`→`"fp8"`, `"F8_E5M2"`→`"fp8"`, unknown→`"fp32"`.
   - This is a pure function, no dependencies.

6. Implement `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]`:
   - **0. Detect native_dtype:** Iterate over `f.keys()`, find the first key ending in `".weight"`, call `f.get_slice(key).get_dtype()`, map through `_safetensors_dtype_to_canonical()`. Default to `"fp32"` if no weight tensor found (no-metadata fixtures with `xyz_` keys may lack `.weight` suffix).
   - **1. Infer hidden_dim:** Iterate over all keys. Look for keys ending in `time_text_embed.timestep_embedder.0.weight` (primary pattern for Flux 2 Klein) or `time_text_embed.context_embedder`. Get the shape's first dimension. If neither found, raise `ValueError`.
   - **2. Count double blocks:** Scan all keys for `double_blocks.` prefix, extract numeric suffix via regex `r"double_blocks\.(\d+)"`, count = max_index + 1. Fallback: count keys containing `"double_block"` and divide by 2 (pairs of mod+attention tensors per block).
   - **3. Count single blocks:** Scan all keys for `single_blocks.` prefix, extract numeric suffix via regex `r"single_blocks\.(\d+)"`, count = max_index + 1. Fallback: count keys containing `"single_block"` — each is one single block.
   - **4. Infer latent dimensions:** Find key ending in `"latents"`, get shape[1] as `latent_channels`. For latent_height/width: primary path — find key ending in `final_layer.linear`, use `shape[1] / (PATCH_SIZE * PATCH_SIZE * OUT_CHANNELS)` to derive latent_dim, then `latent_height = latent_width = sqrt(latent_dim / latent_channels)`. Fallback — use `latents` tensor shape[2] and shape[3].
   - **5. Derive patch_size:** `patch_size = latent_height` (for square latent tensors, same as zit.py).
   - **6. Infer architecture string:** Primary path — check `f.metadata().get("arch")`. Fallback — scan keys for Flux 2 Klein patterns (`"double_block"`, `"single_block"`, `"final_layer"`, `"img_mod"`, `"txt_mod"`). If neither path matches, raise `ValueError`.
   - Return dict with all inferred hyperparameters.

7. Implement `_infer_hyperparams(path: str) -> dict[str, Any]`:
   - Public wrapper that calls `safe_open(path, framework="np")` inside a `with` block, passing the handle to `_infer_hyperparams_inner()`.
   - Wraps in try/except: `FileNotFoundError`/`OSError` → `ValueError("cannot open safetensors file: ...")`, all others → `ValueError("cannot parse safetensors header: ...")`.
   - Full Google-style docstring with Args/Returns/Raises sections.

8. Add inline comments at every decision point: dtype default rationale, key pattern rationale, fallback path rationale, P904 truncation prevention note.

**Step 2 — Create `worker/tests/test_arch_flux2klein.py`.**

1. Write module docstring and guarded torch import (same pattern as zit's test file).
2. Import `_infer_hyperparams` from `worker.nodes.arch.diffusion.flux2klein`.
3. Define `_FIXTURE_DIR = Path(__file__).parent / "fixtures"`.

4. Test `test_infer_hyperparams_regular_fixture`:
   - Call `_infer_hyperparams(str(_FIXTURE_DIR / "flux2klein4b_tiny.safetensors"))`.
   - Assert all expected keys present: hidden_dim, double_block_count, single_block_count, latent_channels, latent_height, latent_width, patch_size, arch, native_dtype.
   - Assert correct values: hidden_dim=128, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=8, arch="flux2klein", native_dtype="fp32".

5. Test `test_infer_hyperparams_no_metadata_fixture`:
   - Call `_infer_hyperparams(str(_FIXTURE_DIR / "flux2klein4b_tiny_no_metadata.safetensors"))`.
   - Assert shape-based hyperparameters match (same values as regular fixture).
   - Assert arch is detected via key patterns fallback (the no-metadata fixture uses `xyz_`-prefixed keys that contain `"double_block"` and `"single_block"` substrings).

6. Test `test_infer_hyperparams_nonexistent_path_raises`:
   - Call with a path that doesn't exist, assert `ValueError` raised with `"No such file"` in message.

7. Test `test_infer_hyperparams_truncated_header_raises`:
   - Create a temp file with invalid binary data (8 bytes), call `_infer_hyperparams()`, assert `ValueError` raised.

**Step 3 — Pre-test static check.** Run `python -m py_compile worker/nodes/arch/diffusion/flux2klein.py worker/tests/test_arch_flux2klein.py` to confirm syntax before test execution.

**Step 4 — Run tests.** Run `python -m pytest worker/tests/test_arch_flux2klein.py -v`. Expect >= 4 tests, all passing, exit 0.

## Public API Surface

| Path | Item | Signature |
|------|------|-----------|
| `worker.nodes.arch.diffusion.flux2klein` | `ARCH` (module constant) | `ARCH: str = "flux2klein"` |
| `worker.nodes.arch.diffusion.flux2klein` | `_infer_hyperparams` | `_infer_hyperparams(path: str) -> dict[str, Any]` |
| `worker.nodes.arch.diffusion.flux2klein` | `_infer_hyperparams_inner` | `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]` |
| `worker.nodes.arch.diffusion.flux2klein` | `_safetensors_dtype_to_canonical` | `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str` |

All functions are private (prefixed with `_`) except `ARCH`. No `pub` items in the Python sense — this is a module-level function.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/flux2klein.py` | New Flux 2 Klein diffusion arch module — `_infer_hyperparams()` only |
| CREATE | `worker/tests/test_arch_flux2klein.py` | Test file with >= 4 tests for `_infer_hyperparams()` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_flux2klein.py` | `test_infer_hyperparams_regular_fixture` | _infer_hyperparams() returns correct hyperparameters for the regular Flux 2 Klein 4B fixture (with arch metadata) | P25-A1 fixture `flux2klein4b_tiny.safetensors` exists | Path to regular fixture | Dict with hidden_dim=128, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=8, arch="flux2klein", native_dtype="fp32" | `python -m pytest worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_regular_fixture -v` exits 0 |
| `worker/tests/test_arch_flux2klein.py` | `test_infer_hyperparams_no_metadata_fixture` | _infer_hyperparams() infers arch from key patterns when metadata is absent | P25-A1 fixture `flux2klein4b_tiny_no_metadata.safetensors` exists | Path to no-metadata fixture | Dict with same shape values as regular fixture, arch detected via key patterns | `python -m pytest worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_no_metadata_fixture -v` exits 0 |
| `worker/tests/test_arch_flux2klein.py` | `test_infer_hyperparams_nonexistent_path_raises` | _infer_hyperparams() raises ValueError for non-existent path | None | Path to nonexistent file | ValueError raised | `python -m pytest worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0 |
| `worker/tests/test_arch_flux2klein.py` | `test_infer_hyperparams_truncated_header_raises` | _infer_hyperparams() raises ValueError for corrupted/truncated file | None | Temp file with invalid binary data | ValueError raised | `python -m pytest worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_truncated_header_raises -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the existing naming convention (`test_arch_*.py`) and is automatically collected by pytest's standard discovery. It does not import `torch` at module level (guarded import), so it is safe for collection in the `worker-linux-mock` / `worker-windows-mock` CI jobs that install `requirements/base.txt` only (no torch). The tests exercise `_infer_hyperparams()` which uses `safetensors` (already in base.txt).

## Platform Considerations

None identified. The `_infer_hyperparams()` function is platform-neutral: it reads file headers using `safetensors` (pure C extension, cross-platform), uses only Python stdlib (`os`, `re`, `logging`), and performs no platform-specific operations. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Flux 2 Klein key patterns differ from ZiT patterns in ways the inference logic doesn't account for (e.g. `time_text_embed.timestep_embedder.0.weight` shape inference may not match how `hidden_dim` is derived from `final_layer.linear` shape). | Medium | High | The fixture builder (`build_flux2klein_fixture.py`) already defines the key patterns and tensor shapes. Build the inference logic by reverse-engineering from the fixture: inspect every key in the fixture, trace how each hyperparameter maps to a key, and write the inference to match. Test against the fixture first, then validate against the no-metadata variant. |
| The no-metadata fixture's `xyz_`-prefixed keys may not contain recognizable Flux 2 Klein patterns (the `xyz_` prefix removes `.` from key names like `double_blocks.0.img_mod.lin` → `xyz_double_blocks_0_img_mod_lin`). The fallback pattern matching (`"double_block" in key`) must still match. | Low | Medium | The `xyz_`-prefixed keys still contain the substrings `"double_block"` and `"single_block"` (e.g. `xyz_double_blocks_0_img_mod_lin` contains `"double_block"`). The pattern check is a substring match, not a prefix match, so it will match. Verify by running the test against the no-metadata fixture. |
| `framework="np"` in `safe_open()` may not be available in older safetensors versions. | Low | Medium | The project's `base.txt` pins a recent safetensors version that supports `framework="np"`. If an older version is encountered, fall back to `framework="pt"` — but this would require torch at import time for the header read, breaking mock-mode collection. The MCP lookup confirms current safetensors supports `"np"`. |
| `_infer_hyperparams()` is called in both mock-mode (test collection) and real-mode (actual loading). If the function raises on import, it breaks mock-mode test collection. | Low | High | The guarded torch import ensures torch is absent during mock-mode collection, but `_infer_hyperparams()` never imports torch — it only uses `safetensors` which is in base.txt. The function is import-safe. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/flux2klein.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_flux2klein.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py -v` exits 0 with >= 4 tests passing
- [ ] `test_infer_hyperparams_regular_fixture` asserts all expected keys and values match the P25-A1 4B fixture
- [ ] `test_infer_hyperparams_no_metadata_fixture` asserts shape-based hyperparameters and key-pattern arch detection
- [ ] `test_infer_hyperparams_nonexistent_path_raises` asserts ValueError on missing file
- [ ] `test_infer_hyperparams_truncated_header_raises` asserts ValueError on corrupted data
