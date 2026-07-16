"""ZiT VAE architecture module — shape inference from safetensors header.

This module implements step 1 of the four-step loading contract
(`ANVILML_DESIGN.md §11.3`) for the ZiT-compatible VAE architecture family.
It reads only the safetensors header of a ZiT-VAE checkpoint, infers encoder
and decoder channel counts, latent channel count, architecture string, and
native dtype, and returns them as a dict.

The module is genuinely independent of `zit.py`'s diffusion shape-inference
logic (`ANVILML_DESIGN.md §11.4`) — each arch family has its own formula.

This file is the first of the ZiT VAE arch module tasks in Phase 23.
"""

from __future__ import annotations

import re
from typing import Any

from safetensors import safe_open


# Architecture identifier — used by can_handle() (P23-B2) and by dispatch.
ARCH: str = "zit_vae"

# ---------------------------------------------------------------------------
# Safetensors dtype mapping
# ---------------------------------------------------------------------------


def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str:
    """Map a safetensors dtype string to its canonical lowercase form.

    Recognised mappings:

    +----------------+------------------+
    | safetensors    | canonical        |
    +----------------+------------------+
    | ``"F32"``      | ``"fp32"``       |
    | ``"F16"``      | ``"fp16"``       |
    | ``"BF16"``     | ``"bf16"``       |
    | ``"F8_E4M3"``  | ``"fp8"``        |
    | ``"F8_E5M2"``  | ``"fp8"``        |
    +----------------+------------------+

    Any unrecognised dtype falls through to ``"fp32"``.

    Args:
        safetensors_dtype: The dtype string from
            ``safetensors.SafeTensor.get_slice().get_dtype()``.

    Returns:
        A canonical lowercase dtype string.
    """
    mapping: dict[str, str] = {
        "F32": "fp32",
        "F16": "fp16",
        "BF16": "bf16",
        "F8_E4M3": "fp8",
        "F8_E5M2": "fp8",
    }
    return mapping.get(safetensors_dtype, "fp32")


# ---------------------------------------------------------------------------
# Inner inference logic (runs inside the safe_open context)
# ---------------------------------------------------------------------------


def _infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]:
    """Infer hyperparameters from the safetensors header of a ZiT-VAE checkpoint.

    This function runs inside the ``safe_open`` context manager and performs
    five inferences:

    1. **Native dtype** — iterates all keys for the first ``.weight``-suffixed
       key, reads its dtype via ``get_slice().get_dtype()``, and maps it
       through ``_safetensors_dtype_to_canonical()``. Defaults to ``"fp32"``
       if no weight tensor is found.

    2. **Encoder channel count** — finds the first key matching the pattern
       ``encoder.blocks.N.conv.weight`` and extracts ``shape[0]`` (out_channels).
       If no such key exists, falls back to the ``xyz_encoder_block*conv``
       pattern and extracts ``shape[0]``.

    3. **Decoder channel count** — same logic as encoder but with
       ``decoder.blocks.N.conv.weight`` / ``xyz_decoder_block*conv``.

    4. **Latent channel count** — finds the key ending with ``latents``
       (or ``xyz_latents`` for the no-metadata fixture) and extracts
       ``shape[1]`` from its shape.

    5. **Architecture string** — primary path reads
       ``f.metadata().get("arch")``; fallback checks for recognisable VAE
       key patterns (``encoder.blocks``, ``decoder.blocks``, ``mid_block``)
       and sets ``arch = "zit_vae"`` if found.

    P904 regression prevention: reads ALL keys via ``f.keys()`` without
    truncation — never ``list(f.keys())[:N]``.

    Args:
        f: An open ``safetensors.SafeOpen`` handle (the context manager
            from ``safe_open(path, framework="np")``).
        path: The original file path, used in error messages.

    Returns:
        A dict with keys:
        ``encoder_channels``, ``decoder_channels``, ``latent_channels``,
        ``arch``, ``native_dtype``.

    Raises:
        ValueError: If the safetensors header cannot be parsed (e.g.
            corrupted file). This is raised by the caller wrapping
            ``safe_open`` — the inner function itself should not raise
            beyond exceptions from safetensors internals.
    """
    # Read ALL keys — P904 regression prevention: never truncate.
    keys = f.keys()  # type: ignore[union-attr]

    # --- Native dtype detection ---
    # Iterate keys for the first .weight-suffixed key to detect dtype.
    native_dtype: str = "fp32"  # default if no weight tensor found
    for key in keys:
        if key.endswith(".weight"):
            # Read the dtype from this weight tensor's slice metadata.
            # get_slice() does not load data — it only reads header info.
            slice_info = f.get_slice(key)  # type: ignore[union-attr]
            raw_dtype = slice_info.get_dtype()
            native_dtype = _safetensors_dtype_to_canonical(raw_dtype)
            break

    # --- Encoder channel count ---
    encoder_channels = 0
    # Primary pattern: encoder.blocks.N.conv.weight (out_channels = shape[0])
    encoder_pattern = re.compile(r"^encoder\.blocks\.\d+\.conv\.weight$")
    for key in keys:
        if encoder_pattern.match(key):
            slice_info = f.get_slice(key)  # type: ignore[union-attr]
            encoder_channels = slice_info.get_shape()[0]
            break

    # Fallback: xyz_encoder_block*conv pattern (no .weight suffix in no-meta fixture)
    if encoder_channels == 0:
        xyz_encoder_pattern = re.compile(r"^xyz_encoder_block(\d+)_conv$")
        for key in keys:
            if xyz_encoder_pattern.match(key):
                slice_info = f.get_slice(key)  # type: ignore[union-attr]
                encoder_channels = slice_info.get_shape()[0]
                break

    # --- Decoder channel count ---
    decoder_channels = 0
    # Primary pattern: decoder.blocks.N.conv.weight
    decoder_pattern = re.compile(r"^decoder\.blocks\.\d+\.conv\.weight$")
    for key in keys:
        if decoder_pattern.match(key):
            slice_info = f.get_slice(key)  # type: ignore[union-attr]
            decoder_channels = slice_info.get_shape()[0]
            break

    # Fallback: xyz_decoder_block*conv pattern
    if decoder_channels == 0:
        xyz_decoder_pattern = re.compile(r"^xyz_decoder_block(\d+)_conv$")
        for key in keys:
            if xyz_decoder_pattern.match(key):
                slice_info = f.get_slice(key)  # type: ignore[union-attr]
                decoder_channels = slice_info.get_shape()[0]
                break

    # --- Latent channel count ---
    latent_channels = 0
    # Primary pattern: key ending with "latents"
    for key in keys:
        if key.endswith("latents"):
            slice_info = f.get_slice(key)  # type: ignore[union-attr]
            latent_channels = slice_info.get_shape()[1]
            break

    # --- Architecture detection ---
    # Primary path: read "arch" from safetensors metadata.
    arch: str | None = None
    try:
        metadata = f.metadata()  # type: ignore[union-attr]
        if metadata is not None:
            arch = metadata.get("arch")
    except (AttributeError, TypeError):
        # Some safetensors versions may not support .metadata() — fall back.
        pass

    # Fallback: detect from key naming patterns.
    if arch is None:
        has_encoder = any(re.match(r"^encoder\.blocks", k) for k in keys)
        has_decoder = any(re.match(r"^decoder\.blocks", k) for k in keys)
        has_mid_block = any(re.match(r"^mid_block", k) for k in keys)
        if has_encoder and has_decoder and has_mid_block:
            arch = "zit_vae"

    # If arch is still None after all fallbacks, default to "zit_vae".
    if arch is None:
        arch = "zit_vae"

    return {
        "encoder_channels": encoder_channels,
        "decoder_channels": decoder_channels,
        "latent_channels": latent_channels,
        "arch": arch,
        "native_dtype": native_dtype,
    }


# ---------------------------------------------------------------------------
# Public entry point
# ---------------------------------------------------------------------------


def _infer_hyperparams(path: str) -> dict[str, Any]:
    """Infer hyperparameters from a ZiT-VAE checkpoint's safetensors header.

    Opens the safetensors file header-only (no tensor data loaded) using
    ``framework="np"`` — this avoids requiring torch at import time and
    keeps the function safe for mock-mode collection.

    The function reads ALL keys from the header (P904 regression prevention),
    infers encoder/decoder/latent channel counts, native dtype, and
    architecture string, and returns them as a dict.

    Example return dict:

    .. code-block:: python

        {
            "encoder_channels": 16,
            "decoder_channels": 16,
            "latent_channels": 4,
            "arch": "zit_vae",
            "native_dtype": "fp32",
        }

    Args:
        path: File system path to the ZiT-VAE safetensors checkpoint.

    Returns:
        A dict with keys ``encoder_channels``, ``decoder_channels``,
        ``latent_channels``, ``arch``, and ``native_dtype``.

    Raises:
        ValueError: If the file cannot be opened (missing or unreadable)
            or if the safetensors header is corrupted/unparseable.
    """
    try:
        with safe_open(path, framework="np") as f:  # type: ignore[arg-type]
            return _infer_hyperparams_inner(f, path)
    except (FileNotFoundError, OSError) as exc:
        # File missing or permission error — wrap with a descriptive message.
        raise ValueError(f"cannot open safetensors file: {exc}") from exc
    except Exception as exc:
        # Catch-all for SafetensorError or other parsing failures.
        raise ValueError(f"cannot parse safetensors header: {exc}") from exc


# ---------------------------------------------------------------------------
# Dispatch registration
# ---------------------------------------------------------------------------


def can_handle(key: str) -> bool:
    """Confirm this module handles the given architecture key.

    The dispatcher passes the architecture string (from safetensors
    metadata or path fallback) as *key*. This function returns True
    only when the key matches this module's canonical identifier.

    Args:
        key: Architecture string to check, e.g. ``"zit_vae"`` or
            ``"flux2_vae"``.

    Returns:
        ``True`` if *key* equals ``"zit_vae"``, ``False`` otherwise.
    """
    return key == ARCH
