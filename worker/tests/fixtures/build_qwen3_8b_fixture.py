#!/usr/bin/env python3
"""Build the Qwen3-8B fixture checkpoint file.

Generates a tiny synthetic .safetensors file used by real-mode tests to
exercise the Qwen3 text-encoder model-loading contract end-to-end with
an 8B-class architecture:

1. qwen3_8b_tiny.safetensors -- a representative subset of the actual
   Qwen3/HF checkpoint keys (model.layers.N.*, model.norm.*,
   model.embed_tokens.*) with structurally valid tensor shapes.
   Carries ``arch: "qwen3"`` metadata in the safetensors header.

The dimension values (hidden_dim=128, intermediate_size=256) are larger
than the 4B fixture (hidden_dim=64, intermediate_size=128) to be
distinguishable, but small enough to keep file size under 10 MB. The
key patterns match Qwen3's actual checkpoint naming convention
(separate self_attn.{q,k,v,o}_proj keys, not concatenated in_proj) so
that ``_infer_hyperparams()`` can parse them once the real architecture
module exists.

This fixture also includes a small set of tensors at ``torch.float8_e4m3fn``
dtype to demonstrate mixed-precision checkpoint loading — the same
capability the real Qwen3-8B model uses.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_qwen3_8b_fixture.py
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

# Hyperparameters for the Qwen3-8B fixture. These are structurally valid
# but tiny (hidden_dim=128 instead of the real ~4096) so the fixture
# stays under 10 MB. The key patterns match Qwen3's actual checkpoint
# naming convention so that ``_infer_hyperparams()`` can parse them once
# the real architecture module exists.
_HIDDEN_DIM = 128
_NUM_HIDDEN_LAYERS = 1
_INTERMEDIATE_SIZE = 256
_VOCAB_SIZE = 128


def _qwen3_8b_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with Qwen3-8B checkpoint key patterns.

    Hand-crafted tensors matching the actual Qwen3/HF checkpoint key
    structure (model.layers.N.*, model.norm.*, model.embed_tokens.*).
    Uses structurally valid shapes with hidden_dim=128 to distinguish
    from the 4B fixture (hidden_dim=64) while keeping file size under
    10 MB.

    Includes a small set of FP8 tensors (``float8_e4m3fn`` dtype) to
    demonstrate mixed-precision checkpoint loading.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    tensors: dict[str, torch.Tensor] = {}

    # -- Layer 0: self-attention projections --
    # q/k/v projections: (hidden_dim, hidden_dim) each
    tensors["model.layers.0.self_attn.q_proj.weight"] = torch.randn(
        _HIDDEN_DIM, _HIDDEN_DIM
    )
    tensors["model.layers.0.self_attn.k_proj.weight"] = torch.randn(
        _HIDDEN_DIM, _HIDDEN_DIM
    )
    tensors["model.layers.0.self_attn.v_proj.weight"] = torch.randn(
        _HIDDEN_DIM, _HIDDEN_DIM
    )
    # Output projection: (hidden_dim, hidden_dim)
    tensors["model.layers.0.self_attn.o_proj.weight"] = torch.randn(
        _HIDDEN_DIM, _HIDDEN_DIM
    )

    # -- Layer 0: MLP feed-forward projections --
    # gate and up projections: (intermediate_size, hidden_dim)
    tensors["model.layers.0.mlp.gate_proj.weight"] = torch.randn(
        _INTERMEDIATE_SIZE, _HIDDEN_DIM
    )
    tensors["model.layers.0.mlp.up_proj.weight"] = torch.randn(
        _INTERMEDIATE_SIZE, _HIDDEN_DIM
    )
    # Down projection: (hidden_dim, intermediate_size)
    tensors["model.layers.0.mlp.down_proj.weight"] = torch.randn(
        _HIDDEN_DIM, _INTERMEDIATE_SIZE
    )

    # -- Layer 0: LayerNorm weights --
    tensors["model.layers.0.input_layernorm.weight"] = torch.randn(
        _HIDDEN_DIM
    )
    tensors["model.layers.0.post_attention_layernorm.weight"] = torch.randn(
        _HIDDEN_DIM
    )

    # -- Output layer: normalization --
    tensors["model.norm.weight"] = torch.randn(_HIDDEN_DIM)

    # -- Embedding layer --
    tensors["model.embed_tokens.weight"] = torch.randn(
        _VOCAB_SIZE, _HIDDEN_DIM
    )

    # -- FP8 tensors (to demonstrate mixed-precision checkpoint loading) --
    # These are copies of the float32 tensors converted to float8_e4m3fn.
    # torch.float8_e4m3fn is available in torch 2.5+ (the project's
    # current torch build); it supports native tensor construction via
    # .to(torch.float8_e4m3fn).
    tensors["model.layers.0.self_attn.q_proj.weight_fp8"] = (
        torch.randn(_HIDDEN_DIM, _HIDDEN_DIM)
        .to(torch.float8_e4m3fn)
    )
    tensors["model.layers.0.mlp.gate_proj.weight_fp8"] = (
        torch.randn(_INTERMEDIATE_SIZE, _HIDDEN_DIM)
        .to(torch.float8_e4m3fn)
    )
    tensors["model.embed_tokens.weight_fp8"] = (
        torch.randn(_VOCAB_SIZE, _HIDDEN_DIM)
        .to(torch.float8_e4m3fn)
    )

    return tensors


def build() -> None:
    """Build the Qwen3-8B fixture .safetensors file.

    Writes:
        - ``qwen3_8b_tiny.safetensors`` with ``arch: "qwen3"``
          metadata.

    The file is written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent -- safe to re-run
    without side effects.

    No no-metadata variant is generated here -- the per-family
    metadata-fallback regression case is already covered by the
    existing qwen3_tiny_no_metadata.safetensors fixture.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "qwen3_8b_tiny.safetensors")
    save_file(
        _qwen3_8b_tensors(),
        regular_path,
        metadata={"arch": "qwen3"},
    )
    print(f"Written: {regular_path}")


if __name__ == "__main__":
    build()
