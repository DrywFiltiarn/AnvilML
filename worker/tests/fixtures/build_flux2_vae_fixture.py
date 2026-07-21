#!/usr/bin/env python3
"""Build the Flux 2 VAE fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the Flux 2 VAE model-loading contract end-to-end:

1. flux2_vae_tiny.safetensors -- a representative subset of the actual
   Flux 2 VAE checkpoint keys (encoder blocks, mid block, decoder blocks)
   with structurally valid tensor shapes. Carries ``arch: "flux2"``
   metadata in the safetensors header.

2. flux2_vae_tiny_no_metadata.safetensors -- a deliberately minimal,
   non-recognizable-key-prefix fixture that only needs to exercise
   ``_infer_hyperparams()``'s metadata-fallback path. Built independently
   of ``_flux2_vae_tensors()`` with ``xyz_``-prefixed keys and no
   ``arch`` metadata in the header.

Since ``flux2_vae.py`` does not yet exist, these use hand-crafted tensors
with Flux 2 VAE's actual checkpoint key pattern and structurally valid
shapes. The dimension values are small enough to keep file size under
10 MB but large enough to exercise the shape-inference formula.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_flux2_vae_fixture.py
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

# Insert the repo root (three levels up from this file's directory:
# worker/tests/fixtures/ -> repo root) onto sys.path so ``import worker...``
# resolves regardless of invocation style.
_REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
)
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

# Hyperparameters for the Flux 2 VAE fixture. These are structurally valid
# but tiny (encoder/decoder_channels=8, latent_channels=4) so the fixture
# stays under 10 MB. The key patterns match Flux 2 VAE's actual checkpoint
# naming convention so that ``_infer_hyperparams()`` can parse them once
# the real architecture module exists.
_ENCODER_CHANNELS = 8
_DECODER_CHANNELS = 8
_LATENT_CHANNELS = 4
_ENCODER_BLOCK_COUNT = 2
_DECODER_BLOCK_COUNT = 2


def _flux2_vae_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with Flux 2 VAE checkpoint key patterns.

    Hand-crafted tensors matching the actual Flux 2 VAE checkpoint key
    structure (encoder.blocks, mid_block, decoder.blocks). Uses
    structurally valid shapes but tiny dimensions to keep file size under
    10 MB.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    tensors: dict[str, torch.Tensor] = {}

    # Encoder blocks — channel interpolation from encoder_channels to
    # latent_channels across blocks (same formula as ZiT VAE).
    for i in range(_ENCODER_BLOCK_COUNT):
        if i == 0:
            in_ch = _ENCODER_CHANNELS
        else:
            in_ch = _LATENT_CHANNELS  # previous block's output
        out_ch = int(
            _ENCODER_CHANNELS
            + ((i + 1) / _ENCODER_BLOCK_COUNT) * (
                _LATENT_CHANNELS - _ENCODER_CHANNELS
            )
        )
        if out_ch < 1:
            out_ch = 1
        tensors[f"encoder.blocks.{i}.conv.weight"] = torch.randn(
            out_ch, in_ch, 3, 3
        )
        tensors[f"encoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    # Mid block — operates at latent resolution
    tensors["mid_block.conv.weight"] = torch.randn(
        _LATENT_CHANNELS, _LATENT_CHANNELS, 3, 3
    )
    tensors["mid_block.norm.weight"] = torch.randn(_LATENT_CHANNELS)

    # Decoder blocks — channel interpolation from latent_channels back to
    # decoder_channels across blocks (same formula as ZiT VAE).
    for i in range(_DECODER_BLOCK_COUNT):
        if i == 0:
            in_ch = _LATENT_CHANNELS
        else:
            in_ch = _LATENT_CHANNELS  # previous block's output
        out_ch = int(
            _LATENT_CHANNELS
            + ((i + 1) / _DECODER_BLOCK_COUNT) * (
                _ENCODER_CHANNELS - _LATENT_CHANNELS
            )
        )
        if out_ch < 1:
            out_ch = 1
        tensors[f"decoder.blocks.{i}.conv.weight"] = torch.randn(
            out_ch, in_ch, 3, 3
        )
        tensors[f"decoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    # Marker tensor for _infer_hyperparams()'s shape-inference contract
    # (not a real Flux 2 VAE parameter).
    tensors["latents"] = torch.randn(1, _LATENT_CHANNELS, 8, 8)
    return tensors


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:`_flux2_vae_tensors` but with a prefix
    that no known VAE architecture pattern matcher can identify. Combined
    with the absent ``arch`` metadata key, this exercises the
    metadata-fallback code path.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        "xyz_encoder_block0_conv": torch.randn(
            _LATENT_CHANNELS, _ENCODER_CHANNELS, 3, 3
        ),
        "xyz_encoder_block0_norm": torch.randn(_LATENT_CHANNELS),
        "xyz_encoder_block1_conv": torch.randn(
            _LATENT_CHANNELS, _LATENT_CHANNELS, 3, 3
        ),
        "xyz_encoder_block1_norm": torch.randn(_LATENT_CHANNELS),
        "xyz_mid_block_conv": torch.randn(
            _LATENT_CHANNELS, _LATENT_CHANNELS, 3, 3
        ),
        "xyz_mid_block_norm": torch.randn(_LATENT_CHANNELS),
        "xyz_decoder_block0_conv": torch.randn(
            _LATENT_CHANNELS, _LATENT_CHANNELS, 3, 3
        ),
        "xyz_decoder_block0_norm": torch.randn(_LATENT_CHANNELS),
        "xyz_decoder_block1_conv": torch.randn(
            _ENCODER_CHANNELS, _LATENT_CHANNELS, 3, 3
        ),
        "xyz_decoder_block1_norm": torch.randn(_ENCODER_CHANNELS),
        "xyz_latents": torch.randn(1, _LATENT_CHANNELS, 8, 8),
    }


def build() -> None:
    """Build both Flux 2 VAE fixture .safetensors files.

    Writes:
        - ``flux2_vae_tiny.safetensors`` with ``arch: "flux2"`` metadata.
        - ``flux2_vae_tiny_no_metadata.safetensors`` with no metadata,
          exercising the metadata-fallback regression path.

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "flux2_vae_tiny.safetensors")
    save_file(
        _flux2_vae_tensors(),
        regular_path,
        metadata={"arch": "flux2"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture — non-recognizable keys, no arch key in header
    no_meta_path = os.path.join(
        _FIXTURES_DIR, "flux2_vae_tiny_no_metadata.safetensors"
    )
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
