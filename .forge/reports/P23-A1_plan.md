# Plan Report: P23-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-A1                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/tests/fixtures/: ZiT VAE fixture safetensors builder |
| Depends on  | P19-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-16T16:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/tests/fixtures/build_zit_vae_fixture.py`, a builder script that generates two tiny synthetic `.safetensors` checkpoint files for the ZiT VAE architecture: (1) `zit_vae_tiny.safetensors` with structurally valid ZiT-VAE-shaped tensor keys and `arch: "zit_vae"` metadata in the header, and (2) `zit_vae_tiny_no_metadata.safetensors` with non-recognizable key prefixes and no `arch` metadata, exercising the mandatory metadata-fallback regression path. Both files must load successfully via `safetensors.safe_open` and be under 10 MB combined.

## Scope

### In Scope
- Create `worker/tests/fixtures/build_zit_vae_fixture.py` following the builder script conventions established by `build_zit_fixture.py` (Phase 20) and `build_qwen3_fixture.py` (Phase 22).
- The builder script generates two `.safetensors` files:
  - `zit_vae_tiny.safetensors` — ZiT-VAE-shaped tensor keys (`encoder.blocks.N.conv.weight`, `decoder.blocks.N.conv.weight`, `mid_block.conv.weight`, `latents`, etc.) with `arch: "zit_vae"` metadata.
  - `zit_vae_tiny_no_metadata.safetensors` — same structural shapes but with `xyz_` key prefix and no `arch` metadata key, forcing the metadata-fallback code path.
- Run the script to produce both `.safetensors` files under `worker/tests/fixtures/`.
- Acceptance: `python worker/tests/fixtures/build_zit_vae_fixture.py` exits 0, both files under 10 MB combined, both load via `safetensors.safe_open`.

### Out of Scope
None. This task's `defers_to` is `[]` (absent) — no scope may be deferred. All functionality described in the task context, including any "confirm at ACT time" language, is implemented in full by this task.

## Existing Codebase Assessment

The `worker/tests/fixtures/` directory already contains three fixture families built by Phase 20 (ZiT diffusion: `build_zit_fixture.py` + `zit_tiny.safetensors` / `zit_tiny_no_metadata.safetensors`) and Phase 22 (Qwen3 CLIP: `build_qwen3_fixture.py` + `qwen3_tiny.safetensors`). The builder script pattern is well-established: a self-contained Python script using `torch.randn()` to construct tensors, `safetensors.torch.save_file()` to write them, and `os.path.dirname(os.path.abspath(__file__))` for path resolution.

The `arch/vae/__init__.py` dispatcher exists (Phase 10, P10-B2) with zero registered modules and expects module keys like `"zit_vae"` or `"flux2_vae"`. The `zit_vae.py` arch module does not yet exist — it will be created in Phase 23 Group B.

The fixture conventions in `worker/tests/fixtures/README.md` are clear: tensor shapes must be structurally valid for the architecture's shape-inference formula, not a miniaturized copy of real model shapes; each file under 10 MB; and every family needs a metadata-fallback variant with non-recognizable keys and no `arch` metadata.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                    |
| python | torch       | (project venv)  | n/a            | n/a                    |

`safetensors==0.8.0` matches the project's `worker/requirements/base.txt`. The `save_file` function from `safetensors.torch` is the standard API — confirmed via the MCP package info which shows `from safetensors.torch import save_file, load_file` in the usage docs.

## Approach

**Step 1 — Write `worker/tests/fixtures/build_zit_vae_fixture.py`.**

Create the builder script following the established pattern from `build_zit_fixture.py`. The script must:

a) **Imports and path resolution** (lines 1–45): Same boilerplate as existing builders — `from __future__ import annotations`, `import os`, `import torch`, `from safetensors.torch import save_file`. Resolve `_FIXTURES_DIR` using `os.path.dirname(os.path.abspath(__file__))`.

b) **`_zit_vae_tensors()` function** (lines 47–85): Return a dict of ZiT-VAE-shaped tensors with recognizable key prefixes. A VAE architecture has encoder blocks, decoder blocks, and a middle/bottleneck block — each with convolutional weights and normalization weights. The tensor keys and shapes should be structurally valid for a VAE's shape-inference formula:

```python
def _zit_vae_tensors() -> dict[str, torch.Tensor]:
    """Return ZiT-VAE-shaped tensor dict with recognizable key prefixes.

    Keys follow a VAE checkpoint structure: encoder blocks, decoder blocks,
    and a middle/bottleneck block — each with convolutional weights and
    normalization scale vectors. Channel counts are small (8–16) and
    structurally valid for the VAE's shape-inference formula.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        # Encoder blocks — convolutional downsampling
        "encoder.blocks.0.conv.weight": torch.randn(16, 8, 3, 3),
        "encoder.blocks.0.norm.weight": torch.randn(16),
        "encoder.blocks.1.conv.weight": torch.randn(32, 16, 3, 3),
        "encoder.blocks.1.norm.weight": torch.randn(32),
        # Decoder blocks — convolutional upsampling
        "decoder.blocks.0.conv.weight": torch.randn(32, 16, 3, 3),
        "decoder.blocks.0.norm.weight": torch.randn(32),
        "decoder.blocks.1.conv.weight": torch.randn(16, 8, 3, 3),
        "decoder.blocks.1.norm.weight": torch.randn(16),
        # Middle/bottleneck block
        "mid_block.conv.weight": torch.randn(32, 32, 3, 3),
        "mid_block.norm.weight": torch.randn(32),
        # Small latent tensor (4 channels, 8×8 spatial — standard VAE latent space)
        "latents": torch.randn(1, 4, 8, 8),
    }
```

c) **`_no_metadata_tensors()` function** (lines 87–105): Return tensors with the same structural shapes but `xyz_` key prefix and no `arch` metadata. This mirrors the pattern from `build_zit_fixture.py`'s `_no_metadata_tensors()`:

```python
def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:`_zit_vae_tensors` but with a prefix
    that no known VAE architecture pattern matcher can identify. Combined
    with the absent ``arch`` metadata key, this exercises the metadata-
    fallback code path.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        "xyz_encoder_block0_conv": torch.randn(16, 8, 3, 3),
        "xyz_encoder_block0_norm": torch.randn(16),
        "xyz_encoder_block1_conv": torch.randn(32, 16, 3, 3),
        "xyz_encoder_block1_norm": torch.randn(32),
        "xyz_decoder_block0_conv": torch.randn(32, 16, 3, 3),
        "xyz_decoder_block0_norm": torch.randn(32),
        "xyz_decoder_block1_conv": torch.randn(16, 8, 3, 3),
        "xyz_decoder_block1_norm": torch.randn(16),
        "xyz_mid_block_conv": torch.randn(32, 32, 3, 3),
        "xyz_mid_block_norm": torch.randn(32),
        "xyz_latents": torch.randn(1, 4, 8, 8),
    }
```

d) **`build()` function** (lines 107–130): Write both files. The regular fixture gets `metadata={"arch": "zit_vae"}`. The no-metadata fixture gets no metadata argument at all:

```python
def build() -> None:
    """Build both ZiT VAE fixture .safetensors files.

    Writes:
        - ``zit_vae_tiny.safetensors`` with ``arch: "zit_vae"`` metadata.
        - ``zit_vae_tiny_no_metadata.safetensors`` with no metadata,
          exercising the metadata-fallback regression path.

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    regular_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny.safetensors")
    save_file(
        _zit_vae_tensors(),
        regular_path,
        metadata={"arch": "zit_vae"},
    )
    print(f"Written: {regular_path}")

    no_meta_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")
```

e) **Entry point** (line 132–133): `if __name__ == "__main__": build()`

**Step 2 — Run the builder script.** Execute `python worker/tests/fixtures/build_zit_vae_fixture.py` from the repo root. Verify:
- Exit code is 0
- Both `.safetensors` files are created
- Combined file size is under 10 MB

**Step 3 — Verify both files load via `safetensors.safe_open`.** Run a quick verification:
```bash
python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/zit_vae_tiny.safetensors', framework='pt'); safe_open('worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors', framework='pt'); print('OK')"
```

## Public API Surface

None. This task creates a builder script (not a library module) — no `pub` items, no public API surface. The script's `build()` function is a module-level function invoked via `if __name__ == "__main__"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_zit_vae_fixture.py` | Builder script for ZiT VAE fixture checkpoints |
| CREATE | `worker/tests/fixtures/zit_vae_tiny.safetensors` | ZiT-VAE-shaped fixture with `arch: "zit_vae"` metadata |
| CREATE | `worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors` | Metadata-fallback regression fixture (non-recognizable keys, no `arch` key) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (builder script itself) | `build_zit_vae_fixture.py` execution | Script exits 0 and produces both files | None | None (runs end-to-end) | Exit 0, two `.safetensors` files created under `worker/tests/fixtures/` | `python worker/tests/fixtures/build_zit_vae_fixture.py` exits 0 |
| (builder script itself) | `safetensors.safe_open` verification | Both files load successfully without error | Files exist from previous step | Paths to both `.safetensors` files | No exception raised, file headers parsed successfully | `python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/zit_vae_tiny.safetensors', framework='pt'); safe_open('worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors', framework='pt'); print('OK')"` |
| (builder script itself) | File size check | Combined size under 10 MB | Files exist from previous step | Paths to both `.safetensors` files | Total file size < 10,485,760 bytes | `python -c "import os; s=os.path.getsize('worker/tests/fixtures/zit_vae_tiny.safetensors')+os.path.getsize('worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors'); assert s < 10*1024*1024, f'Size {s} exceeds 10MB'; print(f'OK: {s} bytes')"` |

## CI Impact

No CI changes required. The `.safetensors` fixture files are committed and consumed by existing test infrastructure (`worker-linux-real` / `worker-windows-real` CI jobs run `pytest worker/tests -v -m real_mode`). The builder script itself is not part of any test suite — it is a one-shot code-generation tool that produces committed artifacts. Adding a new fixture file does not change any CI job's behavior; subsequent Phase 23 tasks (P23-B1 through P23-F1) will reference these fixtures in their tests.

## Platform Considerations

None identified. The builder script uses only `torch` and `safetensors.torch.save_file`, both of which are platform-neutral Python APIs. The tensor shapes and key names are pure data — no platform-specific paths, line endings, or system calls are involved. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| VAE tensor key names/shape conventions differ from what `zit_vae.py`'s `_infer_hyperparams()` will expect in Phase 23 Group B | Medium | High | The fixture shapes are deliberately structural, not dimensional — they need only be valid for a VAE's shape-inference logic (convolutional encoder/decoder with normalization), not match the real model exactly. The task's context explicitly states "structurally valid for whatever shape-inference formula zit_vae.py's load() will read, NOT a miniaturized copy of the real model's shapes." If a later task discovers the key naming convention differs, the fixture keys can be adjusted in a follow-up task (P23-B1's shape inference will define the canonical key names). |
| `torch.randn()` on CPU may not support certain dtypes used in real VAE checkpoints (e.g., float8) | Low | Low | This task only uses default float32 tensors (same as `build_zit_fixture.py`). FP8 fixtures are handled by a separate script (`build_zit_fp8_fixture.py` pattern) in a later phase. |
| Combined file size exceeds 10 MB | Very Low | Medium | The planned tensor shapes are extremely small: 10 tensors total, largest being `(32, 32, 3, 3)` float32 = 36,864 bytes ≈ 36 KB. Total estimated size is well under 500 KB — far below the 10 MB budget. |

## Acceptance Criteria

- [ ] `python worker/tests/fixtures/build_zit_vae_fixture.py` exits 0
- [ ] `test -f worker/tests/fixtures/zit_vae_tiny.safetensors && test -f worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors` — both files exist
- [ ] `python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/zit_vae_tiny.safetensors', framework='pt'); safe_open('worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors', framework='pt'); print('OK')"` exits 0
- [ ] `python -c "import os; s=os.path.getsize('worker/tests/fixtures/zit_vae_tiny.safetensors')+os.path.getsize('worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors'); assert s < 10*1024*1024, f'Size {s} exceeds 10MB'; print(f'OK: {s} bytes')"` exits 0
