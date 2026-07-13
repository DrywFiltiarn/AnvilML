#!/usr/bin/env python3
"""Build the ZiT FP8 fixture checkpoint file.

Generates a tiny synthetic .safetensors file with FP8 (float8_e4m3fn)
dtype tensors, used by real-mode tests to exercise the FP8 branch of
the dtype selection precedence (ANVILML_DESIGN.md §11.5).

The fixture has the same structural tensor shapes as zit_tiny.safetensors
but with FP8 dtype, enabling tests that verify fp8 is selected when
caps.fp8=True and the checkpoint native dtype is FP8.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_fp8_fixture.py
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


def _zit_fp8_tensors() -> dict[str, torch.Tensor]:
    """Return the ZiT-shaped tensor dict with FP8 dtype.

    Creates float32 tensors first, then converts to float8_e4m3fn
    because torch.randn() does not support float8 on CPU builds.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values in FP8.
    """
    return {
        # Input projection: latent space → hidden dimension
        "input_proj.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Time-step + text embedding projection
        "time_text_emb.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Cross-attention dimension (1-D tensor, common in diffusion transformers)
        "c_crossattn_dim": torch.randn(64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # First double-block self-attention projection
        "double_blocks.0.img_attn.proj.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # First double-block cross-attention projection
        "double_blocks.0.txt_attn.proj.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # First single-block linear projection
        "single_blocks.0.linear1.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Output projection back to latent space
        "output_proj.weight": torch.randn(64, 64, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
        # Small latent tensor for shape inference on channel/spatial dimensions
        # (4 channels from VAE encoding, 8x8 for a downscaled 1024x1024 image)
        "latents": torch.randn(1, 4, 8, 8, dtype=torch.float32).to(
            torch.float8_e4m3fn
        ),
    }


def build() -> None:
    """Build the ZiT FP8 fixture .safetensors file.

    Writes:
        - ``zit_tiny_fp8.safetensors`` with ``arch: "zit"`` metadata and
          FP8 (float8_e4m3fn) tensor dtype.

    The file is written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    fp8_path = os.path.join(_FIXTURES_DIR, "zit_tiny_fp8.safetensors")
    save_file(
        _zit_fp8_tensors(),
        fp8_path,
        metadata={"arch": "zit"},
    )
    print(f"Written: {fp8_path}")


if __name__ == "__main__":
    build()
