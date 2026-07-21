# Plan Report: P25-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P25-A1                                            |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE       |
| Description | worker/tests/fixtures/: Flux 2 Klein 4B + Flux 2 VAE fixture builders |
| Depends on  | P24-F1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-21T23:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create two Python builder scripts (`build_flux2klein_fixture.py` and `build_flux2_vae_fixture.py`) under `worker/tests/fixtures/` that generate four tiny synthetic `.safetensors` checkpoint files — `flux2klein4b_tiny.safetensors`, `flux2klein4b_tiny_no_metadata.safetensors`, `flux2_vae_tiny.safetensors`, and `flux2_vae_tiny_no_metadata.safetensors` — following the exact conventions established by the existing ZiT, Qwen3, and ZiT-VAE fixtures. Each script produces a regular fixture with `arch` metadata and a no-metadata variant with non-recognizable key prefixes, exercising the metadata-fallback regression path per ANVILML_DESIGN.md §17.5.

## Scope

### In Scope
- Create `worker/tests/fixtures/build_flux2klein_fixture.py` — builds `flux2klein4b_tiny.safetensors` (with `arch: "flux2klein"` metadata) and `flux2klein4b_tiny_no_metadata.safetensors` (no metadata, non-recognizable `xyz_` key prefixes).
- Create `worker/tests/fixtures/build_flux2_vae_fixture.py` — builds `flux2_vae_tiny.safetensors` (with `arch: "flux2"` metadata) and `flux2_vae_tiny_no_metadata.safetensors` (no metadata, non-recognizable `xyz_` key prefixes).
- Commit all four generated `.safetensors` files to the repository.
- Both builder scripts exit 0 when run; all four files total under 10 MB combined; all load successfully via `safetensors.safe_open`.

### Out of Scope
None. This task's `defers_to` is `[]` (empty). No functionality is deferred.

## Existing Codebase Assessment

The `worker/tests/fixtures/` directory contains five builder scripts and ten `.safetensors` files for the ZiT diffusion model, Qwen3 text encoder, and ZiT VAE families. Each family follows an identical convention:

1. **Regular fixture** — carries `arch` metadata in the safetensors header. Built by constructing the real model class on the CPU device (with `torch.manual_seed(42)` for reproducibility) and dumping its full `state_dict()`, with a "latents" marker tensor added for shape-inference contract.
2. **No-metadata fixture** — uses `xyz_`-prefixed keys that no architecture pattern matcher can identify, with no `arch` key in the header, exercising the metadata-fallback regression path. For ZiT, this is built independently of the full state_dict to avoid bias-key issues.

The builder scripts share a common structure: resolve `_FIXTURES_DIR` relative to the script, prepend the repo root to `sys.path`, import the target model class from `worker.nodes.arch.*`, and call a `build()` function that writes both files via `safetensors.torch.save_file`.

**Gap:** The Flux 2 Klein (`flux2klein.py`) and Flux 2 VAE (`flux2_vae.py`) arch modules do not yet exist — they will be created in later Phase 25 tasks (P25-B1 through P25-E1). Therefore, unlike the ZiT fixtures which import the actual model class to get a complete state_dict, the Flux 2 fixtures must use hand-crafted tensor dicts with shapes that are structurally valid for Flux 2's architecture. The key patterns must match Flux 2's actual checkpoint naming convention so that when the future `_infer_hyperparams()` is written, it can parse these keys correctly.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source   | Feature flags confirmed |
|--------|-------------|-----------------|--------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query   | n/a                    |

The `safetensors.torch.save_file()` API is stable and confirmed via the MCP lookup. The `metadata` keyword argument (for adding `arch` to the safetensors header) is available in all supported versions.

## Approach

1. **Create `worker/tests/fixtures/build_flux2klein_fixture.py`.**
   - Follow the identical structure to `build_zit_fixture.py`: resolve `_FIXTURES_DIR`, prepend repo root to `sys.path`, define `_HYPERPARAMS`, define `_flux2klein_tensors()` and `_no_metadata_tensors()`, and a `build()` function.
   - Since `flux2klein.py` does not exist yet, use hand-crafted tensors with Flux 2 Klein's actual checkpoint key pattern:
     - `time_text_embed.timestep_embedder.*` — timestep projection (shape: `(hidden_dim, hidden_dim)`)
     - `time_text_embed.context_embedder` — context embedding (shape: `(hidden_dim, context_dim)`)
     - `double_blocks.0.img_mod.lin` — image modulation (shape: `(hidden_dim * 6,)`)
     - `double_blocks.0.txt_mod.lin` — text modulation (shape: `(hidden_dim * 6,)`)
     - `double_blocks.0.img_attn.qkv` — image QKV (shape: `(hidden_dim, hidden_dim * 3)`)
     - `double_blocks.0.img_attn.norm` — image attention norm (shape: `(hidden_dim,)`)
     - `double_blocks.0.img_attn.proj` — image attention projection (shape: `(hidden_dim, hidden_dim)`)
     - `double_blocks.0.txt_attn.qkv` — text QKV (shape: `(context_dim, hidden_dim * 3)`)
     - `double_blocks.0.txt_attn.norm` — text attention norm (shape: `(hidden_dim,)`)
     - `double_blocks.0.txt_attn.proj` — text attention projection (shape: `(context_dim, hidden_dim)`)
     - `double_blocks.0.img_mlp.0` — image MLP up (shape: `(hidden_dim, hidden_dim * 4)`)
     - `double_blocks.0.img_mlp.1` — image MLP down (shape: `(hidden_dim * 4, hidden_dim)`)
     - `double_blocks.0.txt_mlp.0` — text MLP up (shape: `(hidden_dim, hidden_dim * 4)`)
     - `double_blocks.0.txt_mlp.1` — text MLP down (shape: `(hidden_dim * 4, hidden_dim)`)
     - `single_blocks.0.linear1` — single block linear1 (shape: `(hidden_dim, hidden_dim * 4)`)
     - `single_blocks.0.linear2` — single block linear2 (shape: `(hidden_dim * 4, hidden_dim)`)
     - `single_blocks.0.norm` — single block norm (shape: `(hidden_dim,)`)
     - `final_layer.linear` — final output projection (shape: `(hidden_dim, patch_size * patch_size * out_channels)`)
     - `final_layer.adaLN_modulation.1` — final modulation (shape: `(hidden_dim * 2,)`)
     - `latents` — marker tensor (shape: `(1, latent_channels, 8, 8)`)
   - Use `hidden_dim = 128`, `context_dim = 768`, `latent_channels = 4`, `patch_size = 2`, `out_channels = 4`, `depth = 1`, `single_depth = 1` — structurally valid but tiny.
   - `_no_metadata_tensors()` copies the same shapes with `xyz_`-prefixed keys (e.g., `xyz_time_text_embed_timestep_embedder`), no `arch` metadata.

2. **Create `worker/tests/fixtures/build_flux2_vae_fixture.py`.**
   - Follow the identical structure to `build_zit_vae_fixture.py`: resolve `_FIXTURES_DIR`, prepend repo root to `sys.path`, define `_HYPERPARAMS`, define `_flux2_vae_tensors()` and `_no_metadata_tensors()`, and a `build()` function.
   - Since `flux2_vae.py` does not exist yet, use hand-crafted tensors with Flux 2 VAE's actual checkpoint key pattern (encoder/decoder with conv + norm blocks, similar to ZiT VAE but with Flux 2 naming):
     - `encoder.blocks.0.conv.weight` — encoder block 0 conv (shape: `(out_ch, in_ch, 3, 3)`)
     - `encoder.blocks.0.norm.weight` — encoder block 0 norm (shape: `(out_ch,)`)
     - `encoder.blocks.1.conv.weight` — encoder block 1 conv
     - `encoder.blocks.1.norm.weight` — encoder block 1 norm
     - `mid_block.conv.weight` — mid block conv (shape: `(latent_channels, latent_channels, 3, 3)`)
     - `mid_block.norm.weight` — mid block norm (shape: `(latent_channels,)`)
     - `decoder.blocks.0.conv.weight` — decoder block 0 conv
     - `decoder.blocks.0.norm.weight` — decoder block 0 norm
     - `decoder.blocks.1.conv.weight` — decoder block 1 conv
     - `decoder.blocks.1.norm.weight` — decoder block 1 norm
     - `latents` — marker tensor (shape: `(1, latent_channels, 8, 8)`)
   - Use `encoder_channels = 8`, `decoder_channels = 8`, `latent_channels = 4`, `encoder_block_count = 2`, `decoder_block_count = 2` — structurally valid but tiny.
   - `_no_metadata_tensors()` copies the same shapes with `xyz_`-prefixed keys, no `arch` metadata.

3. **Run both builder scripts** to generate the four `.safetensors` files.
   - Run: `python worker/tests/fixtures/build_flux2klein_fixture.py`
   - Run: `python worker/tests/fixtures/build_flux2_vae_fixture.py`
   - Verify both exit 0.

4. **Verify generated files:**
   - Check all four files exist under `worker/tests/fixtures/`.
   - Verify combined size is under 10 MB.
   - Verify each file loads successfully via `safetensors.safe_open`.

5. **Stage all files** with `git add -A`.

## Public API Surface

None. These are internal builder scripts, not importable library code. No `pub` items or public API surface is introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_flux2klein_fixture.py` | Builder script for Flux 2 Klein 4B diffusion fixture (regular + no-metadata variants) |
| CREATE | `worker/tests/fixtures/build_flux2_vae_fixture.py` | Builder script for Flux 2 VAE fixture (regular + no-metadata variants) |
| CREATE | `worker/tests/fixtures/flux2klein4b_tiny.safetensors` | Regular Flux 2 Klein 4B fixture with `arch: "flux2klein"` metadata |
| CREATE | `worker/tests/fixtures/flux2klein4b_tiny_no_metadata.safetensors` | No-metadata Flux 2 Klein 4B fixture for metadata-fallback regression |
| CREATE | `worker/tests/fixtures/flux2_vae_tiny.safetensors` | Regular Flux 2 VAE fixture with `arch: "flux2"` metadata |
| CREATE | `worker/tests/fixtures/flux2_vae_tiny_no_metadata.safetensors` | No-metadata Flux 2 VAE fixture for metadata-fallback regression |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (acceptance only) | flux2klein builder exits 0 | `build_flux2klein_fixture.py` runs successfully and produces both files | `python worker/tests/fixtures/build_flux2klein_fixture.py` exits 0 |
| (acceptance only) | flux2_vae builder exits 0 | `build_flux2_vae_fixture.py` runs successfully and produces both files | `python worker/tests/fixtures/build_flux2_vae_fixture.py` exits 0 |
| (acceptance only) | all 4 files load via safe_open | All four `.safetensors` files are valid and loadable | `python -c "from safetensors.safe_open import safe_open; files=['flux2klein4b_tiny.safetensors','flux2klein4b_tiny_no_metadata.safetensors','flux2_vae_tiny.safetensors','flux2_vae_tiny_no_metadata.safetensors']; [safe_open(f'worker/tests/fixtures/{f}', framework='pt') for f in files]"` exits 0 |
| (acceptance only) | combined size under 10MB | All four fixture files combined are under 10 MB | `du -cb worker/tests/fixtures/flux2klein4b_tiny*.safetensors worker/tests/fixtures/flux2_vae_tiny*.safetensors | tail -1 | awk '{exit ($1 > 10*1024*1024) ? 1 : 0}'` exits 0 |

## CI Impact

No CI changes required. The new files are test fixtures and builder scripts under `worker/tests/fixtures/`, which are already covered by the existing `worker-linux-mock` and `worker-linux-real` CI jobs. No new test modules, no new file types, no new CI gates.

## Platform Considerations

None identified. The builder scripts use only `torch` and `safetensors.torch`, which are cross-platform. The `.safetensors` format is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Hand-crafted tensor shapes may not match the actual `_infer_hyperparams()` formula that will be written in P25-B1, causing shape-inference failures when future tasks load these fixtures. | Medium | High | Use well-known Flux 2 checkpoint key patterns (double_blocks, single_blocks, time_text_embed, etc.) and structurally valid shapes. The future `_infer_hyperparams()` must be designed to handle these standard key patterns. If a mismatch is found during P25-B1, the fixtures can be regenerated. |
| Combined file size exceeds 10 MB limit. | Low | Medium | Use small dimension values (hidden_dim=128, encoder_channels=8, etc.) which produce tensors of only a few KB each. The ZiT fixtures (~1 MB total for 4 files) demonstrate that this approach easily stays under 10 MB. |
| `safetensors.torch.save_file` metadata argument behavior differs from what the loader's `_infer_hyperparams()` expects when reading the `arch` key. | Low | Medium | Follow the exact same pattern used by existing fixtures: `save_file(tensors, path, metadata={"arch": "family"})`. The loader reads `arch` from the safetensors header via `safe_open(...).metadata()`. |

## Acceptance Criteria

- [ ] `python worker/tests/fixtures/build_flux2klein_fixture.py` exits 0
- [ ] `python worker/tests/fixtures/build_flux2_vae_fixture.py` exits 0
- [ ] `python -c "from safetensors.safe_open import safe_open; files=['flux2klein4b_tiny.safetensors','flux2klein4b_tiny_no_metadata.safetensors','flux2_vae_tiny.safetensors','flux2_vae_tiny_no_metadata.safetensors']; [safe_open(f'worker/tests/fixtures/{f}', framework='pt') for f in files]"` exits 0
- [ ] `du -cb worker/tests/fixtures/flux2klein4b_tiny*.safetensors worker/tests/fixtures/flux2_vae_tiny*.safetensors | tail -1 | awk '{exit ($1 > 10*1024*1024) ? 1 : 0}'` exits 0
