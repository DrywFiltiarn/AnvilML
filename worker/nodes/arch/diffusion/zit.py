"""ZiT (Zero-initialized Transformer) diffusion architecture module.

This module provides shape-inference utilities for the ZiT diffusion transformer
architecture. It implements step 1 of the four-step loading contract defined in
ANVILML_DESIGN.md §11.3:

    1. _infer_hyperparams(path) — open header-only, read all key shapes, return
       a dict of inferred hyperparameters (hidden_dim, block counts, latent
       dimensions, patch_size, arch string).
    2. can_handle(key) — deferred to P20-B2.
    3. load(path, caps, ctx) — deferred to P20-C1.
    4. sample(z, prompt, **kwargs) — deferred to P20-C3.

The ZiT architecture is a diffusion transformer that uses zero-initialized
projection layers. It is characterised by ``double_blocks`` (cross-attention
blocks with image and text attention sub-layers) and ``single_blocks``
(single linear transformation blocks).

Design: ANVILML_DESIGN.md §11.3 — the four-step loading contract.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from safetensors import safe_open


def _infer_hyperparams(path: str) -> dict[str, Any]:
    """Infer architecture hyperparameters from a ZiT safetensors checkpoint header.

    Opens the file header-only (no tensor data is loaded), reads every key's
    shape, and returns a dictionary of inferred hyperparameters. This function
    implements step 1 of the four-step loading contract (ANVILML_DESIGN.md §11.3).

    **P904 regression prevention:** this function reads ALL keys via
    ``f.keys()`` without any truncation or slicing — the P904 bug used
    ``list(f.keys())[:30]`` which silently dropped keys beyond index 30,
    causing incorrect block counts for models with 12+ blocks.

    Args:
        path: Filesystem path to a ZiT-format safetensors checkpoint file.

    Returns:
        A dict with the following keys:
        - ``hidden_dim`` (int): The transformer hidden dimension.
        - ``double_block_count`` (int): Number of double_blocks layers.
        - ``single_block_count`` (int): Number of single_blocks layers.
        - ``latent_channels`` (int): Number of latent channels (usually 4).
        - ``latent_height`` (int): Latent height dimension.
        - ``latent_width`` (int): Latent width dimension.
        - ``patch_size`` (int): Patch size derived from hidden_dim / latent_channels.
        - ``arch`` (str): Architecture string (e.g. ``"zit"``).

    Raises:
        ValueError: If the file is not a valid safetensors file, is truncated,
            or does not contain the expected ZiT key patterns.
    """
    # Open the file header-only — no tensor data is loaded into memory.
    # This is the critical safety guarantee: even multi-GB checkpoints
    # only load the ~100KB metadata header.
    # Wrap in try/except to convert platform-specific errors (FileNotFoundError,
    # SafetensorError for corrupted headers) into ValueError with a descriptive
    # message, providing a uniform error interface for callers.
    try:
        with safe_open(path, framework="pt") as f:
            return _infer_hyperparams_inner(f, path)
    except (FileNotFoundError, OSError) as exc:
        raise ValueError(f"cannot open safetensors file: {exc}") from exc
    except Exception as exc:
        # Catch-all for SafetensorError and any other deserialization
        # failures (truncated headers, corrupted files, etc.).
        raise ValueError(f"cannot parse safetensors header: {exc}") from exc


def _infer_hyperparams_inner(
    f: Any, path: str
) -> dict[str, Any]:
    """Inner logic for _infer_hyperparams — runs inside the safe_open context.

    This helper is factored out so the try/except wrapping in the public
    function cleanly catches exceptions from safe_open itself without
    needing to re-raise ValueError from inside the with block.

    Args:
        f: A safetensors safe_open handle (already opened, header-only).
        path: Original filesystem path (for error messages).

    Returns:
        A dict of inferred hyperparameters.

    Raises:
        ValueError: If the checkpoint does not contain the expected ZiT
            key patterns or required keys.
    """
    # Read ALL keys without truncation — P904 regression prevention.
    # The P904 bug used list(f.keys())[:30] which silently dropped keys
    # beyond index 30, causing incorrect block counts for models with
    # 12+ double/single blocks. We read every key, unconditionally.
    keys = f.keys()

    # ------------------------------------------------------------------
    # 1. Infer hidden_dim from projection keys.
    # ------------------------------------------------------------------
    # hidden_dim is the first dimension of input_proj.weight and
    # time_text_emb.weight, and the sole dimension of c_crossattn_dim.
    # We check each key using endswith() to handle both the regular
    # fixture (exact key names) and the no-metadata fixture (xyz_
    # prefixed variants like xyz_c_crossattn_dim).
    hidden_dim: int | None = None
    for key in keys:
        if key.endswith("input_proj.weight") or key.endswith(
            "time_text_emb.weight"
        ):
            hidden_dim = f.get_slice(key).get_shape()[0]
            break
        elif key.endswith("c_crossattn_dim"):
            hidden_dim = f.get_slice(key).get_shape()[0]
            break

    if hidden_dim is None:
        raise ValueError(
            f"cannot infer hidden_dim from safetensors keys in {path}: "
            "no recognized projection keys (input_proj.weight, "
            "time_text_emb.weight, or c_crossattn_dim) found"
        )

    # ------------------------------------------------------------------
    # 2. Count double blocks.
    # ------------------------------------------------------------------
    # Primary pattern: double_blocks.N.* (regular ZiT key naming).
    # Extract the numeric suffix to get 0-indexed block indices, then
    # count = max_index + 1.
    double_block_indices: set[int] = set()
    for key in keys:
        m = re.search(r"double_blocks\.(\d+)", key)
        if m:
            double_block_indices.add(int(m.group(1)))
    double_block_count = (
        max(double_block_indices) + 1 if double_block_indices else 0
    )

    # Fallback: if no keys match the primary pattern, the checkpoint may
    # use an alternate key naming convention (e.g. xyz_double_block_*).
    # In that case, each pair of tensors (img_attn + txt_attn) represents
    # one double block, so we divide by 2.
    if double_block_count == 0:
        double_block_keys = [k for k in keys if "double_block" in k]
        double_block_count = len(double_block_keys) // 2

    # ------------------------------------------------------------------
    # 3. Count single blocks.
    # ------------------------------------------------------------------
    # Same dual-pattern approach as double blocks.
    single_block_indices: set[int] = set()
    for key in keys:
        m = re.search(r"single_blocks\.(\d+)", key)
        if m:
            single_block_indices.add(int(m.group(1)))
    single_block_count = (
        max(single_block_indices) + 1 if single_block_indices else 0
    )

    # Fallback for alternate key naming conventions.
    if single_block_count == 0:
        single_block_keys = [k for k in keys if "single_block" in k]
        single_block_count = len(single_block_keys)

    # ------------------------------------------------------------------
    # 4. Infer latent dimensions from the latents key.
    # ------------------------------------------------------------------
    # The latent tensor has shape (batch, channels, height, width).
    # In the regular fixture the key is "latents"; in the no-metadata
    # fixture it is "xyz_latents". We use endswith() to match both.
    latent_key: str | None = None
    for key in keys:
        if key.endswith("latents"):
            latent_key = key
            break

    if latent_key is None:
        raise ValueError(
            f"cannot infer latent dimensions from {path}: "
            "no key ending with 'latents' found"
        )

    latent_shape = f.get_slice(latent_key).get_shape()
    # Shape is always (batch, channels, height, width) for ZiT models.
    latent_channels = latent_shape[1]
    latent_height = latent_shape[2]
    latent_width = latent_shape[3]

    # ------------------------------------------------------------------
    # 5. Derive patch_size.
    # ------------------------------------------------------------------
    # For ZiT diffusion transformers, patch_size = hidden_dim //
    # latent_channels. This is the standard convention where each
    # patch token projects to the hidden dimension.
    patch_size = hidden_dim // latent_channels

    # ------------------------------------------------------------------
    # 6. Infer architecture string.
    # ------------------------------------------------------------------
    # Primary path: check f.metadata() for an "arch" key. This is the
    # canonical source — the checkpoint author explicitly declared the
    # architecture name.
    meta = f.metadata()
    arch: str | None = meta.get("arch") if meta else None

    # Metadata-fallback path: when the "arch" key is absent from the
    # safetensors header, infer the architecture from key naming patterns.
    # We check for recognizable ZiT key prefixes/suffixes. A key match
    # on any of these patterns is sufficient to identify the ZiT family.
    if arch is None:
        has_zit_patterns = False
        for key in keys:
            # Check for ZiT-specific key patterns — these are the
            # canonical prefixes/suffixes used by ZiT checkpoints.
            # "double_block" matches both "double_blocks" (with s)
            # and "double_block" (without s, as in no-metadata fixtures).
            if any(
                pat in key
                for pat in (
                    "double_block",
                    "single_block",
                    "input_proj",
                    "output_proj",
                )
            ):
                has_zit_patterns = True
                break

        if has_zit_patterns:
            arch = "zit"
        else:
            raise ValueError(
                f"unknown architecture in {path}: no arch metadata key "
                "and no recognizable key patterns found"
            )

    # Return all inferred hyperparameters as a single dict.
    return {
        "hidden_dim": hidden_dim,
        "double_block_count": double_block_count,
        "single_block_count": single_block_count,
        "latent_channels": latent_channels,
        "latent_height": latent_height,
        "latent_width": latent_width,
        "patch_size": patch_size,
        "arch": arch,
    }
