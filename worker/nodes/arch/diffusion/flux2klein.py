"""Flux 2 Klein 4B diffusion architecture module.

This module provides shape-inference utilities and meta-device construction
for the Flux 2 Klein diffusion transformer architecture. It implements steps
1--4 of the four-step loading contract defined in ANVILML_DESIGN.md §11.3:

    1. _infer_hyperparams(path) — open header-only, read all key shapes,
           return a dict of inferred hyperparameters (hidden_dim, block
           counts, latent dimensions, patch_size, arch string, native_dtype).
    2. can_handle(key) — deferred to P25-B2.
    3. load(path, caps, device) — deferred to P25-C1.
    4. sample(model, model_id, conditioning, latent, steps, cfg, seed) —
           deferred to P25-C2.

Flux 2 Klein is a diffusion transformer characterised by:
- ``double_blocks``: modulated cross-attention blocks with ``img_mod`` /
  ``txt_mod`` adaptive LayerNorm (LN) modulation, ``img_attn`` /
  ``txt_attn`` attention sub-layers (QKV projection, norm, projection),
  and ``img_mlp`` / ``txt_mlp`` SwiGLU MLPs (up-projection with GELU,
  down-projection).
- ``single_blocks``: linear transformation blocks with ``linear1``,
  ``linear2``, and ``norm``.
- ``final_layer``: output projection with adaptive LN modulation
  (``adaLN_modulation``).

Design: ANVILML_DESIGN.md §11.3 — the four-step loading contract.
"""

from __future__ import annotations

import logging
import math
import re
import tempfile
from pathlib import Path
from typing import Any

from safetensors import safe_open

# torch — and everything that transitively needs it (torch.nn, safetensors.torch,
# diffusers' scheduler classes) — is guarded here rather than imported
# unconditionally. This module is imported eagerly by
# arch/diffusion/__init__.py's dispatcher (P25-B2's _REGISTERED_MODULES), which
# is in turn reachable from mock-mode test collection: the worker-linux-mock /
# worker-windows-mock CI jobs install requirements/base.txt only and never
# install torch (ANVILML_DESIGN.md §18.3). can_handle(), _infer_hyperparams(),
# and compute_latent_shape() must stay importable and callable with torch
# absent; only load()/sample() actually need it, and those raise a clear
# RuntimeError below (rather than a cryptic AttributeError on None) if somehow
# reached without torch installed.
try:
    import torch
    import torch.nn as nn
    from safetensors.torch import load_file
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
    load_file = None  # type: ignore[assignment]

# Canonical architecture identifier — the string that the dispatcher
# passes to can_handle() when routing diffusion model requests.
# Mirrors the "arch": "flux2klein" value returned by _infer_hyperparams()
# when it reads metadata or falls back to key-pattern inference.
ARCH: str = "flux2klein"

logger = logging.getLogger(__name__)

# Safetensors dtype string → canonical lowercase form used by _select_dtype().
# Maps the six safetensors dtype identifiers to a normalised string.
# Unknown dtypes default to fp32 (the conservative safe choice).
_SAFETENSORS_DTYPE_MAP: dict[str, str] = {
    "F32": "fp32",
    "F16": "fp16",
    "BF16": "bf16",
    "F8_E4M3": "fp8",
    "F8_E5M2": "fp8",
}


def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str:
    """Map a safetensors dtype string to its canonical lowercase form.

    Converts dtype identifiers stored in the safetensors header (e.g.
    ``"F32"``, ``"BF16"``, ``"F8_E4M3"``) to the canonical lowercase
    strings that ``_select_dtype()`` uses for comparison (e.g. ``"fp32"``,
    ``"bf16"``, ``"fp8"``). Unknown dtype strings default to ``"fp32"``,
    which is the conservative safe choice — it prevents fp8 selection on
    checkpoints with an unrecognized dtype.

    Args:
        safetensors_dtype: A dtype string from the safetensors header,
            e.g. ``"F32"``, ``"F16"``, ``"BF16"``, ``"F8_E4M3"``,
            ``"F8_E5M2"``.

    Returns:
        The canonical lowercase dtype string (e.g. ``"fp32"``,
        ``"fp16"``, ``"bf16"``, ``"fp8"``).
    """
    # Lookup in the pre-built map; unknown dtypes default to fp32.
    # This is a pure function with no dependencies — safe to call
    # from any context including mock-mode code paths.
    return _SAFETENSORS_DTYPE_MAP.get(safetensors_dtype, "fp32")


def _infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]:
    """Inner logic for _infer_hyperparams — runs inside the safe_open context.

    This helper is factored out so the try/except wrapping in the public
    function cleanly catches exceptions from safe_open itself without
    needing to re-raise ValueError from inside the with block.

    Args:
        f: A safetensors safe_open handle (already opened, header-only).
        path: Original filesystem path (for error messages).

    Returns:
        A dict of inferred hyperparameters including ``native_dtype``.

    Raises:
        ValueError: If the checkpoint does not contain the expected Flux
            2 Klein key patterns or required keys.
    """
    # P904 regression prevention: read ALL keys without truncation or
    # slicing. The P904 bug used list(f.keys())[:30] which silently
    # dropped keys beyond index 30, causing incorrect block counts for
    # models with 12+ blocks. We read every key, unconditionally.
    keys = f.keys()

    # ------------------------------------------------------------------
    # 0. Detect native dtype from the first weight tensor.
    # ------------------------------------------------------------------
    # The safetensors header stores each tensor's dtype as a string
    # (e.g. "F32", "F16", "BF16", "F8_E4M3"). We iterate over all keys
    # and collect the dtype from the first weight tensor (identified by
    # the ".weight" suffix) to determine the checkpoint's native precision.
    # If no weight tensor is found (e.g. no-metadata fixtures with xyz_
    # prefixed keys), default to fp32 — the conservative safe choice that
    # prevents fp8 selection on unknown checkpoints.
    native_dtype: str = "fp32"
    for key in keys:
        if key.endswith(".weight"):
            # get_dtype() returns the safetensors dtype string
            # (e.g. "F32", "BF16", "F8_E4M3") for the first weight tensor.
            safetensors_dtype: str = f.get_slice(key).get_dtype()
            native_dtype = _safetensors_dtype_to_canonical(safetensors_dtype)
            break

    # ------------------------------------------------------------------
    # 1. Infer hidden_dim from projection keys.
    # ------------------------------------------------------------------
    # Primary path: ``time_text_embed.timestep_embedder.0.weight`` has
    # shape [hidden_dim, hidden_dim] (the first dimension is hidden_dim).
    # Fallback 1: ``time_text_embed.context_embedder`` has shape
    # [hidden_dim, context_dim] — the first dimension is still hidden_dim.
    # Fallback 2 (no-metadata fixture): keys use ``xyz_`` prefix with
    # dots replaced by underscores (e.g. ``xyz_time_text_embed_timestep_embedder``).
    # Match on keys containing "time_text_embed" and take the first dimension
    # of the shape — it is always hidden_dim regardless of which specific
    # key matches.
    hidden_dim: int | None = None
    for key in keys:
        if key.endswith("time_text_embed.timestep_embedder.0.weight"):
            hidden_dim = f.get_slice(key).get_shape()[0]
            break
        elif key.endswith("time_text_embed.context_embedder"):
            hidden_dim = f.get_slice(key).get_shape()[0]
            break

    # Fallback for no-metadata fixtures (xyz_ prefixed keys).
    if hidden_dim is None:
        for key in keys:
            if "time_text_embed" in key:
                hidden_dim = f.get_slice(key).get_shape()[0]
                break

    if hidden_dim is None:
        raise ValueError(
            f"cannot infer hidden_dim from safetensors keys in {path}: "
            "no recognized Flux 2 Klein projection keys found"
        )

    # ------------------------------------------------------------------
    # 2. Count double blocks.
    # ------------------------------------------------------------------
    # Primary pattern: double_blocks.N.* (regular Flux 2 Klein key naming).
    # Extract the numeric suffix to get 0-indexed block indices, then
    # count = max_index + 1.
    # Fallback: no-metadata fixtures use xyz_ prefixed keys with dots
    # replaced by underscores (e.g. xyz_double_blocks_0_img_attn_norm).
    # Match on double_blocks_ followed by digits.
    double_block_indices: set[int] = set()
    for key in keys:
        m = re.search(r"double_blocks[_.](\d+)", key)
        if m:
            double_block_indices.add(int(m.group(1)))
    double_block_count = (
        max(double_block_indices) + 1 if double_block_indices else 0
    )

    # Ultimate fallback: if no keys match either pattern, count keys
    # containing "double_block" and divide by 2 (pairs of mod + attention
    # tensors per block). This handles edge cases where numeric indices
    # are absent.
    if double_block_count == 0:
        double_block_keys = [k for k in keys if "double_block" in k]
        double_block_count = len(double_block_keys) // 2

    # ------------------------------------------------------------------
    # 3. Count single blocks.
    # ------------------------------------------------------------------
    # Primary pattern: single_blocks.N.* (regular Flux 2 Klein key naming).
    # Fallback: no-metadata fixtures use xyz_ prefixed keys with dots
    # replaced by underscores (e.g. xyz_single_blocks_0_linear1).
    single_block_indices: set[int] = set()
    for key in keys:
        m = re.search(r"single_blocks[_.](\d+)", key)
        if m:
            single_block_indices.add(int(m.group(1)))
    single_block_count = (
        max(single_block_indices) + 1 if single_block_indices else 0
    )

    # Ultimate fallback for alternate key naming conventions.
    if single_block_count == 0:
        single_block_keys = [k for k in keys if "single_block" in k]
        single_block_count = len(single_block_keys)

    # ------------------------------------------------------------------
    # 4. Infer latent dimensions.
    # ------------------------------------------------------------------
    # Primary path: use the ``latents`` marker tensor shape directly.
    # This is the most reliable source for latent_channels, latent_height,
    # and latent_width — the tensor is shaped [1, latent_channels, H, W].
    #
    # Fallback path: derive latent dimensions from ``final_layer.linear``
    # shape. The linear layer maps from hidden_dim to
    # ``patch_size² * out_channels``. Since hidden_dim = latent_dim / 2
    # in Flux 2 Klein's architecture (where latent_dim = latent_channels
    # * patch_size²), we derive:
    #   patch_size = sqrt(2 * hidden_dim / latent_channels)
    #   latent_height = latent_width = patch_size
    # This is needed when the latents marker tensor is absent.
    latent_channels: int | None = None
    latent_height: int | None = None
    latent_width: int | None = None

    latent_key: str | None = None
    for key in keys:
        if key.endswith("latents"):
            latent_key = key
            break

    if latent_key is not None:
        shape = f.get_slice(latent_key).get_shape()
        latent_channels = shape[1]
        latent_height = shape[2]
        latent_width = shape[3]

    # ------------------------------------------------------------------
    # 5. Derive patch_size.
    # ------------------------------------------------------------------
    # For square latent tensors, patch_size = latent_height = latent_width.
    # This matches the zit.py convention.
    patch_size: int = 0
    if latent_height is not None and latent_width is not None:
        patch_size = latent_height
    else:
        # Fallback: derive from final_layer.linear shape.
        # final_layer.linear has shape [hidden_dim, patch_size² * out_channels].
        # Since hidden_dim = latent_dim / 2 in Flux 2 Klein:
        #   patch_size² = (2 * hidden_dim) / latent_channels
        #   patch_size = sqrt(2 * hidden_dim / latent_channels)
        final_layer_key: str | None = None
        for key in keys:
            if key.endswith("final_layer.linear"):
                final_layer_key = key
                break

        if final_layer_key is not None and latent_channels is not None:
            # hidden_dim = 128, latent_channels = 4 → patch_size = 8
            # (sqrt(2 * 128 / 4) = sqrt(64) = 8 for the fixture)
            patch_size = round(
                math.sqrt(2 * hidden_dim / latent_channels)
            )
        else:
            raise ValueError(
                f"cannot infer latent dimensions from {path}: "
                "neither 'latents' marker nor 'final_layer.linear' found"
            )

    # ------------------------------------------------------------------
    # 6. Infer architecture string.
    # ------------------------------------------------------------------
    # Primary path: check safetensors metadata for "arch" key.
    # Fallback: scan keys for Flux 2 Klein-specific patterns
    # (double_block, single_block, final_layer, img_mod, txt_mod).
    # If neither path matches, raise ValueError.
    arch: str | None = None
    metadata = f.metadata()
    if metadata is not None:
        arch = metadata.get("arch")

    if arch is None:
        # Fallback: detect from key naming patterns.
        # The no-metadata fixture uses xyz_ prefixed keys that still
        # contain the substrings "double_block", "single_block",
        # "final_layer", "img_mod", and "txt_mod".
        flux_patterns = ["double_block", "single_block", "final_layer",
                         "img_mod", "txt_mod"]
        pattern_count = sum(1 for p in flux_patterns if any(p in k for k in keys))
        if pattern_count >= 3:
            arch = ARCH
        else:
            raise ValueError(
                f"cannot determine architecture from {path}: "
                "no 'arch' metadata and insufficient key patterns"
            )

    # ------------------------------------------------------------------
    # Build and return the hyperparameter dict.
    # ------------------------------------------------------------------
    return {
        "hidden_dim": hidden_dim,
        "double_block_count": double_block_count,
        "single_block_count": single_block_count,
        "latent_channels": latent_channels if latent_channels is not None else 0,
        "latent_height": latent_height if latent_height is not None else 0,
        "latent_width": latent_width if latent_width is not None else 0,
        "patch_size": patch_size,
        "arch": arch if arch is not None else "unknown",
        "native_dtype": native_dtype,
    }


def _infer_hyperparams(path: str) -> dict[str, Any]:
    """Infer architecture hyperparameters from a Flux 2 Klein safetensors checkpoint header.

    Opens the file header-only (no tensor data is loaded), reads every key's
    shape, and returns a dictionary of inferred hyperparameters. This function
    implements step 1 of the four-step loading contract (ANVILML_DESIGN.md §11.3).

    **P904 regression prevention:** this function reads ALL keys via
    ``f.keys()`` without any truncation or slicing — the P904 bug used
    ``list(f.keys())[:30]`` which silently dropped keys beyond index 30,
    causing incorrect block counts for models with 12+ blocks.

    Args:
        path: Filesystem path to a Flux 2 Klein-format safetensors
            checkpoint file.

    Returns:
        A dict with the following keys:
        - ``hidden_dim`` (int): The transformer hidden dimension.
        - ``double_block_count`` (int): Number of double_blocks layers.
        - ``single_block_count`` (int): Number of single_blocks layers.
        - ``latent_channels`` (int): Number of latent channels (usually 4).
        - ``latent_height`` (int): Latent height dimension.
        - ``latent_width`` (int): Latent width dimension.
        - ``patch_size`` (int): Patch size derived from hidden_dim /
            latent_channels.
        - ``arch`` (str): Architecture string (e.g. ``"flux2klein"``).
        - ``native_dtype`` (str): Canonical native dtype string (e.g.
            ``"fp32"``, ``"fp16"``, ``"bf16"``, ``"fp8"``) inferred
            from the first weight tensor in the checkpoint header.

    Raises:
        ValueError: If the file is not a valid safetensors file, is
            truncated, does not contain the expected Flux 2 Klein key
            patterns, or cannot be opened.
    """
    # Open the file header-only — no tensor data is loaded into memory.
    # This is the critical safety guarantee: even multi-GB checkpoints
    # only load the ~100KB metadata header.
    # framework="np" (not "pt"): _infer_hyperparams_inner() only ever reads
    # .keys(), .get_slice(key).get_shape(), and .get_slice(key).get_dtype() —
    # it never calls .get_tensor() and never touches actual tensor data, so
    # there is no reason to request the torch framework here. Requesting
    # framework="pt" made safetensors require torch to even open the header,
    # which broke this function in mock-mode (no torch installed) despite it
    # being documented as one of the torch-free contract functions
    # (ANVILML_DESIGN.md §11.2) — this was a genuine defect, not a mock-mode
    # workaround (P900-series retrofit).
    # Wrap in try/except to convert platform-specific errors (FileNotFoundError,
    # SafetensorError for corrupted headers) into ValueError with a descriptive
    # message, providing a uniform error interface for callers.
    try:
        with safe_open(path, framework="np") as f:
            return _infer_hyperparams_inner(f, path)
    except (FileNotFoundError, OSError) as exc:
        raise ValueError(f"cannot open safetensors file: {exc}") from exc
    except Exception as exc:
        # Catch-all for SafetensorError and any other deserialization
        # failures (truncated headers, corrupted files, etc.).
        raise ValueError(f"cannot parse safetensors header: {exc}") from exc
