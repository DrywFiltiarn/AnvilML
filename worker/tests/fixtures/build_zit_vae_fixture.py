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
   This combination forces the loader's metadata-fallback code path.

Tensor shapes are computed to match the interpolation formula used by
ZiTVaeModel construction:
  - encoder_channels=16, decoder_channels=32, latent_channels=4
  - encoder_block_count=2, decoder_block_count=2
  - Encoder interpolates 16→4 (decreasing), mid=4, decoder interpolates 4→16

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_vae_fixture.py
"""

from __future__ import annotations

import os

import torch
from safetensors.torch import save_file

# Resolve the fixtures directory relative to this script's location so the
# script is idempotent regardless of the working directory from which it is
# invoked.
_FIXTURES_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
)

# Hyperparameters matching the test fixture design.
ENCODER_CHANNELS = 16
DECODER_CHANNELS = 32
LATENT_CHANNELS = 4
ENCODER_BLOCK_COUNT = 2
DECODER_BLOCK_COUNT = 2


def _compute_encoder_channels() -> list[tuple[int, int]]:
    """Compute (in_ch, out_ch) for each encoder block.

    Encoder interpolates from encoder_channels down to latent_channels.
    Block 0 takes encoder_channels as input. Block N's input is block N-1's output.
    """
    blocks: list[tuple[int, int]] = []
    for i in range(ENCODER_BLOCK_COUNT):
        out_ch = int(
            ENCODER_CHANNELS
            + ((i + 1) / ENCODER_BLOCK_COUNT) * (LATENT_CHANNELS - ENCODER_CHANNELS)
        )
        if i == 0:
            in_ch = ENCODER_CHANNELS
        else:
            in_ch = blocks[-1][1]  # previous block's output
        blocks.append((in_ch, out_ch))
    return blocks


def _compute_decoder_channels() -> list[tuple[int, int]]:
    """Compute (in_ch, out_ch) for each decoder block.

    Decoder interpolates from latent_channels up to encoder_channels.
    Block 0 takes latent_channels as input. Block N's input is block N-1's output.
    """
    blocks: list[tuple[int, int]] = []
    for i in range(DECODER_BLOCK_COUNT):
        out_ch = int(
            LATENT_CHANNELS
            + ((i + 1) / DECODER_BLOCK_COUNT) * (ENCODER_CHANNELS - LATENT_CHANNELS)
        )
        if i == 0:
            in_ch = LATENT_CHANNELS
        else:
            in_ch = blocks[-1][1]  # previous block's output
        blocks.append((in_ch, out_ch))
    return blocks


def _zit_vae_tensors() -> dict[str, torch.Tensor]:
    """Return ZiT-VAE-shaped tensor dict with recognizable key prefixes.

    Tensor shapes match the ZiTVaeModel interpolation formula exactly.
    """
    tensors: dict[str, torch.Tensor] = {}

    # Encoder blocks
    enc_channels = _compute_encoder_channels()
    for i, (in_ch, out_ch) in enumerate(enc_channels):
        tensors[f"encoder.blocks.{i}.conv.weight"] = torch.randn(out_ch, in_ch, 3, 3)
        tensors[f"encoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    # Mid-block
    mid_out_ch = LATENT_CHANNELS
    tensors["mid_block.conv.weight"] = torch.randn(mid_out_ch, mid_out_ch, 3, 3)
    tensors["mid_block.norm.weight"] = torch.randn(mid_out_ch)

    # Decoder blocks
    dec_channels = _compute_decoder_channels()
    for i, (in_ch, out_ch) in enumerate(dec_channels):
        tensors[f"decoder.blocks.{i}.conv.weight"] = torch.randn(out_ch, in_ch, 3, 3)
        tensors[f"decoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    # Small latent tensor (4 channels, 8x8 spatial — standard VAE latent space)
    tensors["latents"] = torch.randn(1, LATENT_CHANNELS, 8, 8)

    return tensors


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:_zit_vae_tensors but with a prefix
    that no known VAE architecture pattern matcher can identify.
    """
    tensors = _zit_vae_tensors()
    result: dict[str, torch.Tensor] = {}

    for ckpt_key, tensor in tensors.items():
        if ckpt_key.startswith("encoder.blocks."):
            # encoder.blocks.N.* -> xyz_encoder_blockN_*
            parts = ckpt_key.split(".")
            idx = parts[2]  # block index
            suffix = ".".join(parts[3:])  # conv.weight or norm.weight
            new_key = f"xyz_encoder_block{idx}_{suffix}"
            result[new_key] = tensor
        elif ckpt_key.startswith("decoder.blocks."):
            # decoder.blocks.N.* -> xyz_decoder_blockN_*
            parts = ckpt_key.split(".")
            idx = parts[2]  # block index
            suffix = ".".join(parts[3:])
            new_key = f"xyz_decoder_block{idx}_{suffix}"
            result[new_key] = tensor
        elif ckpt_key == "mid_block.conv.weight":
            result["xyz_mid_block_conv"] = tensor
        elif ckpt_key == "mid_block.norm.weight":
            result["xyz_mid_block_norm"] = tensor
        elif ckpt_key == "latents":
            result["xyz_latents"] = tensor
        else:
            result[ckpt_key] = tensor

    return result


def build() -> None:
    """Build both ZiT VAE fixture .safetensors files."""
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny.safetensors")
    save_file(
        _zit_vae_tensors(),
        regular_path,
        metadata={"arch": "zit_vae"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture
    no_meta_path = os.path.join(_FIXTURES_DIR, "zit_vae_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
