#!/usr/bin/env python3
"""Build the Flux 2 Klein 4B fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the Flux 2 Klein model-loading contract end-to-end:

1. flux2klein4b_tiny.safetensors -- a representative subset of the actual
   Flux 2 Klein 4B checkpoint keys (double_blocks, single_blocks,
   time_text_embed, final_layer) with structurally valid tensor shapes.
   Carries ``arch: "flux2klein"`` metadata in the safetensors header.

2. flux2klein4b_tiny_no_metadata.safetensors -- a deliberately minimal,
   non-recognizable-key-prefix fixture that only needs to exercise
   ``_infer_hyperparams()``'s metadata-fallback path. Built independently
   of ``_flux2klein_tensors()`` with ``xyz_``-prefixed keys and no
   ``arch`` metadata in the header.

Since ``flux2klein.py`` does not yet exist, these use hand-crafted tensors
with Flux 2 Klein's actual checkpoint key pattern and structurally valid
shapes. The dimension values (hidden_dim=128, context_dim=768, etc.) are
small enough to keep file size under 10 MB but large enough to exercise
the shape-inference formula.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_flux2klein_fixture.py
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

# Hyperparameters for the Flux 2 Klein fixture. These are structurally valid
# but tiny (hidden_dim=128 instead of the real 3072) so the fixture stays
# under 10 MB. The key patterns match Flux 2 Klein's actual checkpoint
# naming convention so that ``_infer_hyperparams()`` can parse them once
# the real architecture module exists.
_HIDDEN_DIM = 128
_CONTEXT_DIM = 768
_LATENT_CHANNELS = 4
_PATCH_SIZE = 2
_OUT_CHANNELS = 4


def _flux2klein_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with Flux 2 Klein checkpoint key patterns.

    Hand-crafted tensors matching the actual Flux 2 Klein 4B checkpoint
    key structure (double_blocks, single_blocks, time_text_embed,
    final_layer). Uses structurally valid shapes but tiny dimensions
    (hidden_dim=128) to keep file size under 10 MB.

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
        # Double block 0 — image modulation
        "double_blocks.0.img_mod.lin": torch.randn(_HIDDEN_DIM * 6),
        # Double block 0 — text modulation
        "double_blocks.0.txt_mod.lin": torch.randn(_HIDDEN_DIM * 6),
        # Double block 0 — image attention QKV
        "double_blocks.0.img_attn.qkv": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 3
        ),
        # Double block 0 — image attention norm
        "double_blocks.0.img_attn.norm": torch.randn(_HIDDEN_DIM),
        # Double block 0 — image attention projection
        "double_blocks.0.img_attn.proj": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM
        ),
        # Double block 0 — text attention QKV
        "double_blocks.0.txt_attn.qkv": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM * 3
        ),
        # Double block 0 — text attention norm
        "double_blocks.0.txt_attn.norm": torch.randn(_HIDDEN_DIM),
        # Double block 0 — text attention projection
        "double_blocks.0.txt_attn.proj": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM
        ),
        # Double block 0 — image MLP up (swiglu gate)
        "double_blocks.0.img_mlp.0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Double block 0 — image MLP down
        "double_blocks.0.img_mlp.1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Double block 0 — text MLP up (swiglu gate)
        "double_blocks.0.txt_mlp.0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Double block 0 — text MLP down
        "double_blocks.0.txt_mlp.1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Single block 0 — linear1
        "single_blocks.0.linear1": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        # Single block 0 — linear2
        "single_blocks.0.linear2": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        # Single block 0 — norm
        "single_blocks.0.norm": torch.randn(_HIDDEN_DIM),
        # Final layer — output projection
        "final_layer.linear": torch.randn(
            _HIDDEN_DIM,
            _PATCH_SIZE * _PATCH_SIZE * _OUT_CHANNELS,
        ),
        # Final layer — adaptive LN modulation
        "final_layer.adaLN_modulation.1": torch.randn(_HIDDEN_DIM * 2),
        # Marker tensor for _infer_hyperparams()'s shape-inference
        # contract (not a real Flux 2 Klein parameter).
        "latents": torch.randn(1, _LATENT_CHANNELS, 8, 8),
    }


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:`_flux2klein_tensors` but with a prefix
    that no known architecture pattern matcher can identify. Combined with
    the absent ``arch`` metadata key, this exercises the metadata-fallback
    code path.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        "xyz_time_text_embed_timestep_embedder": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM
        ),
        "xyz_time_text_embed_context_embedder": torch.randn(
            _HIDDEN_DIM, _CONTEXT_DIM
        ),
        "xyz_double_blocks_0_img_mod_lin": torch.randn(_HIDDEN_DIM * 6),
        "xyz_double_blocks_0_txt_mod_lin": torch.randn(_HIDDEN_DIM * 6),
        "xyz_double_blocks_0_img_attn_qkv": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 3
        ),
        "xyz_double_blocks_0_img_attn_norm": torch.randn(_HIDDEN_DIM),
        "xyz_double_blocks_0_img_attn_proj": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM
        ),
        "xyz_double_blocks_0_txt_attn_qkv": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM * 3
        ),
        "xyz_double_blocks_0_txt_attn_norm": torch.randn(_HIDDEN_DIM),
        "xyz_double_blocks_0_txt_attn_proj": torch.randn(
            _CONTEXT_DIM, _HIDDEN_DIM
        ),
        "xyz_double_blocks_0_img_mlp_0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        "xyz_double_blocks_0_img_mlp_1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        "xyz_double_blocks_0_txt_mlp_0": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        "xyz_double_blocks_0_txt_mlp_1": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        "xyz_single_blocks_0_linear1": torch.randn(
            _HIDDEN_DIM, _HIDDEN_DIM * 4
        ),
        "xyz_single_blocks_0_linear2": torch.randn(
            _HIDDEN_DIM * 4, _HIDDEN_DIM
        ),
        "xyz_single_blocks_0_norm": torch.randn(_HIDDEN_DIM),
        "xyz_final_layer_linear": torch.randn(
            _HIDDEN_DIM, _PATCH_SIZE * _PATCH_SIZE * _OUT_CHANNELS
        ),
        "xyz_final_layer_adaLN_modulation_1": torch.randn(
            _HIDDEN_DIM * 2
        ),
        "xyz_latents": torch.randn(1, _LATENT_CHANNELS, 8, 8),
    }


def build() -> None:
    """Build both Flux 2 Klein fixture .safetensors files.

    Writes:
        - ``flux2klein4b_tiny.safetensors`` with ``arch: "flux2klein"``
          metadata.
        - ``flux2klein4b_tiny_no_metadata.safetensors`` with no metadata,
          exercising the metadata-fallback regression path.

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "flux2klein4b_tiny.safetensors")
    save_file(
        _flux2klein_tensors(),
        regular_path,
        metadata={"arch": "flux2klein"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture — non-recognizable keys, no arch key in header
    no_meta_path = os.path.join(
        _FIXTURES_DIR, "flux2klein4b_tiny_no_metadata.safetensors"
    )
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
