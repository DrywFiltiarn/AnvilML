#!/usr/bin/env python3
"""Build the Flux 2 Klein 9B fixture checkpoint file.

Generates a tiny synthetic .safetensors file used by real-mode tests to
exercise the Flux 2 Klein model-loading contract end-to-end with a
9B-class architecture:

1. flux2klein9b_tiny.safetensors -- a representative subset of the actual
   Flux 2 Klein 9B checkpoint keys (double_blocks, single_blocks,
   time_text_embed, final_layer) with structurally valid tensor shapes.
   Carries ``arch: "flux2klein"`` metadata in the safetensors header.

The dimension values (hidden_dim=256, context_dim=512) are larger than the
4B fixture (hidden_dim=128, context_dim=768) to be distinguishable, but
small enough to keep file size under 10 MB. The key patterns match Flux 2
Klein's actual checkpoint naming convention so that
``_infer_hyperparams()`` can parse them once the real architecture module
exists.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_flux2klein_9b_fixture.py
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
# resolves regardless of invocation style. Running this script directly
# puts only this file's own directory on sys.path[0], not the repo root,
# even when invoked from the repo root as cwd.
_REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
)
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

# Hyperparameters for the Flux 2 Klein 9B fixture. These are structurally
# valid but tiny (hidden_dim=256 instead of the real ~3072) so the fixture
# stays under 10 MB. The key patterns match Flux 2 Klein's actual checkpoint
# naming convention so that ``_infer_hyperparams()`` can parse them once
# the real architecture module exists.
#
# context_dim is reduced from the real 4096 to 512 because the text
# attention QKV tensor at (context_dim, hidden_dim*3) would be ~12 MB at
# full scale -- exceeding the 10 MB total limit. 512 keeps the total file
# size at ~5 MB while preserving the correct shape relationship.
_HIDDEN_DIM = 256
_CONTEXT_DIM = 512
_LATENT_CHANNELS = 4
_PATCH_SIZE = 2
_OUT_CHANNELS = 4


def _flux2klein_9b_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with Flux 2 Klein 9B checkpoint key patterns.

    Hand-crafted tensors matching the actual Flux 2 Klein 9B checkpoint
    key structure (double_blocks, single_blocks, time_text_embed,
    final_layer). Uses structurally valid shapes with hidden_dim=256
    to distinguish from the 4B fixture (hidden_dim=128) while keeping
    file size under 10 MB.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        # Time/text embedding blocks
        "time_text_embed.timestep_embedder.0.weight": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM
        ),
        "time_text_embed.context_embedder": torch.randn(
            _HIDDEN_DIM, _CONTEXT_DIM
        ),
        # Double block 0 -- image modulation
        "double_blocks.0.img_mod.lin": torch.randn(_HIDDEN_DIM * 6),
        # Double block 0 -- text modulation
        "double_blocks.0.txt_mod.lin": torch.randn(_HIDDEN_DIM * 6),
        # Double block 0 -- image attention QKV
        "double_blocks.0.img_attn.qkv": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 3
        ),
        # Double block 0 -- image attention norm
        "double_blocks.0.img_attn.norm": torch.randn(_HIDDEN_DIM),
        # Double block 0 -- image attention projection
        "double_blocks.0.img_attn.proj": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM
        ),
        # Double block 0 -- text attention QKV
        "double_blocks.0.txt_attn.qkv": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM * 3
        ),
        # Double block 0 -- text attention norm
        "double_blocks.0.txt_attn.norm": torch.randn(_HIDDEN_DIM),
        # Double block 0 -- text attention projection
        "double_blocks.0.txt_attn.proj": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM
        ),
        # Double block 0 -- image MLP up (swiglu gate)
        "double_blocks.0.img_mlp.0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Double block 0 -- image MLP down
        "double_blocks.0.img_mlp.1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Double block 0 -- text MLP up (swiglu gate)
        "double_blocks.0.txt_mlp.0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Double block 0 -- text MLP down
        "double_blocks.0.txt_mlp.1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Single block 0 -- linear1
        "single_blocks.0.linear1": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Single block 0 -- linear2
        "single_blocks.0.linear2": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Single block 0 -- norm
        "single_blocks.0.norm": torch.randn(_HIDDEN_DIM),
        # Final layer -- output projection
        "final_layer.linear": torch.randn(
            _HIDDEN_DIM,
            _PATCH_SIZE * _PATCH_SIZE * _OUT_CHANNELS,
        ),
        # Final layer -- adaptive LN modulation
        "final_layer.adaLN_modulation.1": torch.randn(_HIDDEN_DIM * 2),
        # Marker tensor for _infer_hyperparams()'s shape-inference
        # contract (not a real Flux 2 Klein parameter).
        "latents": torch.randn(1, _LATENT_CHANNELS, 8, 8),
    }


def build() -> None:
    """Build the Flux 2 Klein 9B fixture .safetensors file.

    Writes:
        - ``flux2klein9b_tiny.safetensors`` with ``arch: "flux2klein"``
          metadata.

    The file is written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent -- safe to re-run
    without side effects.

    No no-metadata variant is generated here -- the per-family
    metadata-fallback regression case is already covered by the
    existing flux2klein4b_tiny_no_metadata.safetensors fixture.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "flux2klein9b_tiny.safetensors")
    save_file(
        _flux2klein_9b_tensors(),
        regular_path,
        metadata={"arch": "flux2klein"},
    )
    print(f"Written: {regular_path}")


if __name__ == "__main__":
    build()
