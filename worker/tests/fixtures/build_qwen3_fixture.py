#!/usr/bin/env python3
"""Build the Qwen3 CLIP fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the Qwen3 CLIP text-encoder loading contract end-to-end:

1. qwen3_tiny.safetensors — Qwen3 text-encoder-shaped tensor keys with
   ``arch: "qwen3"`` metadata in the safetensors header. Recognizable key
   prefixes (``model.embed_tokens``, ``model.layers.N.self_attn.*_proj``,
   ``model.layers.N.mlp.*_proj``, ``model.layers.N.*_layernorm``,
   ``model.norm``) let the loader identify the architecture family from
   key naming conventions alone.

2. qwen3_tiny_no_metadata.safetensors — the same tensor keys and shapes as
   (1), but with no ``arch`` metadata key in the header. Exercises the
   metadata-fallback code path (the historical ``st.metadata`` vs
   ``st.metadata()`` call-as-property bug this project has hit before —
   ``ANVILML_DESIGN.md`` §17.5) — see ``_no_metadata_tensors()`` for why
   this fixture keeps the real key schema rather than mangling it the way
   ``build_zit_fixture.py``'s equivalent does.

Tensor shapes are structurally valid for a transformer-based text encoder's
shape-inference formula (consistent hidden dimension of 64 — a structurally
valid but small dimension that stays well under the 10 MB per-file budget
while preserving the same key-prefix patterns and shape relationships as a
real Qwen3 text encoder), 2 hidden layers, vocab size 128.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_qwen3_fixture.py
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


def _qwen3_tensors() -> dict[str, torch.Tensor]:
    """Return the Qwen3 text-encoder-shaped tensor dict for the fixture.

    Keys follow the standard Qwen/Qwen3 transformer text-encoder safetensors
    key naming convention:

    - ``model.embed_tokens.weight`` — vocab_size × hidden_dim embedding table
    - ``model.layers.N.self_attn.{q,k,v,o}_proj.weight`` — per-layer
      attention projections (hidden_dim × hidden_dim for q/k/v/o)
    - ``model.layers.N.mlp.{gate,up,down}_proj.weight`` — per-layer MLP
      projections (intermediate_size × hidden_dim for gate/up,
      hidden_dim × intermediate_size for down)
    - ``model.layers.N.{input,post_attention}_layernorm.weight`` — per-layer
      layer norm scale vectors (hidden_dim,)
    - ``model.norm.weight`` — final normalization (hidden_dim,)

    Hidden dimension is 64 (consistent across all tensors), num_hidden_layers
    is 2, intermediate_size is 128, vocab_size is 128. These are structurally
    valid dimensions for a transformer-based text encoder shape-inference
    formula — not a miniaturized copy of the real 4B model's actual shapes.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    hidden_dim = 64
    intermediate_size = 128
    vocab_size = 128
    num_hidden_layers = 2

    tensors: dict[str, torch.Tensor] = {
        # Embedding table: vocab_size × hidden_dim
        "model.embed_tokens.weight": torch.randn(vocab_size, hidden_dim),
    }

    for layer_idx in range(num_hidden_layers):
        layer_prefix = f"model.layers.{layer_idx}"

        # Self-attention projections — all hidden_dim × hidden_dim
        tensors[f"{layer_prefix}.self_attn.q_proj.weight"] = torch.randn(
            hidden_dim, hidden_dim
        )
        tensors[f"{layer_prefix}.self_attn.k_proj.weight"] = torch.randn(
            hidden_dim, hidden_dim
        )
        tensors[f"{layer_prefix}.self_attn.v_proj.weight"] = torch.randn(
            hidden_dim, hidden_dim
        )
        tensors[f"{layer_prefix}.self_attn.o_proj.weight"] = torch.randn(
            hidden_dim, hidden_dim
        )

        # MLP projections — gate and up are intermediate_size × hidden_dim,
        # down is hidden_dim × intermediate_size (the expand-then-contract
        # pattern used by SwiGLU-style MLPs in Qwen3)
        tensors[f"{layer_prefix}.mlp.gate_proj.weight"] = torch.randn(
            intermediate_size, hidden_dim
        )
        tensors[f"{layer_prefix}.mlp.up_proj.weight"] = torch.randn(
            intermediate_size, hidden_dim
        )
        tensors[f"{layer_prefix}.mlp.down_proj.weight"] = torch.randn(
            hidden_dim, intermediate_size
        )

        # Layer norm scale vectors — hidden_dim-dimensional
        tensors[f"{layer_prefix}.input_layernorm.weight"] = torch.randn(
            hidden_dim
        )
        tensors[f"{layer_prefix}.post_attention_layernorm.weight"] = (
            torch.randn(hidden_dim)
        )

    # Final normalization after all transformer layers
    tensors["model.norm.weight"] = torch.randn(hidden_dim)

    return tensors


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return the same Qwen3-shaped tensor dict as :func:`_qwen3_tensors`.

    Unlike ``build_zit_fixture.py``'s ``_no_metadata_tensors()`` (which uses
    a non-recognizable ``xyz_`` key prefix), this fixture reuses the real
    Qwen3 key schema unchanged: ``qwen3.py``'s ``_infer_hyperparams()``
    computes ``hidden_dim`` from the shapes of specifically-named keys
    (``self_attn.{q,k,v,o}_proj.weight``, etc. — see its docstring) rather
    than zit.py's more generic shape-counting approach, so a
    non-recognizable prefix would simply fail to load at all rather than
    exercising a fallback path. What this fixture isolates instead is the
    exact regression this file's module docstring names: the historical
    ``st.metadata`` vs ``st.metadata()`` call-as-property bug, i.e.
    correctly handling a header with recognizable keys but *no* ``arch``
    entry in its metadata dict (``ANVILML_DESIGN.md`` §17.5; P901 retrofit
    — this is the CLIP family's counterpart to the diffusion/VAE families'
    no-metadata fixtures, using the key-shape difference between the arch
    families deliberately rather than copying their prefix-mangling
    approach verbatim).

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values, identical to
        :func:`_qwen3_tensors`'s output.
    """
    return _qwen3_tensors()


def build() -> None:
    """Build both Qwen3 CLIP fixture .safetensors files.

    Writes:
        - ``qwen3_tiny.safetensors`` with ``arch: "qwen3"`` metadata.
        - ``qwen3_tiny_no_metadata.safetensors`` with no metadata,
          exercising the metadata-fallback regression path (P901 retrofit;
          `ANVILML_DESIGN.md` §17.5).

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    regular_path = os.path.join(_FIXTURES_DIR, "qwen3_tiny.safetensors")
    save_file(
        _qwen3_tensors(),
        regular_path,
        metadata={"arch": "qwen3"},
    )
    print(f"Written: {regular_path}")

    no_meta_path = os.path.join(_FIXTURES_DIR, "qwen3_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
