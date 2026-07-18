#!/usr/bin/env python3
"""Build the ZiT VAE fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the VAE model-loading contract end-to-end:

1. zit_vae_tiny.safetensors -- a COMPLETE checkpoint: every real parameter
   in ``ZiTVaeModel``'s actual ``state_dict()`` (P902 retrofit; previously
   this fixture only included ``.conv.weight``/``.norm.weight`` keys,
   leaving all 10 bias parameters -- every conv and norm bias across the
   encoder, mid-block, and decoder -- unpopulated by the checkpoint.
   ``load()`` already had a defensive zero-init step for exactly this gap
   (added before this retrofit, scoped to ``.bias``-suffixed parameters
   only, which happened to be sufficient for this fixture's specific gap
   -- see zit_vae.py's ``load()`` for the P902 change widening that step
   to cover all parameters, matching zit.py/qwen3.py, since a
   bias-only scope wouldn't have protected against a future *weight*
   gap the way zit.py's and qwen3.py's identical gaps needed). Built by
   directly constructing ``ZiTVaeModel(_HYPERPARAMS)`` (real device, not
   meta) and dumping its full ``state_dict()`` -- this guarantees the
   fixture always exactly matches the real architecture, keyed
   identically, with no hand-maintained key list to fall out of sync as
   the model evolves. Carries ``arch: "zit_vae"`` metadata in the
   safetensors header.

2. zit_vae_tiny_no_metadata.safetensors -- unchanged by this retrofit: a
   deliberately minimal, non-recognizable-key-prefix fixture that only
   needs to exercise ``_infer_hyperparams()``'s metadata-fallback path and
   ``load()``'s ability to construct without crashing -- not a full
   forward pass -- so full-state-dict completeness doesn't apply to it.
   Built independently of ``_zit_vae_tensors()`` (its own minimal
   weight-only tensor set, matching this fixture's pre-P902 shape) rather
   than reusing it, since a full state_dict would introduce bias keys
   this function's rename logic doesn't handle and isn't meant to.

``_HYPERPARAMS`` reproduces exactly what ``_infer_hyperparams()`` inferred
from the pre-P902 fixture (encoder_channels=16, decoder_channels=10,
latent_channels=4) -- chosen so every existing test that depends on these
inferred values is unaffected by this retrofit; only the fixture's
*completeness*, not its *shape*, changed. ``encoder_block_count``/
``decoder_block_count`` are hardcoded to 2 inside ``ZiTVaeModel.__init__``
itself (not part of the hyperparams dict), so this fixture is a fixed
2-encoder-block, 2-decoder-block VAE regardless.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_vae_fixture.py
"""

from __future__ import annotations

import os
import re
import sys

import torch
from safetensors.torch import save_file

# Resolve the fixtures directory relative to this script's location so the
# script is idempotent regardless of the working directory from which it is
# invoked.
_FIXTURES_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
)

# Insert the repo root onto sys.path so `import worker...` resolves
# regardless of invocation style -- see build_zit_fixture.py's identical
# comment for why this is needed.
_REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
)
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from worker.nodes.arch.vae.zit_vae import ZiTVaeModel  # noqa: E402

# Hyperparameters for the real ZiTVaeModel construction below. These
# exactly reproduce what _infer_hyperparams() derived from the pre-P902
# fixture -- see this module's docstring for why that equivalence matters.
_HYPERPARAMS: dict[str, int] = {
    "encoder_channels": 16,
    "decoder_channels": 10,
    "latent_channels": 4,
}

# Retained for _no_metadata_tensors(), which builds its own independent,
# minimal, weight-only tensor set rather than reusing the full state_dict
# -- see this module's docstring.
ENCODER_CHANNELS = _HYPERPARAMS["encoder_channels"]
DECODER_CHANNELS = 32  # unused by _HYPERPARAMS; retained for the minimal
# no-metadata builder's channel-interpolation formula below, matching its
# pre-P902 behavior exactly.
LATENT_CHANNELS = _HYPERPARAMS["latent_channels"]
ENCODER_BLOCK_COUNT = 2
DECODER_BLOCK_COUNT = 2


def _zit_vae_tensors() -> dict[str, torch.Tensor]:
    """Return a COMPLETE tensor dict covering every real ZiTVaeModel parameter.

    Constructs ``ZiTVaeModel(_HYPERPARAMS)`` directly on the real (CPU)
    device -- not the meta device ``load()`` uses for memory efficiency --
    so every ``nn.Conv2d``/``nn.GroupNorm`` submodule runs its normal
    PyTorch default initialization (``reset_parameters()``), producing
    finite, properly-scaled values for every one of the model's real
    parameters, including the biases the pre-P902 fixture omitted.
    ``torch.manual_seed()`` is set immediately before construction so the
    fixture's content is reproducible across runs of this script.

    A "latents" marker tensor is added on top of the real state_dict for
    _infer_hyperparams()'s shape-inference contract (not a real
    ZiTVaeModel parameter).

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values: every key
        in ``ZiTVaeModel(_HYPERPARAMS).state_dict()``, plus "latents".
    """
    torch.manual_seed(42)
    model = ZiTVaeModel(_HYPERPARAMS)
    tensors: dict[str, torch.Tensor] = {}
    for key, value in model.state_dict().items():
        # The model's internal ModuleDict naming is "block_N" (a valid
        # Python identifier, since ModuleDict keys can't contain dots).
        # Real-world checkpoints -- and _infer_hyperparams()'s regex
        # (r"^encoder\.blocks\.\d+\.conv\.weight$") -- use "blocks.N"
        # instead. _build_key_remapping() converts "blocks.N" back to
        # "block_N" at load time; using "block_N" directly here would
        # silently defeat that remapping and break shape inference (this
        # was caught by this retrofit's own regression test below, which
        # asserts _infer_hyperparams() returns nonzero encoder_channels/
        # decoder_channels against the rebuilt fixture).
        checkpoint_key = re.sub(r"\.block_(\d+)\.", r".blocks.\1.", key)
        tensors[checkpoint_key] = value.detach().clone()
    tensors["latents"] = torch.randn(1, _HYPERPARAMS["latent_channels"], 8, 8)
    return tensors


def _compute_encoder_channels() -> list[tuple[int, int]]:
    """Compute (in_ch, out_ch) for each encoder block.

    Used only by _no_metadata_tensors()'s independent minimal tensor
    builder -- see this module's docstring for why that function doesn't
    reuse _zit_vae_tensors()'s full state_dict.
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

    Used only by _no_metadata_tensors()'s independent minimal tensor
    builder -- see this module's docstring.
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


def _minimal_vae_tensors_for_no_metadata() -> dict[str, torch.Tensor]:
    """Return the pre-P902 minimal (weight-only, no-bias) VAE tensor dict.

    This is _no_metadata_tensors()'s source data -- kept independent of
    the full-state_dict _zit_vae_tensors() above (see module docstring).
    Reproduces the exact pre-P902 shapes and key set.
    """
    tensors: dict[str, torch.Tensor] = {}

    enc_channels = _compute_encoder_channels()
    for i, (in_ch, out_ch) in enumerate(enc_channels):
        tensors[f"encoder.blocks.{i}.conv.weight"] = torch.randn(out_ch, in_ch, 3, 3)
        tensors[f"encoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    mid_out_ch = LATENT_CHANNELS
    tensors["mid_block.conv.weight"] = torch.randn(mid_out_ch, mid_out_ch, 3, 3)
    tensors["mid_block.norm.weight"] = torch.randn(mid_out_ch)

    dec_channels = _compute_decoder_channels()
    for i, (in_ch, out_ch) in enumerate(dec_channels):
        tensors[f"decoder.blocks.{i}.conv.weight"] = torch.randn(out_ch, in_ch, 3, 3)
        tensors[f"decoder.blocks.{i}.norm.weight"] = torch.randn(out_ch)

    tensors["latents"] = torch.randn(1, LATENT_CHANNELS, 8, 8)
    return tensors


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:_zit_vae_tensors but with a prefix
    that no known VAE architecture pattern matcher can identify.
    """
    tensors = _minimal_vae_tensors_for_no_metadata()
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
