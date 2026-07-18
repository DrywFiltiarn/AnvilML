#!/usr/bin/env python3
"""Build the Qwen3 CLIP fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the Qwen3 CLIP text-encoder loading contract end-to-end:

1. qwen3_tiny.safetensors -- a COMPLETE checkpoint, in the real Qwen3/HF
   checkpoint key convention ("model."-prefixed, separate
   self_attn.{q,k,v,o}_proj.{weight,bias}), covering every real parameter
   in ``Qwen3TextEncoder``'s actual ``state_dict()`` (P902 retrofit;
   previously this fixture had no bias tensors at all -- only weights --
   leaving every one of the model's bias parameters unpopulated. Worse,
   *none* of this fixture's weights actually loaded either, due to three
   separate bugs in ``qwen3.py``'s loading code this retrofit also fixed:
   a missing "model." prefix strip, a typo'd ``in_proj.weight`` target key
   name (real PyTorch attribute is ``in_proj_weight``, no dot), and no
   actual q/k/v concatenation logic -- see ``_normalize_attention_keys()``
   in ``qwen3.py`` for the full explanation. Before this retrofit, 0 of
   this module's 31 real parameters were ever successfully loaded from any
   realistically-shaped checkpoint, regardless of what this fixture
   contained). Built by directly constructing
   ``Qwen3TextEncoder(_HYPERPARAMS)`` (real device, not meta), dumping its
   full ``state_dict()``, and translating each key from the model's
   internal parameter layout back into the checkpoint convention
   (``model.`` prefix added; ``self_attn.in_proj_weight``/``in_proj_bias``
   split into separate ``q_proj``/``k_proj``/``v_proj`` thirds;
   ``self_attn.out_proj.{weight,bias}`` renamed to
   ``self_attn.o_proj.{weight,bias}``) -- the exact inverse of
   ``_normalize_attention_keys()``, so round-tripping through this
   fixture and ``load()`` reproduces the original model's weights exactly.
   Carries ``arch: "qwen3"`` metadata in the safetensors header.

2. qwen3_tiny_no_metadata.safetensors -- unchanged in spirit by this
   retrofit (still reuses the real key schema rather than mangling it —
   see ``_no_metadata_tensors()``'s docstring, unchanged from the P901
   retrofit that added it), but now built from the same complete,
   correctly-loadable tensor set as (1) rather than the old
   weights-only/no-bias set.

``_HYPERPARAMS`` reproduces exactly what ``_infer_hyperparams()`` inferred
from the pre-P902 fixture (hidden_dim=64, num_hidden_layers=2,
intermediate_size=128, vocab_size=128) -- chosen so every existing test
that depends on these inferred values is unaffected by this retrofit; only
the fixture's *completeness* and *loadability*, not its *shape*, changed.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_qwen3_fixture.py
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

from worker.nodes.arch.clip.qwen3 import Qwen3TextEncoder  # noqa: E402

# Hyperparameters for the real Qwen3TextEncoder construction below. These
# exactly reproduce what _infer_hyperparams() derived from the pre-P902
# fixture -- see this module's docstring for why that equivalence matters.
_HYPERPARAMS: dict[str, int] = {
    "hidden_dim": 64,
    "num_hidden_layers": 2,
    "intermediate_size": 128,
    "vocab_size": 128,
}


def _to_checkpoint_convention(
    state_dict: dict[str, torch.Tensor]
) -> dict[str, torch.Tensor]:
    """Translate a real Qwen3TextEncoder state_dict into checkpoint-key form.

    This is the exact inverse of ``qwen3.py``'s ``_normalize_attention_keys()``:

    - Every key gets a "model." prefix added.
    - ``self_attn.in_proj_weight`` (shape ``(3*hidden_dim, hidden_dim)``)
      is split into three equal thirds along dim 0, written out as
      separate ``self_attn.{q,k,v}_proj.weight`` tensors (each
      ``(hidden_dim, hidden_dim)``) -- matching the real Qwen3/HF
      checkpoint convention. ``in_proj_bias`` is split the same way into
      ``{q,k,v}_proj.bias``.
    - ``self_attn.out_proj.{weight,bias}`` is renamed to
      ``self_attn.o_proj.{weight,bias}``.
    - Every other key passes through with only the "model." prefix added.

    Args:
        state_dict: A real ``Qwen3TextEncoder`` instance's ``state_dict()``.

    Returns:
        A dict keyed in the checkpoint convention, ready to write via
        ``safetensors.torch.save_file()``.
    """
    tensors: dict[str, torch.Tensor] = {}
    for key, value in state_dict.items():
        value = value.detach().clone()
        if key.endswith("self_attn.in_proj_weight"):
            prefix = key[: -len("in_proj_weight")]
            q, k, v = value.chunk(3, dim=0)
            tensors[f"model.{prefix}q_proj.weight"] = q
            tensors[f"model.{prefix}k_proj.weight"] = k
            tensors[f"model.{prefix}v_proj.weight"] = v
        elif key.endswith("self_attn.in_proj_bias"):
            prefix = key[: -len("in_proj_bias")]
            q, k, v = value.chunk(3, dim=0)
            tensors[f"model.{prefix}q_proj.bias"] = q
            tensors[f"model.{prefix}k_proj.bias"] = k
            tensors[f"model.{prefix}v_proj.bias"] = v
        elif "self_attn.out_proj." in key:
            tensors[f"model.{key.replace('out_proj.', 'o_proj.')}"] = value
        else:
            tensors[f"model.{key}"] = value
    return tensors


def _qwen3_tensors() -> dict[str, torch.Tensor]:
    """Return a COMPLETE, checkpoint-convention tensor dict for the fixture.

    Constructs ``Qwen3TextEncoder(_HYPERPARAMS)`` directly on the real
    (CPU) device -- not the meta device ``load()`` uses for memory
    efficiency -- so every ``nn.Embedding``/``nn.Linear``/``nn.LayerNorm``/
    ``nn.MultiheadAttention`` submodule runs its normal PyTorch default
    initialization, producing finite, properly-scaled values for every one
    of the model's real parameters. ``torch.manual_seed()`` is set
    immediately before construction for reproducibility. The resulting
    state_dict is then translated into the real checkpoint key convention
    via ``_to_checkpoint_convention()``.

    Returns:
        Dict mapping checkpoint-convention tensor names to
        ``torch.Tensor`` values, covering every real Qwen3TextEncoder
        parameter.
    """
    torch.manual_seed(42)
    model = Qwen3TextEncoder(_HYPERPARAMS)
    return _to_checkpoint_convention(model.state_dict())

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
