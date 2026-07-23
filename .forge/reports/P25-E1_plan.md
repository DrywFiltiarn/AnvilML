# Plan Report: P25-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P25-E1                                            |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE       |
| Description | worker/nodes/arch/vae/flux2_vae.py: full load() + decode() (single task) |
| Depends on  | P25-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-23T09:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/nodes/arch/vae/flux2_vae.py` implementing the complete four-step loading
contract (`_infer_hyperparams()`, `can_handle()`, `load()`, `decode()`) for the Flux 2
VAE architecture, register it as the **second** real entry in the VAE dispatcher
(`arch/vae/__init__.py`), and create a comprehensive test suite
(`test_arch_vae_flux2.py`) with ≥15 tests covering every contract step plus `decode()`
against the P25-A1 Flux 2 VAE fixture. This mirrors the full zit_vae.py contract from
Phase 23 in a single task, following the established pattern with zero architectural
decisions required.

## Scope

### In Scope
- Create `worker/nodes/arch/vae/flux2_vae.py` with:
  - `ARCH = "flux2"` constant
  - `_safetensors_dtype_to_canonical()` — identical to zit_vae.py (shared utility)
  - `_infer_hyperparams(path)` — reads all keys from safetensors header, infers
    encoder_channels, decoder_channels, latent_channels, arch, native_dtype
  - `Flux2VaeModel(nn.Module)` — meta-device construction with encoder blocks,
    mid-block, decoder blocks (same topology as ZiTVaeModel: Conv2d + GroupNorm + SiLU)
  - `_select_dtype(caps, native_dtype)` — identical precedence to zit_vae.py (§11.5)
  - `_build_key_remapping(ckpt_keys, mod_keys)` — handles `encoder.blocks.N` →
    `encoder.block_N` and `decoder.blocks.N` → `decoder.block_N` patterns, plus
    `xyz_`-prefixed no-metadata keys
  - `load(path, caps, device)` — full four-step contract: meta construction, dtype
    selection, to_empty() + zero-init + remap + load_state_dict(assign=True) + .arch
  - `decode(vae_module, latent, output_mode)` — forward pass, clamp, NCHW→HWC, PIL
  - `can_handle(key)` — returns True for `"flux2"`
  - Import guards for torch, numpy, PIL (mock-mode collection safety)
  - REAL_PATH_VERIFIED / MOCK_PATH_VERIFIED markers on `load()` and `decode()`
  - Google-style docstrings on all public functions and classes
- Modify `worker/nodes/arch/vae/__init__.py` — import and register `flux2_vae` alongside
  `zit_vae` in `_REGISTERED_MODULES`
- Create `worker/tests/test_arch_vae_flux2.py` with ≥15 tests:
  - `_infer_hyperparams()` against regular + no-metadata fixtures
  - `_infer_hyperparams()` error cases (nonexistent path, corrupt file)
  - `ARCH` constant verification
  - `can_handle()` matching "flux2" and rejecting "zit_vae" (disambiguation)
  - `get_module()` dispatcher verification for "flux2" key
  - `load()` meta construction, dtype selection (4 branches), weight loading, .arch
  - `decode()` against fixture-shaped latent
  - Mock-mode sentinel tests for `load()` and `decode()`
- Update `docs/TESTS.md` with entries for every new test

### Out of Scope
None. defers_to (from JSON): [] — this task must implement its full scope.

## Existing Codebase Assessment

**What already exists:** The ZiT VAE module (`zit_vae.py`) provides a complete reference
implementation of the four-step loading contract with identical topology (encoder blocks,
mid-block, decoder blocks using Conv2d + GroupNorm + SiLU). The VAE dispatcher
(`arch/vae/__init__.py`) already imports and registers `zit_vae`. The Flux 2 VAE fixtures
(`flux2_vae_tiny.safetensors` and `flux2_vae_tiny_no_metadata.safetensors`) already exist
from P25-A1, using the same key patterns (`encoder.blocks.N`, `decoder.blocks.N`,
`mid_block.*`, `latents`) as the ZiT VAE. The test infrastructure (`conftest.py`,
fixture builder conventions, pytest marker registration) is fully established.

**Established patterns:** Import guards wrapping torch/numpy/PIL at module scope behind
`try/except ImportError`. Module-level `ARCH` constant. `_ModuleBase = nn.Module if nn
is not None else object` for conditional inheritance. `_select_dtype()` with the fixed
precedence chain (fp8 → bf16 → fp16 → fp32). Key remapping via regex patterns.
`load()` ordering: meta construction → dtype → to_empty() → zero-init → verify .arch →
load_file → build remap → cast-to-dtype → load_state_dict(assign=True, strict=False).
`decode()`: forward pass → clamp → cpu → numpy → float32 cast → channel select →
scale to uint8 → transpose NCHW→HWC → PIL. REAL_PATH_VERIFIED / MOCK_PATH_VERIFIED
markers on `load()` and `decode()` naming tests in the same test file.

**Gap between design doc and current source:** None that affects this task. The Flux 2
VAE fixtures already use the same key patterns as ZiT VAE (encoder.blocks.N,
decoder.blocks.N, mid_block.*), meaning the `_build_key_remapping` logic is identical.
The only difference is the architecture identifier: "flux2" (from fixture metadata)
versus "zit_vae". The dispatcher `__init__.py` currently only registers `zit_vae` and
needs the `flux2_vae` import added.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|------------|-----------------|------------|------------------------|
| python | torch      | (project venv)  | N/A        | N/A                    |
| python | safetensors| (project venv)  | N/A        | N/A                    |
| python | pillow     | (project venv)  | N/A        | N/A                    |
| python | numpy      | (project venv)  | N/A        | N/A                    |

No new external dependencies are introduced. All imports (torch, torch.nn,
safetensors.torch, numpy, PIL.Image) are already declared in `worker/requirements/base.txt`
and used by the existing `zit_vae.py` module.

## Approach

### Step 1: Create `worker/nodes/arch/vae/flux2_vae.py`

**1a. Module docstring and imports.** Write a module-level docstring mirroring
zit_vae.py's format: "Flux 2 VAE architecture module — shape inference from
safetensors header" with summary of the four-step contract. Then add guarded imports:

```python
try:
    import numpy as np
    from PIL import Image
except ImportError:
    np = None  # type: ignore[assignment]
    Image = None  # type: ignore[assignment]

try:
    import torch
    import torch.nn as nn
    import torch.nn.functional as F
    from safetensors.torch import load_file
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
    F = None  # type: ignore[assignment]
    load_file = None  # type: ignore[assignment]

_ModuleBase = nn.Module if nn is not None else object
```

**1b. ARCH constant.** Set `ARCH: str = "flux2"`. This matches the `arch` value in
the P25-A1 fixture metadata (`metadata={"arch": "flux2"}`).

**1c. `_safetensors_dtype_to_canonical()` function.** Copy identically from zit_vae.py.
This is a pure utility — same dtype mappings (F32→fp32, F16→fp16, BF16→bf16,
F8_E4M3→fp8, F8_E5M2→fp8, default→fp32). No changes needed.

**1d. `_infer_hyperparams_inner(f, path)` function.** This function reads the
safetensors header and infers hyperparameters. The Flux 2 VAE uses the **same key
patterns** as ZiT VAE:

- **Native dtype:** iterate keys for first `.weight`-suffixed key, read dtype via
  `get_slice().get_dtype()` → `_safetensors_dtype_to_canonical()`. Default "fp32".
- **Encoder channels:** match `encoder.blocks.N.conv.weight` → `shape[1]` (in_channels).
  Fallback: `xyz_encoder_block*conv.weight` → `shape[1]`.
- **Decoder channels:** match `decoder.blocks.N.conv.weight` → `shape[0]` (out_channels).
  Fallback: `xyz_decoder_block*conv.weight` → `shape[0]`.
- **Latent channels:** key ending with `latents` → `shape[1]`.
  Fallback: key ending with `xyz_latents` → `shape[1]`.
- **Architecture string:** read `f.metadata().get("arch")` → "flux2". Fallback: detect
  from key patterns (encoder.blocks, decoder.blocks, mid_block) → "flux2".

The implementation is structurally identical to zit_vae.py's `_infer_hyperparams_inner`
with the same regex patterns and fallback chain. The only difference is the `arch`
value ("flux2" vs "zit_vae") which comes from the metadata.

**1e. `_infer_hyperparams(path)` function.** Wrapper that opens `safe_open(path,
framework="np")` and calls `_infer_hyperparams_inner`, with the same error handling
(FileNotFoundError→ValueError, catch-all→ValueError). Identical to zit_vae.py.

**1f. `Flux2VaeModel(_ModuleBase)` class.** Construct on meta-device using the same
topology as `ZiTVaeModel`:

- **encoder blocks:** `nn.ModuleDict` with block_0 through block_{N-1}. Each block
  contains `conv` (Conv2d) and `norm` (GroupNorm). Channel interpolation from
  `encoder_channels` to `latent_channels` across blocks.
- **mid-block:** single Conv2d + GroupNorm at `latent_channels`.
- **decoder blocks:** `nn.ModuleDict` with block_0 through block_{N-1}. Channel
  interpolation from `latent_channels` to `decoder_channels` across blocks.
- **`self.arch = "flux2"`** set in `__init__`.
- **`forward(self, latent)`** method: mid-block → sequential decoder blocks, each
  applying Conv2d → GroupNorm → SiLU. Returns tensor of same shape as input.

The topology formula is identical to ZiTVaeModel — both use the same encoder→mid→decoder
block structure with linear channel interpolation. The only difference is `self.arch =
"flux2"`.

**1g. `_select_dtype(caps, native_dtype)` function.** Copy identically from zit_vae.py.
The precedence chain (fp8 if caps.fp8 AND native==fp8 → bf16 → fp16 → fp32) is the
same for all VAE architectures per §11.5.

**1h. `_build_key_remapping(ckpt_keys, mod_keys)` function.** Identical to zit_vae.py's
implementation. The Flux 2 VAE fixture uses the same key patterns:

- `encoder.blocks.N.suffix` → `encoder.block_N.suffix`
- `decoder.blocks.N.suffix` → `decoder.block_N.suffix`
- `xyz_encoder_blockN_suffix` → `encoder.block_N.suffix`
- `xyz_decoder_blockN_suffix` → `decoder.block_N.suffix`
- `xyz_mid_block_conv` → `mid_block.conv.weight`
- `xyz_mid_block_norm` → `mid_block.norm.weight`

These patterns match the constructed module's key naming (singular "block_N" with
underscore) and the fixture's checkpoint key naming (plural "blocks.N" with dot).
The remapping function is architecture-agnostic in its pattern set — it handles any
VAE that follows this naming convention.

**1i. `load(path, caps, device)` function.** Full four-step contract:

```
1. torch is None check → RuntimeError
2. hyperparams = _infer_hyperparams(path)  — step 1 of contract
3. target_dtype = _select_dtype(caps, hyperparams["native_dtype"])  — step 2a
4. with torch.device("meta"): model = Flux2VaeModel(hyperparams)  — step 2b
5. model.to(target_dtype)  — step 2c
6. model = model.to_empty(device=device)  — step 3a
7. Zero-init all parameters and buffers  — step 3b
8. Verify .arch == ARCH, re-set if needed  — step 3c
9. state_dict = load_file(path, device=device)  — step 3d
10. remap = _build_key_remapping(...)  — step 3e
11. Cast tensors to target_dtype, filter by shape match  — step 3f
12. model.load_state_dict(remapped, assign=True, strict=False)  — step 3g
13. Return model with .arch set
```

The logging pattern: DEBUG for dtype selection and materialization, INFO for load
summary (loaded/missing/unexpected counts). Identical to zit_vae.py.

**1j. `decode(vae_module, latent, output_mode)` function.** Identical to zit_vae.py's
decode:

```
1. torch is None check → RuntimeError
2. model_dtype = next(vae_module.parameters()).dtype
3. latent = latent.to(model_dtype)
4. decoded = vae_module.forward(latent)
5. decoded = torch.clamp(decoded, 0.0, 1.0)
6. decoded_np = decoded.detach().to(torch.float32).cpu().numpy()
7. Channel select based on output_mode (RGB → first 3, L → first 1)
8. Scale to uint8: (decoded_np * 255).astype("uint8")
9. For each batch item: transpose NCHW→HWC, create PIL Image
10. Return list of PIL Images
```

**1k. `can_handle(key)` function.** Simple equality check: `return key == ARCH`
(i.e., returns True for `"flux2"`, False for everything else including `"zit_vae"`).

**1l. Dual-mode parity markers.** Add markers on `load()` and `decode()`:

```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_vae_flux2.py::test_load_real_flux2_vae_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_flux2.py::test_load_mock_returns_sentinel
```

```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_vae_flux2.py::test_decode_real_flux2_vae_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_flux2.py::test_decode_mock_returns_sentinel
```

### Step 2: Modify `worker/nodes/arch/vae/__init__.py`

Add import and registration of `flux2_vae`:

```python
from worker.nodes.arch.vae import flux2_vae

_REGISTERED_MODULES: list[ModuleType] = []
_REGISTERED_MODULES.append(zit_vae)
_REGISTERED_MODULES.append(flux2_vae)
```

This makes `flux2_vae` the **second** real entry in the VAE dispatcher. The order
(zit_vae first, flux2_vae second) means `get_module("zit_vae")` returns zit_vae
immediately without scanning flux2_vae, and `get_module("flux2")` passes through
zit_vae (which returns False) before reaching flux2_vae (which returns True). This
bidirectional disambiguation is the same pattern used in the diffusion dispatcher.

### Step 3: Create `worker/tests/test_arch_vae_flux2.py`

Create the test file following the exact structure of `test_arch_vae_zit.py`:

**3a. Module-level guarded torch import** (mock-mode collection safety):

```python
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]
```

**3b. Import block:**

```python
from worker.nodes.arch.vae.flux2_vae import (
    ARCH,
    Flux2VaeModel,
    _build_key_remapping,
    _infer_hyperparams,
)
```

**3c. Fixture path constant:** `_FIXTURE_DIR = Path(__file__).parent / "fixtures"`

**3d. Tests (≥15 total):**

| # | Test Name | What It Verifies | Mode |
|---|-----------|-----------------|------|
| 1 | `test_infer_hyperparams_regular_fixture` | _infer_hyperparams returns correct hyperparams for flux2_vae_tiny.safetensors (arch="flux2", channel counts from key patterns) | mock-compatible |
| 2 | `test_infer_hyperparams_no_metadata_fixture` | _infer_hyperparams infers arch="flux2" from key patterns when metadata absent (xyz_ prefixed keys) | mock-compatible |
| 3 | `test_infer_hyperparams_nonexistent_path_raises` | ValueError for nonexistent path | mock-compatible |
| 4 | `test_infer_hyperparams_truncated_header_raises` | ValueError for corrupt binary data | mock-compatible |
| 5 | `test_arch_constant` | ARCH == "flux2" | mock-compatible |
| 6 | `test_can_handle_matches_flux2_key` | can_handle("flux2") is True | mock-compatible |
| 7 | `test_can_handle_rejects_zit_vae_key` | can_handle("zit_vae") is False (disambiguation) | mock-compatible |
| 8 | `test_get_module_returns_flux2_vae_for_matching_key` | get_module("flux2") returns flux2_vae module | mock-compatible |
| 9 | `test_load_meta_construction_succeeds` | load() returns Flux2VaeModel with bf16 params on cpu, .arch="flux2" | real_mode |
| 10 | `test_load_meta_construction_no_metadata_fixture` | load() against no-metadata fixture succeeds, .arch set | real_mode |
| 11 | `test_load_dtype_fp32_fallback` | load() with all caps False → fp32 params | real_mode |
| 12 | `test_load_weights_loaded_regular_fixture` | Full load: weights loaded, non-zero params, bf16 dtype | real_mode |
| 13 | `test_load_mock_returns_sentinel` | load() with patched load_file returns valid model (remap + load_state_dict path) | real_mode (mock-mode variant) |
| 14 | `test_load_real_flux2_vae_fixture` | End-to-end load against regular fixture, all steps verified | real_mode |
| 15 | `test_decode_real_flux2_vae_fixture` | decode() against loaded model produces valid PIL Image (RGB, 8×8) | real_mode |
| 16 | `test_decode_mock_returns_sentinel` | decode() post-processing with patched forward returns PIL Image | real_mode (mock-mode variant) |

Tests 1-8 verify the dispatch/inference contract (mock-compatible, no torch import
needed for collection). Tests 9-14 verify the load contract (real_mode marked, torch
required). Tests 15-16 verify decode (real_mode marked). The total is 16 tests,
exceeding the ≥15 requirement.

### Step 4: Update `docs/TESTS.md`

Add entries for all 16 new tests with their Mode (mock/real/both) field per the
ANVILML_DESIGN.md §17.1 test catalogue format.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `ARCH` | `worker.nodes.arch.vae.flux2_vae` | `str = "flux2"` |
| `_infer_hyperparams(path)` | `worker.nodes.arch.vae.flux2_vae` | `(path: str) -> dict[str, Any]` |
| `Flux2VaeModel` | `worker.nodes.arch.vae.flux2_vae` | `class Flux2VaeModel(_ModuleBase)` |
| `_select_dtype(caps, native_dtype)` | `worker.nodes.arch.vae.flux2_vae` | `(caps: dict, native_dtype: str) -> torch.dtype` |
| `_build_key_remapping(ckpt_keys, mod_keys)` | `worker.nodes.arch.vae.flux2_vae` | `(ckpt_keys: list[str], mod_keys: list[str]) -> dict[str, str]` |
| `load(path, caps, device)` | `worker.nodes.arch.vae.flux2_vae` | `(path: str, caps: dict, device: str = "cpu") -> Flux2VaeModel` |
| `decode(vae_module, latent, output_mode)` | `worker.nodes.arch.vae.flux2_vae` | `(vae_module: Flux2VaeModel, latent: torch.Tensor, output_mode: str = "RGB") -> list` |
| `can_handle(key)` | `worker.nodes.arch.vae.flux2_vae` | `(key: str) -> bool` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/vae/flux2_vae.py` | Full Flux 2 VAE arch module: _infer_hyperparams, Flux2VaeModel, load, decode, can_handle |
| MODIFY | `worker/nodes/arch/vae/__init__.py` | Import and register flux2_vae as second entry in _REGISTERED_MODULES |
| CREATE | `worker/tests/test_arch_vae_flux2.py` | ≥16 tests covering every contract step |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for all new tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_arch_vae_flux2.py` | `test_infer_hyperparams_regular_fixture` | _infer_hyperparams returns correct hyperparams for flux2_vae_tiny.safetensors (arch="flux2", channels from key patterns) | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_infer_hyperparams_regular_fixture -v` |
| `test_arch_vae_flux2.py` | `test_infer_hyperparams_no_metadata_fixture` | _infer_hyperparams infers arch="flux2" from xyz_ key patterns when metadata absent | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_infer_hyperparams_no_metadata_fixture -v` |
| `test_arch_vae_flux2.py` | `test_infer_hyperparams_nonexistent_path_raises` | ValueError raised for nonexistent file path | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_infer_hyperparams_nonexistent_path_raises -v` |
| `test_arch_vae_flux2.py` | `test_infer_hyperparams_truncated_header_raises` | ValueError raised for corrupt binary data | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_infer_hyperparams_truncated_header_raises -v` |
| `test_arch_vae_flux2.py` | `test_arch_constant` | ARCH == "flux2" | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_arch_constant -v` |
| `test_arch_vae_flux2.py` | `test_can_handle_matches_flux2_key` | can_handle("flux2") returns True | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_can_handle_matches_flux2_key -v` |
| `test_arch_vae_flux2.py` | `test_can_handle_rejects_zit_vae_key` | can_handle("zit_vae") returns False (disambiguation from zit_vae fixture) | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_can_handle_rejects_zit_vae_key -v` |
| `test_arch_vae_flux2.py` | `test_get_module_returns_flux2_vae_for_matching_key` | vae.get_module("flux2") returns flux2_vae module | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_get_module_returns_flux2_vae_for_matching_key -v` |
| `test_arch_vae_flux2.py` | `test_load_meta_construction_succeeds` | load() returns Flux2VaeModel with bf16 params on cpu, .arch="flux2" | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_meta_construction_succeeds -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_load_meta_construction_no_metadata_fixture` | load() against no-metadata fixture succeeds, .arch set | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_meta_construction_no_metadata_fixture -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_load_dtype_fp32_fallback` | load() with all caps False → fp32 params | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_dtype_fp32_fallback -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_load_weights_loaded_regular_fixture` | Full load: weights loaded, non-zero params, correct dtype | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_weights_loaded_regular_fixture -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_load_mock_returns_sentinel` | load() with patched load_file exercises remap + load_state_dict path | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_mock_returns_sentinel -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_load_real_flux2_vae_fixture` | End-to-end load against regular fixture, all steps verified | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_load_real_flux2_vae_fixture -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_decode_real_flux2_vae_fixture` | decode() produces valid PIL Image (RGB, 8×8) from fixture-shaped latent | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_decode_real_flux2_vae_fixture -v -m real_mode` |
| `test_arch_vae_flux2.py` | `test_decode_mock_returns_sentinel` | decode() post-processing with patched forward returns PIL Image | `python -m pytest worker/tests/test_arch_vae_flux2.py::test_decode_mock_returns_sentinel -v -m real_mode` |

## CI Impact

No CI workflow files are modified. The new test file `test_arch_vae_flux2.py` is
automatically picked up by:
- `worker-linux-mock` CI job: collected during mock-mode collection (guarded torch
  import prevents collection errors), tests 1-8 run without torch.
- `worker-linux-real` CI job: all 16 tests run (tests 9-16 are real_mode-marked).
- `worker-windows-mock` / `worker-windows-real`: same coverage on Windows.

No new CI gates or jobs are needed. The existing pytest discovery mechanism picks up
the new test file automatically.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All imports
(torch, safetensors.torch, numpy, PIL) are cross-platform. The Conv2d/GroupNorm/SiLU
topology uses PyTorch primitives that work identically on CPU across platforms. No
`#[cfg(...)]` guards or path-separator handling needed in Python code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Flux 2 VAE fixture key patterns differ from ZiT VAE patterns, causing `_build_key_remapping` to miss keys | Low | Medium | The fixture builder script (P25-A1) uses the same `encoder.blocks.N` / `decoder.blocks.N` / `mid_block.*` patterns as ZiT VAE. Verify by reading the fixture file with `safetensors.safe_open` before writing the remapping. The existing zit_vae.py remapping patterns already cover these. |
| `_infer_hyperparams()` latent_channels detection fails because Flux 2 VAE fixture uses a different key suffix than "latents" | Low | Medium | The fixture builder uses `tensors["latents"] = torch.randn(1, _LATENT_CHANNELS, 8, 8)` — same key as ZiT VAE. If the fixture were different, the fallback pattern `key.endswith("latents")` would need adjustment. Verify against the actual fixture at plan time. |
| `decode()` produces incorrect PIL image dimensions because Flux 2 VAE decoder preserves spatial resolution differently | Low | Low | The Flux 2 VaeModel topology is identical to ZiTVaeModel: mid-block at latent_channels preserves spatial resolution, decoder blocks preserve spatial resolution through Conv2d(kernel=3, padding=1). The output spatial dimensions match the input. Test against fixture-shaped latent confirms this. |
| Mock-mode collection fails because flux2_vae.py imports torch unconditionally at module level | Low | High | The import guard pattern (`try/except ImportError`) is copied from zit_vae.py and applied identically. Every torch-dependent import is guarded. `_ModuleBase` uses conditional inheritance. `load()` and `decode()` raise RuntimeError if torch is None. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/vae/flux2_vae.py` exits 0
- [ ] `python -m py_compile worker/nodes/arch/vae/__init__.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_vae_flux2.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_flux2.py -v` exits 0 (collects all 16 tests)
- [ ] `python -m pytest worker/tests/test_arch_vae_flux2.py -v -m real_mode` exits 0 (all real_mode tests pass)
- [ ] `grep -rn "REAL_PATH_VERIFIED:" worker/nodes/arch/vae/flux2_vae.py` returns 2 lines (load + decode)
- [ ] `grep -rn "MOCK_PATH_VERIFIED:" worker/nodes/arch/vae/flux2_vae.py` returns 2 lines (load + decode)
- [ ] `grep "flux2_vae" worker/nodes/arch/vae/__init__.py` returns the import and registration lines
- [ ] `python -c "from worker.nodes.arch.vae import get_module; m=get_module('flux2'); assert m is not None and m.__name__=='worker.nodes.arch.vae.flux2_vae'"` exits 0
- [ ] `python -c "from worker.nodes.arch.vae.flux2_vae import can_handle; assert can_handle('flux2') is True and can_handle('zit_vae') is False"` exits 0
