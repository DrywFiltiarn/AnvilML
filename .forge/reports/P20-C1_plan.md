# Plan Report: P20-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P20-C1                                            |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/arch/diffusion/zit.py: meta-device construction |
| Depends on  | P20-B2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-13T17:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend `worker/nodes/arch/diffusion/zit.py` with the `load()` function implementing step 2 of the four-step loading contract (ANVILML_DESIGN.md §11.3): construct the target `nn.Module` on `torch.device("meta")` using only the hyperparameters returned by `_infer_hyperparams()` from P20-B1. No `config.json`, no network access, no real memory allocation. The constructed module is returned with its `.arch` attribute set to `"zit"`. This step eliminates the ~15 GB construction crash that P904 experienced by deferring parameter memory allocation until materialization (a later task). Acceptance: ≥3 new tests in `test_arch_zit.py` confirming meta-device construction succeeds and zero real memory is allocated; `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with ≥10 total tests.

## Scope

### In Scope
- Add `ZiTModel(nn.Module)` class in `zit.py` that constructs the ZiT diffusion transformer architecture from diffusers'/transformers' layer/block classes, configured with hyperparameters from `_infer_hyperparams()` (hidden_dim, double_block_count, single_block_count, latent_channels, latent_height, latent_width, patch_size).
- Add `load(path: str) -> ZiTModel` function in `zit.py` that: calls `_infer_hyperparams(path)` to get hyperparameters, constructs `ZiTModel` on `torch.device("meta")`, sets `module.arch = "zit"`, and returns the module.
- Add dual-mode parity markers (`REAL_PATH_VERIFIED:` / `MOCK_PATH_VERIFIED:`) next to the `load()` function per ANVILML_DESIGN.md §10.6.
- Add ≥3 new tests in `worker/tests/test_arch_zit.py`: meta-device construction succeeds, parameters confirmed on meta device (zero real memory), construction against no-metadata fixture variant succeeds, invalid hyperparameters raise ValueError.

### Out of Scope
- Dtype selection (deferred to P20-C2, which genuinely covers "Extend zit.py's load() with dtype selection, the scope P20-C1 deferred, per §11.5's fixed precedence").
- Materialization via `to_empty()` and weight loading via `load_state_dict(..., assign=True)` (deferred to P20-C3).
- Key remapping table construction (deferred to P20-C3).
- `sample()` and `compute_latent_shape()` methods on ZiTModel (reserved for a later counterpart phase).

## Existing Codebase Assessment

**What already exists:** `zit.py` (276 lines) already contains `_infer_hyperparams()` and `_infer_hyperparams_inner()` from P20-B1, which opens the safetensors header, reads ALL keys, and returns a dict of hyperparameters (hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=16, arch="zit"). The `can_handle(key)` function and `ARCH` constant exist from P20-B2. The dispatch mechanism in `arch/diffusion/__init__.py` already registers `zit` as the first entry in `_REGISTERED_MODULES`.

**Established patterns:** The test file `test_arch_zit.py` (157 lines, 7 tests) follows a clear pattern: fixture-based tests assert specific hyperparameter values against the tiny fixture; error-path tests verify ValueError is raised for invalid inputs. Tests use `Path(__file__).parent / "fixtures"` for fixture resolution. Docstrings are Google-style with Args/Returns/Raises sections. The `_infer_hyperparams()` function wraps `safe_open` in try/except to convert platform-specific errors into descriptive ValueError messages.

**Gap between design doc and source:** The design doc specifies `load(model_id, caps, device)` as the full method signature (§10.4), but this task only implements step 2 (meta-device construction). The `load()` function in this task will have a simplified signature `load(path: str) -> ZiTModel` that returns the meta-constructed module. The `caps` (InferenceCaps) and `device` parameters will be added by P20-C2 and P20-C3 respectively. This incremental signature is intentional — each step builds on the previous one without breaking the function's contract.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source       | Feature flags confirmed |
|--------|-----------|-----------------|------------------|------------------------|
| python | diffusers | 0.39.0          | pypi-query MCP   | n/a                    |
| python | torch     | (project venv)  | project lockfile | n/a                    |
| python | safetensors| 0.8.0          | project lockfile | n/a                    |

No new dependencies are introduced. This task uses only `torch` (already in the project's venv via GPU-specific requirements files) and `torch.nn.Module` (stdlib). `diffusers` is already pinned at 0.39.0 in `worker/requirements/base.txt`. The layer/block classes used as building blocks are from `torch.nn` (Linear, LayerNorm, Conv2d, MultiheadAttention) — diffusers' own internal layer classes may be used if available, but the construction relies on `torch.nn` primitives which are the universal building blocks.

## Approach

### Step 1: Add ZiTModel(nn.Module) class

Create a new `ZiTModel(nn.Module)` class in `zit.py` that represents the ZiT diffusion transformer architecture. The class constructor accepts the hyperparameter dict from `_infer_hyperparams()` and builds the module structure using `torch.nn` layer classes:

```python
class ZiTModel(nn.Module):
    """ZiT diffusion transformer model constructed from layer-level building blocks.

    This class assembles the ZiT architecture using torch.nn primitives
    (Linear, LayerNorm, Conv2d) that mirror the tensor shapes found in
    the checkpoint. It is constructed on torch.device("meta") so that
    no real GPU/CPU memory is allocated during construction — this is
    the step that prevents P904's ~15GB-on-construction crash.

    The architecture consists of:
    - input_proj: latent space → hidden dimension projection
    - time_text_emb: time-step + text embedding projection
    - double_blocks: list of cross-attention blocks (image + text attention)
    - single_blocks: list of linear transformation blocks
    - output_proj: hidden dimension → latent space projection
    """

    def __init__(self, hyperparams: dict[str, Any]) -> None:
        """Construct the ZiT model on the meta device.

        Args:
            hyperparams: Dict from _infer_hyperparams() containing
                hidden_dim, double_block_count, single_block_count,
                latent_channels, latent_height, latent_width, patch_size.
        """
        super().__init__()
        # Extract hyperparameters
        hidden_dim = hyperparams["hidden_dim"]
        double_block_count = hyperparams["double_block_count"]
        single_block_count = hyperparams["single_block_count"]
        latent_channels = hyperparams["latent_channels"]
        latent_height = hyperparams["latent_height"]
        latent_width = hyperparams["latent_width"]
        patch_size = hyperparams["patch_size"]

        # Input projection: (latent_channels * patch_size^2) → hidden_dim
        # The latent tensor is reshaped to (batch, latent_channels*patch_size^2, height*height)
        # before projection into the hidden dimension.
        latent_dim = latent_channels * patch_size * patch_size
        self.input_proj = nn.Linear(latent_dim, hidden_dim)

        # Time-step + text embedding projection (fixed-size embedding)
        self.time_text_emb = nn.Linear(hidden_dim, hidden_dim)

        # Double blocks with cross-attention sub-layers
        # Each double block has: image attention (self-attention on image tokens)
        # and text attention (cross-attention conditioned on text tokens).
        self.double_blocks = nn.ModuleList([
            nn.ModuleDict({
                "img_attn": AttentionBlock(hidden_dim),
                "txt_attn": CrossAttentionBlock(hidden_dim, hidden_dim),
                "norm1": nn.LayerNorm(hidden_dim),
                "norm2": nn.LayerNorm(hidden_dim),
                "ff": FeedForwardBlock(hidden_dim),
            })
            for _ in range(double_block_count)
        ])

        # Single blocks with linear transformation
        # Each single block is a simplified linear transformation block.
        self.single_blocks = nn.ModuleList([
            nn.ModuleDict({
                "linear1": nn.Linear(hidden_dim, hidden_dim * 4),
                "linear2": nn.Linear(hidden_dim * 4, hidden_dim),
                "norm": nn.LayerNorm(hidden_dim),
            })
            for _ in range(single_block_count)
        ])

        # Output projection: hidden_dim → (latent_channels * patch_size^2)
        self.output_proj = nn.Linear(hidden_dim, latent_dim)

        # Architecture identifier — set after construction so downstream
        # code can identify this model's family.
        self.arch: str = "zit"
```

**Rationale for using `torch.nn` primitives vs. diffusers internal classes:** The ZiT architecture is a custom diffusion transformer that does not have a dedicated class in diffusers 0.39.0. Using `torch.nn` primitives (Linear, LayerNorm, etc.) is the correct approach because:
1. These are the fundamental building blocks that diffusers' own model classes use internally.
2. The checkpoint keys map directly to these primitive layer weights (e.g., `input_proj.weight` → `self.input_proj.weight`).
3. This avoids depending on diffusers' internal class structure, which is not a stable public API.

### Step 2: Add load() function

Add the `load()` function that implements step 2 of the loading contract:

```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_real
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_mock
def load(path: str) -> ZiTModel:
    """Construct the ZiT model on meta-device (step 2 of loading contract).

    Opens the checkpoint header, infers hyperparameters from tensor shapes,
    constructs the target nn.Module on torch.device("meta"), and returns it
    with .arch set to "zit". No real memory is allocated.

    Args:
        path: Filesystem path to a ZiT-format safetensors checkpoint file.

    Returns:
        A ZiTModel instance constructed on torch.device("meta") with
        .arch = "zit".

    Raises:
        ValueError: If the checkpoint cannot be opened or hyperparameters
            cannot be inferred (delegated to _infer_hyperparams).
    """
    # Step 1 (from P20-B1): infer hyperparameters from checkpoint header.
    hyperparams = _infer_hyperparams(path)

    # Step 2 (this task): construct on meta-device.
    # Using torch.device("meta") means no real memory is allocated for
    # parameters — the module structure exists but tensors have shape
    # metadata only. This prevents the ~15GB crash from P904.
    with torch.device("meta"):
        model = ZiTModel(hyperparams)

    # Set architecture identifier so downstream dispatch (Sampler, VaeDecode)
    # can route correctly without re-deriving the architecture.
    model.arch = ARCH

    return model
```

**Rationale for `torch.device("meta")` context manager:** The `with torch.device("meta"):` context ensures all `nn.Module` submodules are constructed with meta tensors (shape-only, zero memory). This is the standard PyTorch idiom for meta-device construction and is more reliable than passing `device="meta"` to individual layer constructors.

**Rationale for ARCH constant:** Using `ARCH` (the module-level `"zit"` constant) instead of hardcoding the string `"zit"` ensures consistency with `can_handle()` and the dispatch mechanism. If the architecture identifier ever changes, only `ARCH` needs updating.

### Step 3: Add parity markers

Add the dual-mode parity markers next to the `load()` function (included in Step 2 above). The `REAL_PATH_VERIFIED` marker points to the real-mode test that constructs against a fixture checkpoint. The `MOCK_PATH_VERIFIED` marker points to the mock-mode test that verifies construction works when `ANVILML_WORKER_MOCK=1`.

### Step 4: Add tests in test_arch_zit.py

Add four new tests to `test_arch_zit.py`:

1. **`test_load_meta_construction_real`** — Calls `load()` against the regular fixture, verifies the returned module is a `ZiTModel` instance, has `.arch == "zit"`, and all parameters are on `torch.device("meta")`.

2. **`test_load_meta_device_zero_real_memory`** — Calls `load()` and verifies that no real memory was allocated by checking that all parameters have zero numel when inspected via `sum(p.numel() for p in model.parameters())` (meta tensors report their shape but allocate nothing).

3. **`test_load_meta_construction_no_metadata_variant`** — Calls `load()` against the no-metadata fixture variant, verifies it succeeds via the metadata-fallback path and returns a valid `ZiTModel`.

4. **`test_load_raises_invalid_hyperparams`** — Calls `load()` with a path that causes `_infer_hyperparams()` to raise `ValueError`, verifying the error propagates correctly.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `class ZiTModel` | `worker.nodes.arch.diffusion.zit` | `class ZiTModel(nn.Module): def __init__(self, hyperparams: dict[str, Any]) -> None` |
| `def load` | `worker.nodes.arch.diffusion.zit` | `def load(path: str) -> ZiTModel` |
| `ZiTModel.arch` | `worker.nodes.arch.diffusion.zit.ZiTModel` | `arch: str` — Architecture identifier set to `"zit"` after construction |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `ZiTModel(nn.Module)` class, `load()` function, and dual-mode parity markers |
| MODIFY | `worker/tests/test_arch_zit.py` | Add ≥4 new tests for meta-device construction |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `test_arch_zit.py` | `test_load_meta_construction_real` | `load()` against regular fixture returns ZiTModel with .arch="zit", parameters on meta device | `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_construction_real -v` |
| `test_arch_zit.py` | `test_load_meta_device_zero_real_memory` | All parameters of meta-constructed module have zero real memory (numel check on meta tensors) | `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_device_zero_real_memory -v` |
| `test_arch_zit.py` | `test_load_meta_construction_no_metadata_variant` | `load()` against no-metadata fixture succeeds via metadata-fallback path | `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_construction_no_metadata_variant -v` |
| `test_arch_zit.py` | `test_load_raises_invalid_hyperparams` | `load()` raises ValueError when _infer_hyperparams() fails | `python -m pytest worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams -v` |

## CI Impact

No CI changes required. The new tests run as part of the existing `worker-linux-mock` and `worker-linux-real` CI jobs (Step 8 and Step 9 in ENVIRONMENT.md §6). The tests use `torch` at module level only within the test body (imported inside the test function to avoid unconditional torch import at collection time in mock-mode CI), so they are compatible with the mock-mode CI job that does not install torch.

## Platform Considerations

None identified. The `torch.device("meta")` context manager and `torch.nn.Module` are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `_infer_hyperparams()` returns unexpected values for the fixture that cause `ZiTModel.__init__` to construct layers with incompatible shapes, leading to a runtime error during meta-device construction. | Low | Medium | The fixture tensor shapes (hidden_dim=64, 1 double block, 1 single block) are structurally valid and match the inference formula. The `ZiTModel.__init__` derives all layer dimensions from the hyperparams dict, so any shape inconsistency would be caught by a simple assertion or by the test itself. |
| `torch.device("meta")` context manager behavior differs between torch versions in the project venv, causing parameters to not be on meta device as expected. | Low | High | The project uses Python 3.12.x with a pinned venv. `torch.device("meta")` has been stable since torch 1.8+. If a version mismatch occurs, the test `test_load_meta_device_zero_real_memory` will fail immediately, flagging the issue. |
| The no-metadata fixture variant's key patterns don't match the ZiT fallback detection in `_infer_hyperparams()`, causing `ValueError` before meta-device construction even begins. | Low | Low | The fixture builder (`build_zit_fixture.py`) is designed to produce keys that trigger the fallback path: `xyz_double_block_img_attn`, `xyz_double_block_txt_attn`, `xyz_single_block_linear`, `xyz_output_proj` — these contain "double_block", "single_block", and "output_proj" which are the exact patterns checked in `_infer_hyperparams_inner()`. |
| The test file exceeds 500 lines (the review threshold from ARCHITECTURE.md §11). | Low | Low | Current file is 157 lines. Adding 4 tests (~120 lines) brings total to ~280 lines, well under the 500-line threshold. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_construction_real -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_device_zero_real_memory -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_meta_construction_no_metadata_variant -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with ≥10 total tests (7 existing + ≥3 new)
