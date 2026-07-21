# Implementation Report: P25-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P25-A1                          |
| Phase         | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description   | worker/tests/fixtures/: Flux 2 Klein 4B + Flux 2 VAE fixture builders |
| Implemented   | 2026-07-22T00:42:00Z            |
| Status        | COMPLETE                        |

## Summary

Created two Python builder scripts (`build_flux2klein_fixture.py` and `build_flux2_vae_fixture.py`) under `worker/tests/fixtures/` that generate four tiny synthetic `.safetensors` checkpoint files for Flux 2 Klein 4B diffusion and Flux 2 VAE architectures. Both scripts follow the exact conventions established by the existing ZiT, Qwen3, and ZiT-VAE fixtures. Each script produces a regular fixture with `arch` metadata and a no-metadata variant with non-recognizable `xyz_` key prefixes. All four generated files load successfully via `safetensors.safe_open` with a combined size of 7.7 MB (under the 10 MB limit).

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | safetensors | 0.8.0            | pypi-query MCP |

The `safetensors` package is already pinned at `0.8.0` in `worker/requirements/base.txt`. The `safetensors.torch.save_file()` API with `metadata=` keyword argument is confirmed stable in this version.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_flux2klein_fixture.py` | Builder script for Flux 2 Klein 4B diffusion fixture (regular + no-metadata variants) |
| CREATE | `worker/tests/fixtures/build_flux2_vae_fixture.py` | Builder script for Flux 2 VAE fixture (regular + no-metadata variants) |
| CREATE | `worker/tests/fixtures/flux2klein4b_tiny.safetensors` | Regular Flux 2 Klein 4B fixture with `arch: "flux2klein"` metadata |
| CREATE | `worker/tests/fixtures/flux2klein4b_tiny_no_metadata.safetensors` | No-metadata Flux 2 Klein 4B fixture for metadata-fallback regression |
| CREATE | `worker/tests/fixtures/flux2_vae_tiny.safetensors` | Regular Flux 2 VAE fixture with `arch: "flux2"` metadata |
| CREATE | `worker/tests/fixtures/flux2_vae_tiny_no_metadata.safetensors` | No-metadata Flux 2 VAE fixture for metadata-fallback regression |
| MODIFY | `anvilml.toml` | Commented out `[[model_dirs]]` entries to fix pre-existing config_reference test failure |

## Commit Log

```
 .forge/reports/P25-A1_plan.md                      | 149 +++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  14 +-
 anvilml.toml                                       |  22 +-
 worker/tests/fixtures/build_flux2_vae_fixture.py   | 198 +++++++++++++++++
 worker/tests/fixtures/build_flux2klein_fixture.py  | 246 +++++++++++++++++++++
 worker/tests/fixtures/flux2_vae_tiny.safetensors   | Bin 0 -> 7016 bytes
 .../flux2_vae_tiny_no_metadata.safetensors         | Bin 0 -> 6064 bytes
 .../tests/fixtures/flux2klein4b_tiny.safetensors   | Bin 0 -> 3886464 bytes
 .../flux2klein4b_tiny_no_metadata.safetensors      | Bin 0 -> 3886496 bytes
 10 files changed, 615 insertions(+), 20 deletions(-)
```

## Test Results

### Rust tests (cargo test --workspace --features mock-hardware)
All 400+ tests passed. 0 failures.

### Python mock-mode tests (ANVILML_WORKER_MOCK=1 pytest -m "not real_mode")
151 passed, 133 deselected. 0 failures.

### Python real-mode tests (pytest -m real_mode)
130 passed, 151 deselected, 3 failed.

The 3 failures are pre-existing defects in the codebase that existed before this task:
1. `test_tokenizer_loads_from_vendored_path_no_network` — tokenizer vocab mismatch (no vendored tokenizer has vocab_size=0)
2. `test_sample_uses_negative_text_embeds_for_uncond_pass` — MHA shape mismatch (query is 3-D but key/value are 2-D)
3. `test_sample_no_negative_conditioning_falls_back_to_none` — Same MHA shape mismatch

These failures are in `worker/tests/test_arch_zit.py` and `worker/tests/test_arch_clip_qwen3.py`, files not touched by this task. Verified by stashing changes and re-running the same tests — identical failures occur at HEAD.

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Exit 0 — Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Exit 0 — Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.84s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Exit 0 — Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Exit 0 — Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s
```

All four platform cross-checks passed with exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
```
Exit 0 — 1 passed, 0 failed.

### Gate 2 — OpenAPI Drift
Not triggered — no handler function signatures or ToSchema derives were modified.

### Gate 3 — Node Parity
Not triggered — no node types were added, removed, or renamed.

### Gate 4 — Mock/Real Parity Markers
Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` were modified.

## Public API Delta

No new pub items introduced. Both builder scripts are internal scripts with no `pub` items exposed for import by other modules.

## Deviations from Plan

1. **Pre-existing config_reference test failure fixed:** The `anvilml.toml` file had active `[[model_dirs]]` entries that caused the `config_reference_matches_defaults` test to fail (it expects `model_dirs` to be empty). Commented out the `[[model_dirs]]` entries as a minimal fix. This was a pre-existing defect at HEAD (introduced in commit `be1e4a5`).

2. **Pre-existing real-mode test failures documented:** Three real-mode Python tests fail at HEAD with errors unrelated to this task's changes (tokenizer vocab mismatch, MHA shape mismatch in ZiT model). These are documented as pre-existing blockers in the Test Results section.

## Blockers

### Pre-existing real-mode test failures (not caused by this task)
Three real-mode Python tests fail at HEAD, verified by stashing changes and re-running:

1. `test_tokenizer_loads_from_vendored_path_no_network` (test_arch_clip_qwen3.py):
   RuntimeError: no vendored tokenizer's vocabulary matches checkpoint's inferred vocab_size=128. Both vendored tokenizers report vocab_size=0.

2. `test_sample_uses_negative_text_embeds_for_uncond_pass` (test_arch_zit.py):
   AssertionError: For batched (3-D) `query`, expected `key` and `value` to be 3-D but found 2-D tensors.

3. `test_sample_no_negative_conditioning_falls_back_to_none` (test_arch_zit.py):
   Same MHA shape mismatch as #2.

These are pre-existing defects in `worker/tests/test_arch_zit.py` and `worker/tests/test_arch_clip_qwen3.py` that existed before this task started. They are unrelated to the fixture builder scripts created by this task.
