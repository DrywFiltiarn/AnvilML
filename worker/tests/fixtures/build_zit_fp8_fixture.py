#!/usr/bin/env python3
"""Build the ZiT FP8 fixture checkpoint file.

Generates a tiny synthetic .safetensors file with FP8 (float8_e4m3fn)
dtype tensors, used by real-mode tests to exercise the FP8 branch of
the dtype selection precedence (ANVILML_DESIGN.md §11.5).

P902 retrofit: like zit_tiny.safetensors, this fixture previously
hand-picked 8 representative keys, leaving most of the model's real
parameters unpopulated by the checkpoint (relying on load()'s now-fixed
to_empty()-without-zero bug). No test currently loads this specific
fixture (grep found no references outside this file), so the gap was
latent rather than actively manifesting -- but it's fixed here for
consistency and in case that changes. Built the same way as
zit_tiny.safetensors: constructs the real ``ZiTModel`` with the exact same
hyperparameters (reusing ``build_zit_fixture``'s ``_HYPERPARAMS``, which
matches what ``_infer_hyperparams()`` already derives from this fixture --
only ``native_dtype`` differs, to fp8), dumps its full ``state_dict()``,
then casts every tensor to ``float8_e4m3fn`` (``torch.randn()`` does not
support float8 directly on CPU builds, so tensors are created/initialized
in float32 and cast afterward).

Usage:
    worker/.venv/bin/python worker/tests/fixtures/build_zit_fp8_fixture.py
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

from worker.nodes.arch.diffusion.zit import ZiTModel  # noqa: E402

# Same hyperparameters as build_zit_fixture.py's _HYPERPARAMS -- kept as an
# independent copy (rather than importing it) so this script has no
# dependency on build_zit_fixture.py's internals, only on ZiTModel itself.
_HYPERPARAMS: dict[str, int] = {
    "hidden_dim": 64,
    "double_block_count": 1,
    "single_block_count": 1,
    "latent_channels": 4,
    "latent_height": 4,
    "latent_width": 4,
    "patch_size": 4,
}


def _zit_fp8_tensors() -> dict[str, torch.Tensor]:
    """Return a COMPLETE tensor dict covering every real ZiTModel parameter, in FP8.

    Constructs ``ZiTModel(_HYPERPARAMS)`` directly on the real (CPU)
    device in its default (float32) dtype -- torch.randn()/default
    nn.Module init don't support float8 directly -- then casts every
    parameter to ``float8_e4m3fn`` afterward. ``torch.manual_seed()`` is
    set immediately before construction for reproducibility.

    Returns:
        Dict mapping tensor names to ``torch.Tensor`` values in FP8: every
        key in ``ZiTModel(_HYPERPARAMS).state_dict()``, plus the two
        marker tensors ``_infer_hyperparams()`` needs (see
        build_zit_fixture.py's module docstring).
    """
    torch.manual_seed(42)
    model = ZiTModel(_HYPERPARAMS)
    tensors: dict[str, torch.Tensor] = {
        key: value.detach().clone().to(torch.float8_e4m3fn)
        for key, value in model.state_dict().items()
    }
    tensors["latents"] = torch.randn(1, 4, 8, 8, dtype=torch.float32).to(
        torch.float8_e4m3fn
    )
    tensors["c_crossattn_dim"] = torch.randn(
        _HYPERPARAMS["hidden_dim"], dtype=torch.float32
    ).to(torch.float8_e4m3fn)
    return tensors


def build() -> None:
    """Build the ZiT FP8 fixture .safetensors file.

    Writes:
        - ``zit_tiny_fp8.safetensors`` with ``arch: "zit"`` metadata and
          FP8 (float8_e4m3fn) tensor dtype.

    The file is written to ``worker/tests/fixtures/`` (the directory
    containing this script). The script is idempotent — safe to re-run
    without side effects.
    """
    fp8_path = os.path.join(_FIXTURES_DIR, "zit_tiny_fp8.safetensors")
    save_file(
        _zit_fp8_tensors(),
        fp8_path,
        metadata={"arch": "zit"},
    )
    print(f"Written: {fp8_path}")


if __name__ == "__main__":
    build()
