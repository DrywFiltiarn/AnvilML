#!/usr/bin/env python3
"""Build the ZiT diffusion fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the model-loading contract end-to-end:

1. zit_tiny.safetensors — ZiT-shaped tensor keys with ``arch: "zit"``
   metadata in the safetensors header. Recognizable key prefixes
   (``input_proj``, ``time_text_emb``, ``double_blocks``, ``single_blocks``,
   ``output_proj``) let the loader identify the architecture family from
   key naming conventions alone.

2. zit_tiny_no_metadata.safetensors — same structural tensor shapes but with
   non-recognizable ``xyz_`` key prefix and no ``arch`` metadata key. This
   combination forces the loader's metadata-fallback code path, exercising
   the exact regression path that the v3 ``st.metadata`` vs ``st.metadata()``
   bug lived in.

Tensor shapes are structurally valid for a diffusion transformer's
shape-inference formula (consistent hidden dim of 64 — a structurally
valid but small dimension that stays well under the 10 MB per-file
budget while preserving the same key-prefix patterns and shape
relationships as the canonical 768-dim base model), 4-channel 8x8 latent
matching the standard VAE-encoded latent space.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_fixture.py
"""

from __future__ import annotations

import os
import sys

import torch
from safetensors.torch import save_file

# Resolve the fixtures directory relative to this script's location so the
# script is idempotent regardless of the working directory from which it is
# invoked.
_FIXTURES_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
)


def _zit_tensors() -> dict[str, torch.Tensor]:
    """Return the ZiT-shaped tensor dict for the regular fixture.

    Keys use recognizable ZiT diffusion transformer prefixes so the loader
    can identify the architecture from key naming conventions alone. All
    tensors share a consistent hidden dimension of 768 (the canonical
    base-model hidden dim).

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        # Input projection: latent space → hidden dimension
        "input_proj.weight": torch.randn(64, 64),
        # Time-step + text embedding projection
        "time_text_emb.weight": torch.randn(64, 64),
        # Cross-attention dimension (1-D tensor, common in diffusion transformers)
        "c_crossattn_dim": torch.randn(64),
        # First double-block self-attention projection
        "double_blocks.0.img_attn.proj.weight": torch.randn(64, 64),
        # First double-block cross-attention projection
        "double_blocks.0.txt_attn.proj.weight": torch.randn(64, 64),
        # First single-block linear projection
        "single_blocks.0.linear1.weight": torch.randn(64, 64),
        # Output projection back to latent space
        "output_proj.weight": torch.randn(64, 64),
        # Small latent tensor for shape inference on channel/spatial dimensions
        # (4 channels from VAE encoding, 8x8 for a downscaled 1024x1024 image)
        "latents": torch.randn(1, 4, 8, 8),
    }


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:`_zit_tensors` but with a prefix that
    no known architecture pattern matcher can identify. Combined with the
    absent ``arch`` metadata key, this exercises the metadata-fallback
    code path.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        "xyz_random_tensor_data": torch.randn(64, 64),
        "xyz_another_tensor": torch.randn(64, 64),
        "xyz_c_crossattn_dim": torch.randn(64),
        "xyz_double_block_img_attn": torch.randn(64, 64),
        "xyz_double_block_txt_attn": torch.randn(64, 64),
        "xyz_single_block_linear": torch.randn(64, 64),
        "xyz_output_proj": torch.randn(64, 64),
        "xyz_latents": torch.randn(1, 4, 8, 8),
    }


def build() -> None:
    """Build both ZiT fixture .safetensors files.

    Writes:
        - ``zit_tiny.safetensors`` with ``arch: "zit"`` metadata.
        - ``zit_tiny_no_metadata.safetensors`` with no metadata, exercising
          the metadata-fallback regression path.

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "zit_tiny.safetensors")
    save_file(
        _zit_tensors(),
        regular_path,
        metadata={"arch": "zit"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture — non-recognizable keys, no arch key in header
    no_meta_path = os.path.join(_FIXTURES_DIR, "zit_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
