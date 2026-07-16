#!/usr/bin/env python3
"""Build the ZiT VAE fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the VAE model-loading contract end-to-end:

1. zit_vae_tiny.safetensors — ZiT-VAE-shaped tensor keys with
   ``arch: "zit_vae"`` metadata in the safetensors header. Recognizable key
   prefixes (``encoder.blocks.N.conv.weight``, ``decoder.blocks.N.conv.weight``,
   ``mid_block.conv.weight``, ``latents``) let the loader identify the VAE
   architecture from key naming conventions alone.

2. zit_vae_tiny_no_metadata.safetensors — same structural tensor shapes but
   with non-recognizable ``xyz_`` key prefix and no ``arch`` metadata key.
   This combination forces the loader's metadata-fallback code path,
   exercising the exact regression path that the v3
   ``st.metadata`` vs ``st.metadata()`` bug lived in.

Tensor shapes are structurally valid for a VAE's shape-inference formula:
encoder/decoder convolutional blocks with normalization, a middle/bottleneck
block, and a small latent tensor (4 channels, 8x8 spatial — standard VAE
latent space). Channel counts are small (8–16) and well under the 10 MB
per-file budget.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_vae_fixture.py
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
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny.safetensors")
    save_file(
        _zit_vae_tensors(),
        regular_path,
        metadata={"arch": "zit_vae"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture — non-recognizable keys, no arch key in header
    no_meta_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
