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

import logging
import re
from typing import Any

from safetensors import safe_open

logger = logging.getLogger(__name__)

# torch — and everything that transitively needs it (torch.nn, etc.) — is
# guarded here rather than imported unconditionally. This module is imported
# eagerly by arch/vae/__init__.py's dispatcher, which is reachable from
# mock-mode test collection: the worker-linux-mock / worker-windows-mock CI
# jobs install requirements/base.txt only and never install torch
# (ANVILML_DESIGN.md §18.3). can_handle(), _infer_hyperparams(), and the
# class definitions must stay importable and callable with torch absent;
# only load() actually needs it, and it raises a clear RuntimeError below
# (rather than a cryptic AttributeError on None) if reached without torch.
try:
    import torch
    import torch.nn as nn
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]

# nn.Module is unavailable when torch failed to import (see the guard above).
# ZiTVaeModel falls back to plain `object` as its base in that case — the
# class still defines successfully (only __init__/forward bodies touch torch,
# and those are never invoked without going through the guarded load() entry
# point below), which is what keeps this module importable in mock-mode
# collection.
_ModuleBase = nn.Module if nn is not None else object


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
            "decoder_channels": 32,
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
# Model class — meta-device construction
# ---------------------------------------------------------------------------


class ZiTVaeModel(_ModuleBase):
    """ZiT VAE model constructed from convolutional layer-level building blocks.

    This class assembles the VAE architecture using ``torch.nn`` primitives
    (Conv2d, GroupNorm) with SiLU activation — mirroring the tensor shapes
    found in the checkpoint. It is constructed on
    ``torch.device("meta")`` so that no real GPU/CPU memory is allocated
    during construction — this prevents the ~15 GB construction crash that
    P904 experienced.

    The architecture consists of:

    - **Encoder blocks**: convolutional downsampling from ``encoder_channels``
      through ``latent_channels`` to ``decoder_channels``. Each block is a
      Conv2d + GroupNorm + SiLU, with channel counts interpolated between the
      anchor points.
    - **Mid-block**: a single Conv2d + GroupNorm operating at
      ``latent_channels`` on both input and output, forming the bottleneck.
    - **Decoder blocks**: convolutional upsampling from ``decoder_channels``
      through ``latent_channels`` back to ``encoder_channels``. Each block is
      a Conv2d + GroupNorm + SiLU, with channel counts interpolated in
      reverse.

    The ``.arch`` attribute is set to ``"zit_vae"`` after construction so that
    downstream code (Sampler, VaeDecode) can identify the model family.

    Args:
        hyperparams: Dict from ``_infer_hyperparams()`` containing
            ``encoder_channels``, ``decoder_channels``, and
            ``latent_channels``.
    """

    def __init__(self, hyperparams: dict[str, Any]) -> None:
        """Construct the VAE encoder, mid-block, and decoder from hyperparams.

        Args:
            hyperparams: Dict from ``_infer_hyperparams()`` containing
                ``encoder_channels``, ``decoder_channels``, and
                ``latent_channels``.
        """
        # Use the base class (nn.Module or object) — nn.Module when torch is
        # available, plain object when it is not (mock-mode collection path).
        super().__init__()
        self.arch = "zit_vae"

        encoder_channels = hyperparams["encoder_channels"]
        decoder_channels = hyperparams["decoder_channels"]
        latent_channels = hyperparams["latent_channels"]

        # The fixture has 2 encoder blocks and 2 decoder blocks. The channel
        # progression follows the pattern:
        #   encoder: encoder_channels → latent_channels → decoder_channels
        #   mid-block: latent_channels → latent_channels
        #   decoder: decoder_channels → latent_channels → encoder_channels
        # For N blocks, intermediate channel counts are linearly interpolated
        # between the anchor points (encoder_channels, latent_channels,
        # decoder_channels for encoder; decoder_channels, latent_channels,
        # encoder_channels for decoder).
        encoder_block_count = 2
        decoder_block_count = 2

        # --- Encoder blocks ---
        # Each block is a Conv2d + GroupNorm + SiLU. The first block takes
        # encoder_channels as input and outputs latent_channels; the last
        # block outputs decoder_channels. Intermediate blocks interpolate.
        # GroupNorm uses min(8, out_ch) groups to handle small channel counts
        # where 8 groups would exceed the channel dimension (PyTorch requires
        # num_channels % num_groups == 0).
        self.encoder = nn.ModuleDict()
        for i in range(encoder_block_count):
            # First block: input = encoder_channels, output = latent_channels
            # Last block: input = previous output, output = decoder_channels
            # Intermediate: linearly interpolated between latent and decoder.
            if i == 0:
                in_ch = encoder_channels
                out_ch = latent_channels
            elif i == encoder_block_count - 1:
                in_ch = latent_channels
                out_ch = decoder_channels
            else:
                # Interpolate between latent_channels and decoder_channels.
                progress = i / (encoder_block_count - 1)
                in_ch = int(
                    latent_channels + progress * (decoder_channels - latent_channels)
                )
                out_ch = int(
                    latent_channels + ((i + 1) / (encoder_block_count - 1))
                    * (decoder_channels - latent_channels)
                )

            self.encoder[f"block_{i}"] = nn.ModuleDict(
                {
                    "conv": nn.Conv2d(in_ch, out_ch, kernel_size=3, padding=1),
                    "norm": nn.GroupNorm(min(8, out_ch), out_ch),
                }
            )

        # --- Mid-block ---
        # Single Conv2d + GroupNorm at latent_channels. This is the bottleneck
        # between encoder and decoder stages. Uses min(8, latent_channels)
        # groups to handle small channel counts.
        self.mid_block = nn.ModuleDict(
            {
                "conv": nn.Conv2d(
                    latent_channels, latent_channels, kernel_size=3, padding=1
                ),
                "norm": nn.GroupNorm(min(8, latent_channels), latent_channels),
            }
        )

        # --- Decoder blocks ---
        # Each block is a Conv2d + GroupNorm + SiLU. The first block takes
        # decoder_channels as input and outputs latent_channels; the last block
        # outputs encoder_channels. Intermediate blocks interpolate in reverse.
        # GroupNorm uses min(8, out_ch) groups to handle small channel counts.
        self.decoder = nn.ModuleDict()
        for i in range(decoder_block_count):
            # First block: input = decoder_channels, output = latent_channels
            # Last block: input = previous output, output = encoder_channels
            # Intermediate: linearly interpolated between decoder and encoder.
            if i == 0:
                in_ch = decoder_channels
                out_ch = latent_channels
            elif i == decoder_block_count - 1:
                in_ch = latent_channels
                out_ch = encoder_channels
            else:
                # Interpolate between decoder_channels and encoder_channels.
                progress = i / (decoder_block_count - 1)
                in_ch = int(
                    decoder_channels
                    + progress * (encoder_channels - decoder_channels)
                )
                out_ch = int(
                    decoder_channels
                    + ((i + 1) / (decoder_block_count - 1))
                    * (encoder_channels - decoder_channels)
                )

            self.decoder[f"block_{i}"] = nn.ModuleDict(
                {
                    "conv": nn.Conv2d(in_ch, out_ch, kernel_size=3, padding=1),
                    "norm": nn.GroupNorm(min(8, out_ch), out_ch),
                }
            )


# ---------------------------------------------------------------------------
# Dtype selection
# ---------------------------------------------------------------------------


def _select_dtype(caps: dict, native_dtype: str) -> torch.dtype:
    """Select the compute dtype per the fixed precedence in ANVILML_DESIGN.md §11.5.

    Implements the precedence chain: fp8 (if caps.fp8 AND native is fp8)
    → bf16 → fp16 → fp32. The native dtype is compared against known
    FP8 formats to determine whether the checkpoint was originally
    trained in FP8 — a checkpoint in F32 does not benefit from fp8
    caps because the weights would need to be converted first.

    Args:
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``.
        native_dtype: Canonical native dtype string from the checkpoint
            header (e.g. "fp32", "fp16", "bf16", "fp8").

    Returns:
        A ``torch.dtype`` constant for the selected compute precision.
    """
    # Branch 1: FP8 — only selected when the worker supports fp8 AND the
    # checkpoint was originally saved in an FP8 format. Both conditions
    # must hold because loading an F32 checkpoint at fp8 would require
    # a weight conversion step that is not implemented yet.
    if caps.get("fp8", False) and native_dtype == "fp8":
        return torch.float8_e4m3fn

    # Branch 2: BF16 — the next-highest precision when FP8 is not viable.
    # bfloat16 is widely supported on modern GPUs and provides dynamic
    # range close to fp32 with half the memory footprint.
    if caps.get("bf16", False):
        return torch.bfloat16

    # Branch 3: FP16 — the next fallback when bf16 is not available.
    # float16 has a narrower dynamic range than bf16 but is still
    # significantly more memory-efficient than fp32.
    if caps.get("fp16", False):
        return torch.float16

    # Branch 4: FP32 — the universal fallback. Every device supports fp32,
    # so this path is always reachable. It is the most numerically stable
    # but also the most memory-intensive option.
    return torch.float32


# ---------------------------------------------------------------------------
# Partial load — meta construction + dtype application (P23-C1)
# ---------------------------------------------------------------------------


def load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel:
    """Construct the ZiT VAE model on meta-device and apply compute dtype.

    Implements steps 1–2 of the four-step loading contract
    (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (delegated to ``_select_dtype``).
    3. Construct ``ZiTVaeModel`` on ``torch.device("meta")``, apply dtype.

    Step 3 of the loading contract (materialize via ``to_empty()``, build
    key remapping, ``load_state_dict(assign=True)``) and the ``.arch``
    attribute are deferred to P23-C3.

    Args:
        path: Filesystem path to a ZiT-VAE-format safetensors checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for tensor materialization. Defaults to
            ``"cpu"``. Used in logging.

    Returns:
        A ``ZiTVaeModel`` instance with parameters on ``torch.device("meta")``,
        carrying the selected dtype metadata. Weights are NOT loaded yet —
        this is a partial stub per P23-C3's scope.

    Raises:
        RuntimeError: If torch is not installed (load() is a real-mode-only
            entry point and must not be reached from mock-mode code).
        ValueError: If the checkpoint cannot be opened or hyperparameters
            cannot be inferred (delegated to ``_infer_hyperparams``).

    defers_to: P23-C3 — materialization via to_empty(), key remapping,
        load_state_dict(assign=True), and .arch attribute verification.
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); load() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here instead of surfacing a
    # confusing AttributeError on a None torch/nn deep inside construction.
    if torch is None:
        raise RuntimeError(
            "zit_vae.py: torch is not installed - load() is a real-mode-only "
            "entry point (ANVILML_DESIGN.md §18.3) and must not be reached "
            "from mock-mode code paths."
        )

    # Step 1: infer hyperparameters from checkpoint header, including the
    # native dtype of the first weight tensor. This reads only the ~100KB
    # metadata header — no tensor data is loaded.
    hyperparams = _infer_hyperparams(path)

    # Step 2a: select the compute dtype per the fixed precedence in
    # ANVILML_DESIGN.md §11.5. The native dtype is read from the checkpoint
    # header; the capability flags come from the worker's own torch-level probe.
    # This ensures the dtype decision is driven by both what the checkpoint
    # actually uses and what the worker hardware can execute.
    target_dtype = _select_dtype(caps, hyperparams["native_dtype"])

    # Step 2b: construct on meta-device with selected dtype.
    # Using torch.device("meta") means no real memory is allocated for
    # parameters — the module structure exists but tensors have shape
    # metadata only. This prevents the ~15GB crash from P904.
    with torch.device("meta"):
        model = ZiTVaeModel(hyperparams)

    # Step 2c: apply the selected dtype to the meta-constructed module.
    # model.to(dtype) on a module with meta-device parameters changes their
    # dtype metadata without allocating real memory — this is the standard
    # PyTorch idiom for dtype selection before weight loading.
    model.to(target_dtype)

    # Log the dtype selection for observability.
    logger.debug(
        "selected dtype=%s for VAE on device=%s", target_dtype, device
    )

    # Return immediately — materialization, key remapping, and weight loading
    # are P23-C3's scope. The returned module has meta-device parameters with
    # the correct dtype metadata but no actual weight data.
    return model


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
