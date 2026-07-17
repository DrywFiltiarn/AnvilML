#!/usr/bin/env python3
"""Build the ZiT VAE FP8 fixture checkpoint file.

Generates a tiny synthetic .safetensors file with FP8 (float8_e4m3fn)
dtype tensors, used by real-mode tests to exercise the FP8 branch of
the dtype selection precedence (ANVILML_DESIGN.md §11.5).

The fixture has the same structural tensor shapes as zit_vae_tiny.safetensors
but with FP8 dtype, enabling tests that verify fp8 is selected when
caps.fp8=True and the checkpoint native dtype is FP8.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_vae_fp8_fixture.py
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


def _zit_vae_fp8_tensors() -> dict[str, torch.Tensor]:
    """Return ZiT-VAE-shaped tensor dict with FP8 dtype.

    Creates float32 tensors first, then converts to float8_e4m3fn
    because torch.randn() does not support float8 on CPU builds.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values in FP8.
    """
    return {
        # Encoder blocks — convolutional downsampling
        "encoder.blocks.0.conv.weight": torch.randn(16, 8, 3, 3, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "encoder.blocks.0.norm.weight": torch.randn(16, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "encoder.blocks.1.conv.weight": torch.randn(32, 16, 3, 3, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "encoder.blocks.1.norm.weight": torch.randn(32, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Decoder blocks — convolutional upsampling
        "decoder.blocks.0.conv.weight": torch.randn(32, 16, 3, 3, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "decoder.blocks.0.norm.weight": torch.randn(32, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "decoder.blocks.1.conv.weight": torch.randn(16, 8, 3, 3, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "decoder.blocks.1.norm.weight": torch.randn(16, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Middle/bottleneck block
        "mid_block.conv.weight": torch.randn(32, 32, 3, 3, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        "mid_block.norm.weight": torch.randn(32, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Small latent tensor (4 channels, 8×8 spatial — standard VAE latent space)
        "latents": torch.randn(1, 4, 8, 8, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
    }


def build() -> None:
    """Build the ZiT VAE FP8 fixture .safetensors file.

    Writes:
        - ``zit_vae_tiny_fp8.safetensors`` with ``arch: "zit_vae"`` metadata and
          FP8 (float8_e4m3fn) tensor dtype.

    The file is written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    fp8_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny_fp8.safetensors")
    save_file(
        _zit_vae_fp8_tensors(),
        fp8_path,
        metadata={"arch": "zit_vae"},
    )
    print(f"Written: {fp8_path}")


if __name__ == "__main__":
    build()
