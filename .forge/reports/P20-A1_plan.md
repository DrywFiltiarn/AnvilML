# Plan Report: P20-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-A1                                      |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/tests/fixtures/: ZiT diffusion fixture safetensors builder |
| Depends on  | P19-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/tests/fixtures/build_zit_fixture.py`, a self-contained Python script that generates two tiny synthetic `.safetensors` checkpoint files — `zit_tiny.safetensors` (with `arch: "zit"` metadata and recognizable ZiT-style key prefixes) and `zit_tiny_no_metadata.safetensors` (with non-recognizable `xyz_` key prefix and no `arch` metadata key) — following the fixture conventions documented in `worker/tests/fixtures/README.md` and `ANVILML_DESIGN.md §17.5`. The files serve as the testing foundation for all subsequent Phase 20 tasks that build `zit.py`'s shape-inference, construction, and loading logic.

## Scope

### In Scope
- Create `worker/tests/fixtures/build_zit_fixture.py` — a Python script using `safetensors.torch.save_file` and `torch.randn` to produce deterministic tiny synthetic `.safetensors` files.
- The script writes two files:
  - `zit_tiny.safetensors` — ZiT-shaped tensor keys with `arch: "zit"` in safetensors metadata header.
  - `zit_tiny_no_metadata.safetensors` — same structural tensor shapes but with `xyz_` key prefix (non-recognizable) and no `arch` metadata key, exercising the metadata-fallback regression path.
- Tensor shapes are structurally valid for a diffusion transformer's shape-inference formula (small number of transformer layers, small hidden dim, small patch size) — NOT a miniaturized copy of real model shapes.
- Both files must load successfully via `safetensors.safe_open`.
- Combined file size under 10 MB.
- Run the script to produce both `.safetensors` files under `worker/tests/fixtures/`.

### Out of Scope
- None. This task implements its full scope. `defers_to (from JSON): []`.

## Existing Codebase Assessment

**What exists:** The `worker/tests/fixtures/` directory exists with only `README.md` documenting the fixture conventions (Phase 19's P19-D1 deliverable). No `.safetensors` files exist yet. The diffusion architecture dispatcher at `worker/nodes/arch/diffusion/__init__.py` exists but has an empty `_REGISTERED_MODULES` list — no concrete arch modules are wired in yet. `LoadModel`'s real branch currently raises `NotImplementedError` (the deliberate Phase 19 placeholder). The Python worker venv has `safetensors==0.8.0` and `torch==2.12.1+cpu` available.

**Established patterns:** Python test files use Google-style docstrings, pytest conventions, and the `real_mode`/`not real_mode` marker system. Test files in `worker/tests/` follow the naming convention `test_<module>.py`. The fixture README specifies that builder scripts accept no arguments, are idempotent, and use structurally valid but small shapes.

**Gap between design doc and current source:** The design doc (§11.3) describes the four-step loading contract but does not specify the exact key names or shape-inference algorithm for ZiT — those are left to be worked out at implementation time. This is intentional and expected for this phase. The fixture shapes need only be structurally valid for whatever formula `zit.py` will implement in P20-B1.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                  |
| python | torch       | 2.12.1+cpu      | pypi-query MCP | n/a                  |

Both packages are already installed in `worker/.venv` as declared in `worker/requirements/base.txt`. No new dependencies are introduced.

## Approach

1. **Create `worker/tests/fixtures/build_zit_fixture.py`** with a single `build()` function and `if __name__ == "__main__"` entry point. The script uses only `torch` and `safetensors.torch.save_file` — no subprocesses, no network calls, no optional imports.

2. **Define the regular fixture tensors** with recognizable ZiT-style key prefixes and structurally valid shapes:
   - `input_proj.weight`: `(768, 768)` — input projection from latent space to hidden dimension
   - `time_text_emb.weight`: `(768, 768)` — time-step + text embedding projection
   - `c_crossattn_dim`: `(768,)` — cross-attention dimension (1-D tensor, common in diffusion transformers)
   - `double_blocks.0.img_attn.proj.weight`: `(768, 768)` — first double block self-attention projection
   - `double_blocks.0.txt_attn.proj.weight`: `(768, 768)` — first double block cross-attention projection
   - `single_blocks.0.linear1.weight`: `(768, 768)` — first single block linear projection
   - `output_proj.weight`: `(768, 768)` — output projection back to latent space
   - `latents`: `(1, 4, 8, 8)` — a small latent tensor for shape inference on the channel/spatial dimensions
   - Metadata: `{"arch": "zit"}` passed to `save_file()`

   Rationale: These shapes use a consistent hidden dimension (768) across all tensors, which is the canonical hidden dim for the base ZiT model. The 4-channel, 8×8 latent shape matches the standard diffusion latent space (4 channels from VAE encoding, 8×8 for a downscaled 1024×1024 image). The key prefixes (`input_proj`, `time_text_emb`, `double_blocks.*`, `single_blocks.*`, `output_proj`) match the known ZiT diffusion transformer architecture pattern, enabling shape inference to detect the architecture family from key naming conventions.

3. **Define the no-metadata fixture tensors** with non-recognizable `xyz_` key prefix:
   - Same tensor shapes as above, but keys prefixed with `xyz_` (e.g., `xyz_input_proj.weight` → `xyz_random_tensor_data`, `xyz_time_text_emb.weight` → `xyz_another_tensor`)
   - No `metadata` argument to `save_file()` — the header will contain no `arch` key
   - This combination (non-recognizable prefix + no `arch` metadata) forces the loader's metadata-fallback code path

   Rationale: The `xyz_` prefix ensures no known architecture pattern matcher can identify this checkpoint from key names alone. Combined with the absent `arch` metadata key, this exercises the exact code path that the v3 `st.metadata` vs `st.metadata()` bug lived in.

4. **Run the script** to produce both `.safetensors` files:
   ```bash
   worker/.venv/bin/python worker/tests/fixtures/build_zit_fixture.py
   ```
   Verify both files exist and are under 10 MB combined.

5. **Verify both files load via `safetensors.safe_open`:**
   ```bash
   worker/.venv/bin/python -c "
   from safetensors import safe_open
   for f in ['worker/tests/fixtures/zit_tiny.safetensors', 'worker/tests/fixtures/zit_tiny_no_metadata.safetensors']:
       with safe_open(f, framework='pt') as s:
           keys = list(s.keys())
           meta = s.metadata()
           print(f'{f}: {len(keys)} keys, metadata={meta}')
   "
   ```

## Public API Surface

None. This task creates a builder script (not a library module) and data files. The script has no public API — it is invoked as a standalone process via `__main__`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_zit_fixture.py` | Builder script generating both fixture `.safetensors` files |
| CREATE | `worker/tests/fixtures/zit_tiny.safetensors` | ZiT-shaped fixture with `arch: "zit"` metadata |
| CREATE | `worker/tests/fixtures/zit_tiny_no_metadata.safetensors` | ZiT-shaped fixture with non-recognizable keys, no `arch` metadata |

## Tests

This task does not create new test files — the acceptance criterion is the successful execution of the builder script and verification that both files load. The builder script's correctness is validated by:

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (manual verification) | build_zit_fixture.py exits 0 | Script runs without error | `worker/.venv/bin/python worker/tests/fixtures/build_zit_fixture.py` exits 0 |
| (manual verification) | both files exist and under 10 MB combined | File sizes are within budget | `du -b worker/tests/fixtures/zit_tiny.safetensors worker/tests/fixtures/zit_tiny_no_metadata.safetensors | awk '{s+=$1} END {print s " bytes"}'` outputs a value < 10485760 |
| (manual verification) | both files load via safetensors.safe_open | Files are valid safetensors format | `worker/.venv/bin/python -c "from safetensors import safe_open; [safe_open(f, framework='pt') for f in ['worker/tests/fixtures/zit_tiny.safetensors', 'worker/tests/fixtures/zit_tiny_no_metadata.safetensors']]"` exits 0 |

## CI Impact

No CI changes required. The fixture files are committed and loaded by real-mode tests in subsequent tasks. No new test modules, file types, or CI job configuration changes are introduced by this task.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The builder script uses only `torch` and `safetensors.torch` which are cross-platform. No `#[cfg]` guards, path separators, or line-ending handling are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.randn` produces non-deterministic values across runs, potentially causing git diff noise on committed `.safetensors` files | Low | Medium | `torch.randn` is seeded by the global random state which is deterministic within a single process. Since the script runs once and the output is committed, subsequent re-runes will produce identical files as long as no other code mutates the global RNG state. The script does not call `torch.manual_seed()` because it only needs reproducibility within a single invocation, not across separate invocations. |
| Tensor shapes chosen may not align with the shape-inference formula `zit.py` will implement in P20-B1 | Medium | Medium | The shapes are deliberately structurally valid (consistent hidden dim, proper channel/spatial latent shape, transformer block tensors) rather than exact copies of real model shapes. If `zit.py`'s inference formula needs specific key patterns or shape relationships, the fixture can be modified in a follow-up task without architectural changes. The README explicitly states shapes should be "structurally valid" not "dimensionally accurate." |
| `safetensors` version 0.8.0 metadata API differs from what the loader code expects | Low | High | The README documents the v3 `st.metadata` vs `st.metadata()` bug which is a method-call issue, not a version difference. Verified via MCP that `safe_open().metadata()` returns `dict | None` in safetensors 0.8.0, matching the expected API shape. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python worker/tests/fixtures/build_zit_fixture.py` exits 0
- [ ] `du -b worker/tests/fixtures/zit_tiny.safetensors worker/tests/fixtures/zit_tiny_no_metadata.safetensors | awk '{s+=$1} END {print s}'` outputs a value less than 10485760 (10 MB)
- [ ] `worker/.venv/bin/python -c "from safetensors import safe_open; [safe_open(f, framework='pt') for f in ['worker/tests/fixtures/zit_tiny.safetensors', 'worker/tests/fixtures/zit_tiny_no_metadata.safetensors']]"` exits 0
- [ ] `zit_tiny.safetensors` contains `arch: "zit"` in its metadata header (verified via `safe_open(...).metadata() == {"arch": "zit"}`)
- [ ] `zit_tiny_no_metadata.safetensors` has no `arch` key in its metadata header (verified via `safe_open(...).metadata() is None` or `{"arch" not in safe_open(...).metadata()}`)
- [ ] `zit_tiny.safetensors` keys contain recognizable ZiT-style prefixes (`input_proj`, `time_text_emb`, `double_blocks`, `single_blocks`, `output_proj`)
- [ ] `zit_tiny_no_metadata.safetensors` keys use non-recognizable `xyz_` prefix
