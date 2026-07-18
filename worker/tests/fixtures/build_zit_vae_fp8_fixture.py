#!/usr/bin/env python3
"""Build the ZiT VAE FP8 fixture checkpoint file.

Generates a tiny synthetic .safetensors file with FP8 (float8_e4m3fn)
dtype tensors, used by real-mode tests to exercise the FP8 branch of
the dtype selection precedence (ANVILML_DESIGN.md §11.5).

P902 retrofit: like zit_vae_tiny.safetensors, this fixture previously only
included ``.conv.weight``/``.norm.weight`` keys, leaving every bias
parameter unpopulated (relying on load()'s to_empty()-without-zero gap --
now fixed, and widened from a bias-only defensive zero-init to cover all
parameters; see zit_vae.py's load()). This fixture is actively used by
test_load_dtype_fp8_caps_and_native, which only asserts every parameter's
*dtype* (not its value), so the incompleteness never surfaced as a test
failure -- but it's the same underlying gap as the fixtures that did
surface it.

This retrofit also found that the pre-P902 fixture's own hand-picked
channel sizes (encoder 8->16, decoder 16->32) didn't actually correspond
to any hyperparameters ZiTVaeModel's real channel-interpolation formula
would produce -- they were arithmetic that doesn't reproduce from
_infer_hyperparams()'s own derived values, meaning even the tensors that
existed and matched by name would only load if their hand-picked shapes
happened to agree with a differently-interpolated real model, which they
didn't consistently. Rather than debug and preserve that inconsistency,
this fixture now reuses build_zit_vae_fixture.py's exact _HYPERPARAMS
(encoder_channels=16, decoder_channels=10, latent_channels=4) -- the same,
verified-self-consistent architecture as the regular (non-fp8) VAE
fixture, differing only in dtype. No test asserts a specific channel size
for this fixture, only dtype, so this substitution is safe.

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

# Insert the repo root onto sys.path so `import worker...` resolves
# regardless of invocation style -- see build_zit_fixture.py's identical
# comment for why this is needed.
_REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
)
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from worker.nodes.arch.vae.zit_vae import ZiTVaeModel  # noqa: E402

# Same hyperparameters as build_zit_vae_fixture.py's _HYPERPARAMS -- kept
# as an independent copy (rather than importing it) so this script has no
# dependency on build_zit_vae_fixture.py's internals, only on
# ZiTVaeModel itself. See module docstring for why this fixture no longer
# uses its own distinct (and internally inconsistent) channel sizes.
_HYPERPARAMS: dict[str, int] = {
    "encoder_channels": 16,
    "decoder_channels": 10,
    "latent_channels": 4,
}


def _zit_vae_fp8_tensors() -> dict[str, torch.Tensor]:
    """Return a COMPLETE tensor dict covering every real ZiTVaeModel parameter, in FP8.

    Constructs ``ZiTVaeModel(_HYPERPARAMS)`` directly on the real (CPU)
    device in its default (float32) dtype -- torch.randn()/default
    nn.Module init don't support float8 directly -- renames keys from the
    model's internal ``block_N`` naming to the checkpoint convention
    ``blocks.N`` that ``_infer_hyperparams()``'s regex and
    ``_build_key_remapping()`` expect (see build_zit_vae_fixture.py's
    identical comment on ``_zit_vae_tensors()`` for why this renaming is
    necessary), then casts every parameter to ``float8_e4m3fn``.
    ``torch.manual_seed()`` is set immediately before construction for
    reproducibility.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values in FP8: every
        key in ``ZiTVaeModel(_HYPERPARAMS).state_dict()`` (renamed to the
        checkpoint convention), plus "latents".
    """
    import re

    torch.manual_seed(42)
    model = ZiTVaeModel(_HYPERPARAMS)
    tensors: dict[str, torch.Tensor] = {}
    for key, value in model.state_dict().items():
        checkpoint_key = re.sub(r"\.block_(\d+)\.", r".blocks.\1.", key)
        tensors[checkpoint_key] = value.detach().clone().to(torch.float8_e4m3fn)
    tensors["latents"] = torch.randn(
        1, _HYPERPARAMS["latent_channels"], 8, 8, dtype=torch.float32
    ).to(torch.float8_e4m3fn)
    return tensors


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
