#!/usr/bin/env python3
"""Build the ZiT diffusion fixture checkpoint files.

Generates two tiny synthetic .safetensors files used by real-mode tests to
exercise the model-loading contract end-to-end:

1. zit_tiny.safetensors -- a COMPLETE checkpoint: every real parameter in
   ``ZiTModel``'s actual ``state_dict()`` (P902 retrofit; previously this
   fixture hand-picked 8 representative keys, leaving 22 of the model's 28
   real parameters -- every bias, both LayerNorms, the feed-forward block,
   and the second attention block's output projections -- unpopulated by
   the checkpoint. ``load()`` materializes those via ``model.to_empty()``,
   which allocates *uninitialized* memory, not zeros, despite ``load()``'s
   historical assumption otherwise; the result was NaN or near-float32-max
   garbage propagating through the very first forward pass of every
   real-mode ``sample()``/E2E test, invisible because none of them checked
   parameter values, only shapes/dtypes/devices). Built by directly
   constructing ``ZiTModel(_HYPERPARAMS)`` (real device, not meta) and
   dumping its full ``state_dict()`` -- this guarantees the fixture always
   exactly matches the real architecture, keyed identically, with no
   hand-maintained key list to fall out of sync as the model evolves.
   Carries ``arch: "zit"`` metadata in the safetensors header.

2. zit_tiny_no_metadata.safetensors -- unchanged by this retrofit: a
   deliberately minimal, non-recognizable-key-prefix fixture that only
   needs to exercise ``_infer_hyperparams()``'s metadata-fallback path and
   ``load()``'s ability to construct without crashing -- not a full
   forward pass -- so full-state-dict completeness doesn't apply to it.
   See ``_no_metadata_tensors()``'s docstring.

``_HYPERPARAMS`` reproduces exactly what ``_infer_hyperparams()`` inferred
from the pre-P902 fixture (hidden_dim=64, double_block_count=1,
single_block_count=1, latent_channels=4, latent_height=4, latent_width=4,
patch_size=4) -- chosen so every existing test that depends on these
inferred values (shape assertions, dtype selection, etc.) is unaffected by
this retrofit; only the fixture's *completeness*, not its *shape*, changed.

Two synthetic marker tensors -- "latents" and "c_crossattn_dim" -- are
included alongside the real state_dict. These are not real ``ZiTModel``
parameters; ``_infer_hyperparams()`` reads them directly (``"latents"``'s
shape[1] for ``latent_channels``; ``"c_crossattn_dim"``'s shape[0] as a
``hidden_dim`` fallback) as part of its checkpoint-header shape-inference
contract, independent of the model's actual architecture.

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_fixture.py
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
# worker/tests/fixtures/ -> repo root) onto sys.path so `import worker...`
# resolves regardless of invocation style. Running this script directly
# (`python worker/tests/fixtures/build_zit_fixture.py`) puts only this
# file's own directory on sys.path[0], not the repo root, even when
# invoked from the repo root as cwd -- unlike `python -c` or `-m`, which
# do include cwd.
_REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
)
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from worker.nodes.arch.diffusion.zit import ZiTModel  # noqa: E402

# Hyperparameters for the real ZiTModel construction below. These exactly
# reproduce what _infer_hyperparams() derived from the pre-P902 fixture --
# see this module's docstring for why that equivalence matters.
_HYPERPARAMS: dict[str, int] = {
    "hidden_dim": 64,
    "double_block_count": 1,
    "single_block_count": 1,
    "latent_channels": 4,
    "latent_height": 4,
    "latent_width": 4,
    "patch_size": 4,
}


def _zit_tensors() -> dict[str, torch.Tensor]:
    """Return a COMPLETE tensor dict covering every real ZiTModel parameter.

    Constructs ``ZiTModel(_HYPERPARAMS)`` directly on the real (CPU)
    device -- not the meta device ``load()`` uses for memory efficiency --
    so every ``nn.Linear``/``nn.LayerNorm``/``nn.MultiheadAttention``
    submodule runs its normal PyTorch default initialization
    (``reset_parameters()``), producing finite, properly-scaled values for
    every one of the model's real parameters. ``torch.manual_seed()`` is
    set immediately before construction so the fixture's content is
    reproducible across runs of this script, unlike the pre-P902 version
    (which called unseeded ``torch.randn()`` per tensor).

    Two non-parameter marker tensors ("latents", "c_crossattn_dim") are
    added on top of the real state_dict -- see this module's docstring.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values: every key
        in ``ZiTModel(_HYPERPARAMS).state_dict()``, plus the two marker
        tensors.
    """
    torch.manual_seed(42)
    model = ZiTModel(_HYPERPARAMS)
    tensors: dict[str, torch.Tensor] = {
        key: value.detach().clone()
        for key, value in model.state_dict().items()
    }

    # Marker tensors for _infer_hyperparams()'s shape-inference contract
    # (not real ZiTModel parameters -- see module docstring).
    tensors["latents"] = torch.randn(1, 4, 8, 8)
    tensors["c_crossattn_dim"] = torch.randn(_HYPERPARAMS["hidden_dim"])

    return tensors


def _no_metadata_tensors() -> dict[str, torch.Tensor]:
    """Return a tensor dict with non-recognizable ``xyz_`` key prefixes.

    Same structural shapes as :func:`_zit_tensors` but with a prefix that
    no known architecture pattern matcher can identify. Combined with the
    absent ``arch`` metadata key, this exercises the metadata-fallback
    code path.

    ``xyz_latents``'s shape is ``(1, 4, 4, 4)``, NOT ``(1, 4, 8, 8)`` like
    ``_zit_tensors()``'s "latents" marker -- and this difference matters,
    unlike in the regular fixture. There, "latents" is only ever used for
    its channel dimension (``_infer_hyperparams()``'s primary path derives
    ``latent_height``/``latent_width`` from ``input_proj.weight``'s shape
    instead, since that key is present, so "latents"'s own spatial dims
    are never read). Here, with no ``input_proj.weight``-style key
    present, ``_infer_hyperparams()`` falls back to reading
    ``latent_height``/``latent_width`` directly from this tensor's own
    spatial shape — so it must already equal ``(1, 4, 4, 4)`` for this
    fixture to reproduce ``_HYPERPARAMS``' values (patch_size=4,
    latent_height=4, latent_width=4), as this module's docstring and
    ``test_infer_hyperparams_no_metadata_fixture`` both require. (P902
    fix: found via a pre-existing, unrelated bug this retrofit's fixture
    rebuild exposed — the *checked-in* zit_tiny_no_metadata.safetensors
    binary predated a source-code edit that changed this shape to
    ``(1, 4, 8, 8)``, matching ``_zit_tensors()``'s comment by copy-paste
    without noticing this function's fallback path actually depends on
    the value, unlike that one. The binary was never regenerated to match
    the edit, so CI — which only ever loads the committed binary, never
    re-runs this script — silently kept testing the old, correct shape
    while the source drifted to a shape that would have failed the exact
    moment anyone did regenerate it, as this retrofit's own fixture
    rebuild immediately did.)

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values.
    """
    return {
        "xyz_random_tensor_data": torch.randn(64, 64),
        "xyz_another_tensor": torch.randn(64, 64),
        "xyz_c_crossattn_dim": torch.randn(64),
        "xyz_double_block_img_attn": torch.randn(64, 64),
        "xyz_double_block_txt_attn": torch.randn(64, 64),
        "xyz_single_block_linear": torch.randn(64, 64),
        "xyz_output_proj": torch.randn(64, 64),
        "xyz_latents": torch.randn(1, 4, 4, 4),
    }


def build() -> None:
    """Build both ZiT fixture .safetensors files.

    Writes:
        - ``zit_tiny.safetensors`` with ``arch: "zit"`` metadata.
        - ``zit_tiny_no_metadata.safetensors`` with no metadata, exercising
          the metadata-fallback regression path.

    Both files are written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    # Regular fixture with arch metadata
    regular_path = os.path.join(_FIXTURES_DIR, "zit_tiny.safetensors")
    save_file(
        _zit_tensors(),
        regular_path,
        metadata={"arch": "zit"},
    )
    print(f"Written: {regular_path}")

    # No-metadata fixture — non-recognizable keys, no arch key in header
    no_meta_path = os.path.join(_FIXTURES_DIR, "zit_tiny_no_metadata.safetensors")
    save_file(
        _no_metadata_tensors(),
        no_meta_path,
        # No metadata argument — header will contain no ``arch`` key.
    )
    print(f"Written: {no_meta_path}")


if __name__ == "__main__":
    build()
