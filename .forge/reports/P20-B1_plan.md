# Plan Report: P20-B1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P20-B1                                                      |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/arch/diffusion/zit.py: shape inference from safetensors header |
| Depends on  | P20-A1                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-07-13T12:45:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Create `worker/nodes/arch/diffusion/zit.py` implementing `_infer_hyperparams(path: str) -> dict` — the first step of the four-step loading contract defined in ANVILML_DESIGN.md §11.3. This function opens a safetensors checkpoint header-only (no tensor data loaded), reads every key via `f.keys()`, inspects `get_slice(key).get_shape()` for each key, and returns a dict of inferred hyperparameters (hidden_dim, double_block_count, single_block_count, latent_channels, latent_height, latent_width, patch_size). The function must handle both the regular ZiT fixture (with recognizable key prefixes and `arch` metadata) and the no-metadata fixture (non-recognizable keys, no `arch` key) via a fallback path. The function must raise clear errors for malformed or truncated input.

## Scope

### In Scope
- Create `worker/nodes/arch/diffusion/zit.py` with `_infer_hyperparams(path: str) -> dict`.
- Use `safetensors.safe_open(path, framework="pt")` to open the header.
- Read ALL keys via `f.keys()` — never a truncated or sliced sample (P904 regression prevention).
- Inspect `get_slice(key).get_shape()` for every key to infer:
  - `hidden_dim` from `input_proj.weight`, `time_text_emb.weight`, `c_crossattn_dim`.
  - `double_block_count` by counting keys matching `double_blocks.N.*` pattern.
  - `single_block_count` by counting keys matching `single_blocks.N.*` pattern.
  - `latent_channels` and spatial dimensions from the `latents` tensor shape.
  - `patch_size` derived from the ratio of hidden_dim to latent_channels (standard diffusion transformer convention).
- Handle the metadata-fallback path: when the `arch` metadata key is absent from the safetensors header, infer architecture string from key naming patterns.
- Raise `ValueError` with a descriptive message for malformed/truncated input (e.g., file not found, missing required keys, corrupted header).
- Create `worker/tests/test_arch_zit.py` with >=4 tests.

### Out of Scope
- `can_handle(key) -> bool` — deferred to P20-B2.
- Dispatch registration into `arch/diffusion/__init__.py`'s `_REGISTERED_MODULES` — deferred to P20-B2.
- Meta-device construction (`torch.device("meta")`) — deferred to P20-C1.
- Dtype selection per `InferenceCaps` — deferred to P20-C2.
- Key remapping, `load_state_dict`, and `.arch` attribute — deferred to P20-C3.
- `load()`, `sample()`, `compute_latent_shape()` — all deferred to later tasks in this phase.

## Existing Codebase Assessment

The codebase has Phase 10's dispatch infrastructure in place: `worker/nodes/arch/diffusion/__init__.py` defines `get_module(key)` and `_REGISTERED_MODULES` (currently empty). The fixture convention from Phase 19 (P19-D1) is established: `worker/tests/fixtures/` contains two ZiT-shaped checkpoint files — `zit_tiny.safetensors` (with `arch: "zit"` metadata and recognizable ZiT key prefixes: `input_proj`, `time_text_emb`, `double_blocks.N.*`, `single_blocks.N.*`, `output_proj`, `latents`) and `zit_tiny_no_metadata.safetensors` (non-recognizable `xyz_` prefix, no metadata key). Both fixtures use a hidden dimension of 64 and a latent shape of `(1, 4, 8, 8)`.

The established test patterns in `worker/tests/` use Google-style docstrings, `_make_ctx()` helper for NodeContext construction, and `@pytest.mark.real_mode` marker for real-mode tests. Test files import only public interfaces. No prior source exists for `zit.py` — this task creates the file from scratch.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source      | Feature flags confirmed |
|--------|-------------|-----------------|-----------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query MCP  | n/a (already pinned in base.txt) |

The `safe_open(path, framework="pt")` API exists in safetensors 0.8.0. The returned object has `.keys()` returning all tensor names and `.get_slice(key).get_shape()` returning the shape tuple. This API has been stable since at least 0.4.x and is confirmed live via pypi-query MCP. No new dependency is introduced — `safetensors` is already listed in `worker/requirements/base.txt`.

## Approach

1. **Create `worker/nodes/arch/diffusion/zit.py`** with module-level docstring explaining the ZiT architecture's role and the four-step loading contract (§11.3). Include `from __future__ import annotations` at the top.

2. **Implement `_infer_hyperparams(path: str) -> dict`** with the following logic:

   a. Open the file: `with safe_open(path, framework="pt") as f:` — this opens only the header, no tensor data loaded.

   b. Collect all keys: `keys = f.keys()` — read ALL keys, never truncated (P904 regression prevention). This is the critical difference from the P904 bug that used `list(f.keys())[:30]`.

   c. For each key, inspect its shape: `shape = f.get_slice(key).get_shape()`. Build a dict mapping key prefixes to shapes.

   d. Infer `hidden_dim` from the first key matching `input_proj.weight`, `time_text_emb.weight`, or `c_crossattn_dim`. The hidden dimension is the first dimension of `input_proj.weight` and `time_text_emb.weight`, and the sole dimension of `c_crossattn_dim`.

   e. Count double blocks: iterate all keys, count those matching `double_blocks.\d+.*`. The maximum numeric suffix gives the block count (0-indexed, so count = max_index + 1).

   f. Count single blocks: same pattern for `single_blocks.\d+.*`.

   g. Infer latent dimensions from the `latents` key: shape is `(batch, channels, height, width)`. Extract `latent_channels = shape[1]`, `latent_height = shape[2]`, `latent_width = shape[3]`.

   h. Derive `patch_size`: for ZiT diffusion transformers, `patch_size = hidden_dim // latent_channels` (this is the standard convention where each patch token projects to the hidden dimension).

   i. Infer `arch` string: check `f.metadata` for an `"arch"` key. If present, use its value. If absent (metadata-fallback path), derive from key naming patterns — if keys contain `double_blocks`, `single_blocks`, `input_proj`, `output_proj` prefixes, return `"zit"`. If no recognizable pattern is found, raise `ValueError("unknown architecture: no recognizable key patterns or arch metadata found")`.

   j. Return the dict with all inferred hyperparameters:
     ```python
     {
         "hidden_dim": hidden_dim,
         "double_block_count": double_block_count,
         "single_block_count": single_block_count,
         "latent_channels": latent_channels,
         "latent_height": latent_height,
         "latent_width": latent_width,
         "patch_size": patch_size,
         "arch": arch,
     }
     ```

3. **Create `worker/tests/test_arch_zit.py`** with the following tests (>=4):

   a. `test_infer_hyperparams_regular_fixture` — calls `_infer_hyperparams` against `zit_tiny.safetensors`, asserts the returned dict has expected keys and correct values (hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, arch="zit").

   b. `test_infer_hyperparams_no_metadata_fixture` — calls `_infer_hyperparams` against `zit_tiny_no_metadata.safetensors`, asserts the metadata-fallback path succeeds and returns arch="zit" (derived from key naming patterns), with the same shape-based hyperparameters.

   c. `test_infer_hyperparams_malformed_file_raises` — calls `_infer_hyperparams` against a non-existent path or a corrupted file, asserts `ValueError` is raised with a descriptive message.

   d. `test_infer_hyperparams_truncated_header_raises` — creates a truncated/corrupted safetensors file and asserts `ValueError` is raised.

   All tests import `_infer_hyperparams` directly from `worker.nodes.arch.diffusion.zit`. Tests use `Path(__file__).parent / "fixtures" / "zit_tiny.safetensors"` for fixture paths.

4. **Add Google-style docstrings** to `_infer_hyperparams()` with Args, Returns, and Raises sections.

5. **Add inline comments** at every decision point: the P904 prevention guard (reading all keys), the metadata-fallback path logic, the key-prefix matching for architecture inference.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `def _infer_hyperparams(path: str) -> dict` | `worker.nodes.arch.diffusion.zit` | Opens a safetensors checkpoint header-only, reads every key's shape, and returns a dict of inferred architecture hyperparameters including hidden_dim, block counts, latent dimensions, patch_size, and arch string. |

This is the sole public item. The underscore prefix follows Python convention for internal-but-testable functions. `can_handle()` and `load()` are not in scope.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/zit.py` | New file; `_infer_hyperparams()` function implementing step 1 of §11.3 loading contract. |
| CREATE | `worker/tests/test_arch_zit.py` | New test file; >=4 tests for `_infer_hyperparams()` against both fixture variants and error cases. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_zit.py` | `test_infer_hyperparams_regular_fixture` | `_infer_hyperparams()` against `zit_tiny.safetensors` returns correct hyperparameter dict: hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, arch="zit" | `python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_infer_hyperparams_no_metadata_fixture` | `_infer_hyperparams()` against `zit_tiny_no_metadata.safetensors` succeeds via metadata-fallback path, returning arch="zit" derived from key naming patterns | `python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_infer_hyperparams_malformed_file_raises` | `_infer_hyperparams()` raises `ValueError` for a non-existent file path | `python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_malformed_file_raises -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_infer_hyperparams_truncated_header_raises` | `_infer_hyperparams()` raises `ValueError` for a truncated/corrupted safetensors file | `python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the established naming convention (`test_arch_zit.py` mirrors `test_arch_dispatch.py`, `test_nodes_loader.py`) and is automatically picked up by `pytest worker/tests/`. The test does not import `torch` at module level (it only uses `safetensors` for header inspection), so it collects in both mock-mode and real-mode CI jobs without warning.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `safetensors` library is platform-neutral for header-only operations. Path handling uses Python's `pathlib.Path`, which abstracts platform-specific separators. No `os.path` or hardcoded path separators are used.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The fixture's tensor shapes may not contain enough variety to exercise all inference paths (e.g., only 1 double block and 1 single block). A future real model with 12+ blocks would follow a different pattern. | Low | Medium | The inference formula is based on key-prefix counting (not hardcoded block counts), so it generalizes. The test fixture's small size exercises the code paths; the formula itself is not fixture-specific — it iterates all keys. |
| `safe_open(path, framework="pt")` may raise a platform-specific error for non-existent files (e.g., `FileNotFoundError` vs `OSError`). | Low | Low | Catch the specific exception type and re-raise as `ValueError` with a descriptive message, consistent with Python error handling conventions. |
| The metadata-fallback path may not correctly identify ZiT architecture from the `xyz_` prefixed keys in the no-metadata fixture, since those keys deliberately lack recognizable prefixes. | Medium | High | The no-metadata fixture includes `xyz_c_crossattn_dim`, `xyz_double_block_img_attn`, `xyz_single_block_linear`, `xyz_output_proj` — the suffixes (`c_crossattn_dim`, `output_proj`) are recognizable. The fallback should match on suffix patterns, not just prefixes. If this proves insufficient, the fixture builder may need a small adjustment (add one recognizable suffix key). |
| The patch_size derivation formula (`hidden_dim // latent_channels`) may not match the actual ZiT model's convention. | Low | Medium | This formula is the standard diffusion transformer convention. If the ACT agent discovers it's wrong by inspecting the real model, it will be corrected in P20-C1 (construction) where the actual model architecture is examined. For the fixture, hidden_dim=64, latent_channels=4 gives patch_size=16, which is structurally valid. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with >=4 tests collected and passing
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0 (syntax check before test run)
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_zit.py` exits 0 (syntax check before test run)
