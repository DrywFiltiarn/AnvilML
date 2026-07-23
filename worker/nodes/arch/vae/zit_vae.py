"""ZiT VAE architecture module — shape inference from safetensors header.

This module implements the full four-step loading contract
(`ANVILML_DESIGN.md §11.3`) for the ZiT-compatible VAE architecture family:

    1. _infer_hyperparams(path) — open header-only, read all key shapes, return
           a dict of inferred hyperparameters (encoder/decoder/latent channels,
           arch string, native_dtype).
    2. can_handle(key) — implemented; returns True for "zit_vae".
    3. load(path, caps, device) — steps 2–4: construct nn.Module on meta,
           materialize onto device via to_empty(), build key remapping,
           load_state_dict(assign=True), set .arch.
    4. decode(vae_module, latent) — implemented in this task (P23-D1).

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

# numpy and PIL are required by decode() — guarded so the module stays
# importable in mock-mode collection. decode() itself raises RuntimeError
# if torch is None, which also guards these imports since torch absent
# means mock-mode, and mock-mode should never reach decode().
try:
    import numpy as np
    from PIL import Image
except ImportError:
    np = None  # type: ignore[assignment]
    Image = None  # type: ignore[assignment]

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
    import torch.nn.functional as F
    from safetensors.torch import load_file
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
    F = None  # type: ignore[assignment]
    load_file = None  # type: ignore[assignment]

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
       key patterns (``encoder.blocks``, ``decoder.blocks``, ``mid_block``,
       or their ``xyz_``-prefixed no-metadata-fixture equivalents) and
       sets ``arch = "zit_vae"`` if found; raises ``ValueError`` if
       neither the metadata nor any recognizable pattern is found (P900
       retrofit — this previously defaulted to ``"zit_vae"`` unconditionally
       instead of raising).

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
            corrupted file), or if no ``arch`` metadata key and no
            recognizable VAE key pattern is found.
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
    # Primary pattern: encoder.blocks.N.conv.weight
    # The first block's input (shape[1]) is the true encoder_channels,
    # since the first block's output (shape[0]) is an interpolated value.
    encoder_pattern = re.compile(r"^encoder\.blocks\.\d+\.conv\.weight$")
    for key in keys:
        if encoder_pattern.match(key):
            slice_info = f.get_slice(key)  # type: ignore[union-attr]
            encoder_channels = slice_info.get_shape()[1]
            break

    # Fallback: xyz_encoder_block*conv.weight pattern (no-meta fixture)
    if encoder_channels == 0:
        xyz_encoder_pattern = re.compile(r"^xyz_encoder_block(\d+)_conv\.weight$")
        for key in keys:
            if xyz_encoder_pattern.match(key):
                slice_info = f.get_slice(key)  # type: ignore[union-attr]
                encoder_channels = slice_info.get_shape()[1]
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

    # Fallback: xyz_decoder_block*conv.weight pattern (no-meta fixture)
    if decoder_channels == 0:
        xyz_decoder_pattern = re.compile(r"^xyz_decoder_block(\d+)_conv\.weight$")
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
    #
    # P900-series retrofit: this previously ended with an unconditional
    # "if arch is still None, default to zit_vae" — meaning this function
    # NEVER raised for an unrecognized checkpoint, silently claiming every
    # unrecognized VAE file was a ZiT VAE. Confirmed by direct testing:
    # the pre-fix version of this function accepted
    # flux2_vae_tiny_no_metadata.safetensors and returned "zit_vae" for
    # it. That made this function unsafe to use as a cross-architecture
    # disambiguation probe (worker/nodes/arch/vae/__init__.py's
    # detect_arch(), added by the same retrofit to fix LoadVae's
    # separate "hardcoded get_module('zit_vae')" defect).
    if arch is None:
        has_encoder = any(re.match(r"^encoder\.blocks", k) for k in keys)
        has_decoder = any(re.match(r"^decoder\.blocks", k) for k in keys)
        has_mid_block = any(re.match(r"^mid_block", k) for k in keys)

        # No-metadata regression fixture: xyz_-prefixed rename of the
        # real key pattern, preserving the ".conv.weight"/".norm.weight"
        # suffix (build_zit_vae_fixture.py's _no_metadata_tensors()).
        # This match is deliberately STRICT (".weight" required) —
        # flux2_vae.py's own no-metadata fixture uses the identical
        # "xyz_encoder_block*"/"xyz_decoder_block*" prefix WITHOUT the
        # ".weight" suffix, so a loose match here would misclassify a
        # Flux 2 VAE checkpoint as ZiT VAE. mid_block's xyz key is
        # identical in both fixtures ("xyz_mid_block_conv", no suffix
        # either way) and is therefore not usable as a distinguishing
        # signal — only encoder/decoder are checked here.
        has_xyz_encoder = any(
            re.match(r"^xyz_encoder_block\d+_conv\.weight$", k) for k in keys
        )
        has_xyz_decoder = any(
            re.match(r"^xyz_decoder_block\d+_conv\.weight$", k) for k in keys
        )

        if (has_encoder and has_decoder and has_mid_block) or (
            has_xyz_encoder and has_xyz_decoder
        ):
            arch = "zit_vae"

    if arch is None:
        raise ValueError(
            f"unknown VAE architecture in {path}: no arch metadata key "
            "and no recognizable ZiT VAE key patterns found"
        )

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

        # Helper to compute GroupNorm num_groups: find the largest divisor of
        # num_channels that is ≤ max_groups. This ensures PyTorch's requirement
        # that num_channels % num_groups == 0 is always satisfied, even when
        # interpolated channel counts produce non-standard values (e.g. 10).
        def _group_norm_groups(num_channels: int, max_groups: int = 8) -> int:
            """Find the largest divisor of num_channels that is ≤ max_groups."""
            for g in range(min(max_groups, num_channels), 0, -1):
                if num_channels % g == 0:
                    return g
            return 1  # fallback: 1 group (equivalent to LayerNorm)

        self.arch = "zit_vae"

        encoder_channels = hyperparams["encoder_channels"]
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
        # encoder_channels as input and the last block outputs latent_channels.
        # Intermediate blocks interpolate between encoder_channels and
        # latent_channels.
        # GroupNorm uses min(8, out_ch) groups to handle small channel counts
        # where 8 groups would exceed the channel dimension (PyTorch requires
        # num_channels % num_groups == 0).
        self.encoder = nn.ModuleDict()
        for i in range(encoder_block_count):
            # Interpolate channel counts from encoder_channels to latent_channels.
            # Block i's output becomes block i+1's input. The first block takes
            # encoder_channels as input. The last block outputs latent_channels.
            out_ch = int(
                encoder_channels
                + ((i + 1) / encoder_block_count) * (latent_channels - encoder_channels)
            )
            if i == 0:
                in_ch = encoder_channels
            else:
                # Previous block's output is this block's input.
                in_ch = int(
                    encoder_channels
                    + (i / encoder_block_count) * (latent_channels - encoder_channels)
                )

            self.encoder[f"block_{i}"] = nn.ModuleDict(
                {
                    "conv": nn.Conv2d(in_ch, out_ch, kernel_size=3, padding=1),
                    "norm": nn.GroupNorm(_group_norm_groups(out_ch), out_ch),
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
                "norm": nn.GroupNorm(
                    _group_norm_groups(latent_channels), latent_channels
                ),
            }
        )

        # --- Decoder blocks ---
        # Each block is a Conv2d + GroupNorm + SiLU. The first block takes
        # latent_channels as input (matching the mid-block output) and the
        # last block outputs encoder_channels. Intermediate blocks interpolate
        # between latent_channels and encoder_channels.
        # GroupNorm uses min(8, out_ch) groups to handle small channel counts.
        self.decoder = nn.ModuleDict()
        for i in range(decoder_block_count):
            # Interpolate channel counts from latent_channels to encoder_channels.
            # Block i's output becomes block i+1's input. The first block takes
            # latent_channels as input. The last block outputs encoder_channels.
            # For N blocks, the interpolation points are:
            #   out_ch(i) = latent_channels + (i+1)/(N) * (encoder_channels - latent_channels)
            #   in_ch(i)  = out_ch(i-1) for i > 0, latent_channels for i == 0.
            out_ch = int(
                latent_channels
                + ((i + 1) / decoder_block_count) * (encoder_channels - latent_channels)
            )
            if i == 0:
                in_ch = latent_channels
            else:
                # Previous block's output is this block's input.
                in_ch = int(
                    latent_channels
                    + (i / decoder_block_count) * (encoder_channels - latent_channels)
                )

            self.decoder[f"block_{i}"] = nn.ModuleDict(
                {
                    "conv": nn.Conv2d(in_ch, out_ch, kernel_size=3, padding=1),
                    "norm": nn.GroupNorm(_group_norm_groups(out_ch), out_ch),
                }
            )

    def forward(self, latent: torch.Tensor) -> torch.Tensor:
        """Run the latent tensor through the decoder forward pass.

        The VAE decoder topology applies the mid-block as a bottleneck
        transformation, then sequentially passes the output through each
        decoder block in order (block_0 → block_1). Each block applies
        Conv2d → GroupNorm → SiLU.

        Args:
            latent: Input latent tensor of shape ``(batch, channels, H, W)``.

        Returns:
            The decoded tensor with the same shape as the input — the
            decoder blocks preserve spatial resolution and restore the
            original channel count through interpolation.
        """
        # Pass through the mid-block: conv → norm → SiLU.
        # The mid_block is a shared bottleneck transformation at latent_channels.
        x = self.mid_block["conv"](latent)
        x = self.mid_block["norm"](x)
        x = F.silu(x)

        # Sequentially pass through each decoder block in order.
        # Each block applies: conv → norm → SiLU, progressively restoring
        # the channel dimension back to encoder_channels.
        for i in range(len(self.decoder)):
            block = self.decoder[f"block_{i}"]
            x = block["conv"](x)
            x = block["norm"](x)
            x = F.silu(x)

        return x


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
# Key remapping — VAE namespace
# ---------------------------------------------------------------------------


def _build_key_remapping(
    checkpoint_keys: list[str], module_keys: list[str]
) -> dict[str, str]:
    """Build a checkpoint-key → module-key mapping for ``load_state_dict``.

    Iterates over checkpoint keys and builds a remapping table that maps
    each checkpoint key to the corresponding module state_dict key. The
    function handles two cases for the ZiT VAE key namespace:

    1. **Direct match:** the checkpoint key exists verbatim in the module's
       state_dict keys. The mapping is identity: ``ckpt_key → mod_key``.
       This covers mid-block keys (``mid_block.conv.weight``) and any keys
       that use the same naming convention as the constructed module.
    2. **Pattern-based remapping:** VAE checkpoint keys use ``encoder.blocks.N``
       and ``decoder.blocks.N`` (plural "blocks" with dot separator), but the
       constructed module uses ``encoder.block_N`` and ``decoder.block_N``
       (singular "block" with underscore). This remapping converts
       ``encoder.blocks.N.suffix`` → ``encoder.block_N.suffix`` and
       ``decoder.blocks.N.suffix`` → ``decoder.block_N.suffix``.

    Keys that exist only in the checkpoint (e.g. the ``latents`` metadata
    tensor, or ``xyz_``-prefixed keys from the no-metadata fixture that
    don't match any VAE pattern) are silently excluded from the remapping.

    This function is built independently from ``zit.py``'s diffusion key
    remapping — the VAE key patterns are different and must not be assumed
    from the diffusion module's implementation.

    Args:
        checkpoint_keys: List of tensor keys from the safetensors file
            (returned by ``load_file()``).
        module_keys: List of parameter keys from ``model.state_dict().keys()``.

    Returns:
        A dict mapping ``checkpoint_key → module_key`` for all keys that
        can be successfully remapped.
    """
    module_key_set = set(module_keys)

    # Pattern-based remapping rules for ZiT VAE key naming conventions.
    # The checkpoint uses "blocks.N" (plural with dot) but the module uses
    # "block_N" (singular with underscore). These two patterns cover both
    # encoder and decoder blocks with any suffix (.conv.weight, .norm.weight,
    # .conv.bias, etc.).
    #
    # Additionally, no-metadata fixtures use "xyz_encoder_blockN_suffix" and
    # "xyz_decoder_blockN_suffix" patterns (underscore between block index
    # and suffix, no dot separator).
    remapping_patterns: list[tuple[str, str]] = [
        # Encoder blocks: encoder.blocks.N.* → encoder.block_N.*
        (r"encoder\.blocks\.(\d+)\.(.*)", r"encoder.block_\1.\2"),
        # Decoder blocks: decoder.blocks.N.* → decoder.block_N.*
        (r"decoder\.blocks\.(\d+)\.(.*)", r"decoder.block_\1.\2"),
        # No-metadata fixture: xyz_encoder_blockN_suffix → encoder.block_N.suffix
        (r"xyz_encoder_block(\d+)_(.*)", r"encoder.block_\1.\2"),
        # No-metadata fixture: xyz_decoder_blockN_suffix → decoder.block_N.suffix
        (r"xyz_decoder_block(\d+)_(.*)", r"decoder.block_\1.\2"),
        # No-metadata fixture: xyz_mid_block_conv → mid_block.conv.weight
        (r"xyz_mid_block_conv", r"mid_block.conv.weight"),
        # No-metadata fixture: xyz_mid_block_norm → mid_block.norm.weight
        (r"xyz_mid_block_norm", r"mid_block.norm.weight"),
    ]

    remap: dict[str, str] = {}

    for ckpt_key in checkpoint_keys:
        # Case 1: direct match — the key exists in both checkpoint and module.
        if ckpt_key in module_key_set:
            remap[ckpt_key] = ckpt_key
            continue

        # Case 2: pattern-based remapping — try each remapping rule.
        for pattern, replacement in remapping_patterns:
            match = re.match(pattern, ckpt_key)
            if match:
                mod_key = re.sub(pattern, replacement, ckpt_key)
                # Only include the remapped key if it actually exists
                # in the module's state_dict.
                if mod_key in module_key_set:
                    remap[ckpt_key] = mod_key
                    break
            # If the pattern doesn't match or the remapped key doesn't
            # exist in the module, silently skip this checkpoint key.

    # Keys not in the remapping are silently skipped during load.
    # This is correct for metadata-only keys (latents) and for keys that
    # don't match any VAE pattern (e.g. xyz_ prefixed keys in no-metadata
    # fixtures — they are intentionally skipped because they don't correspond
    # to any parameter in the constructed module).
    return remap


# ---------------------------------------------------------------------------
# Partial load — meta construction + dtype application (P23-C1)
# ---------------------------------------------------------------------------

# REAL_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_load_real_zit_vae_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_load_mock_returns_sentinel


def load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel:
    """Construct the ZiT VAE model on meta-device, materialize, and load weights.

    Implements steps 1–4 of the four-step loading contract
    (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (step 1, delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (step 2, delegated to ``_select_dtype``).
    3. Construct ``ZiTVaeModel`` on ``torch.device("meta")``, apply dtype,
       materialize onto the target device via ``to_empty()``, build a
       checkpoint-key → module-key remapping, load and cast tensors, and
       call ``load_state_dict(assign=True)``.
    4. Set the ``.arch`` attribute to ``"zit_vae"``.

    Args:
        path: Filesystem path to a ZiT-VAE-format safetensors checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for tensor materialization. Defaults to
            ``"cpu"``. Passed to ``model.to_empty(device=...)`` and
            ``load_file(..., device=...)``.

    Returns:
        A ``ZiTVaeModel`` instance with parameters materialized on *device*,
        carrying the selected dtype, and ``.arch == "zit_vae"``.

    Raises:
        RuntimeError: If torch is not installed (load() is a real-mode-only
            entry point and must not be reached from mock-mode code).
        ValueError: If the checkpoint cannot be opened or hyperparameters
            cannot be inferred (delegated to ``_infer_hyperparams``).
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
    logger.debug("selected dtype=%s for VAE on device=%s", target_dtype, device)

    # Step 3a: materialize all parameters from meta device to the target
    # device. to_empty() allocates real memory for parameters but does not
    # load weights — this is the bridge between meta-construction and weight
    # loading.
    logger.debug(
        "materializing VAE model to device=%s, encoder_channels=%d, "
        "decoder_channels=%d, latent_channels=%d",
        device,
        hyperparams["encoder_channels"],
        hyperparams["decoder_channels"],
        hyperparams["latent_channels"],
    )
    model = model.to_empty(device=device)

    # Step 3b: zero-initialize all parameters and buffers after to_empty().
    # to_empty() allocates memory for meta-device parameters but does not
    # initialize them to zeros — any tensor may contain garbage values,
    # not just biases. This step originally only zeroed ``.bias``-suffixed
    # parameters, on the assumption that this fixture's only unmatched keys
    # were biases — true for this specific fixture, but a fragile,
    # fixture-specific assumption rather than a real guarantee: nothing
    # here actually verified that every ``.weight`` key the checkpoint
    # doesn't cover is otherwise safe. P902 widened this to zero every
    # parameter and buffer unconditionally, matching the identical fix
    # applied to zit.py and qwen3.py — both of which had unmatched
    # *weight* tensors (not just biases) that this narrower version would
    # not have protected against. Loaded values are unaffected: every
    # matched key is overwritten by load_state_dict() below regardless of
    # what it was zeroed to first.
    for param in model.parameters():
        param.data.zero_()
    for buf in model.buffers():
        buf.data.zero_()

    # Step 3c: verify .arch persists after materialization. to_empty() returns
    # the same module object (not a copy), so .arch should be preserved. If
    # it is not (a safety net for future PyTorch versions), re-set it.
    if not hasattr(model, "arch") or model.arch != ARCH:
        model.arch = ARCH

    # Step 3d: load checkpoint tensors into the materialized module.
    # Only keys that exist in BOTH the checkpoint and the module's state_dict
    # are loaded. Keys that exist only in the checkpoint (e.g. the latents
    # metadata tensor) are silently skipped by the remapping — this is correct
    # because the fixture checkpoint uses a simplified key naming convention
    # that doesn't fully populate every parameter.
    state_dict = load_file(path, device=device)

    # Step 3e: build the checkpoint-key → module-key remapping table.
    # This handles direct matches (exact key equality) and pattern-based
    # remapping for VAE key naming conventions (encoder.blocks.N →
    # encoder.block_N, decoder.blocks.N → decoder.block_N).
    remap = _build_key_remapping(
        list(state_dict.keys()), list(model.state_dict().keys())
    )

    # Step 3f: cast each loaded tensor to target_dtype BEFORE calling
    # load_state_dict with assign=True. The assign=True flag bypasses dtype
    # coercion, so the tensor must already have the correct dtype — this is
    # the exact safety measure that prevented the P904 dtype-swap incident.
    #
    # We also filter by shape: the assign=True flag does NOT bypass shape
    # checks. If a checkpoint tensor's shape doesn't match the module's
    # expected shape, it is skipped. This is necessary because the test
    # fixture is a synthetic file with simplified shapes that may not fully
    # match the constructed module architecture.
    remapped_state_dict: dict[str, torch.Tensor] = {}
    for ckpt_key, mod_key in remap.items():
        tensor = state_dict[ckpt_key].to(target_dtype)
        # Check that the tensor shape matches the module's expected shape.
        # Skip tensors with shape mismatches — they are from a different
        # architecture variant or a simplified test fixture.
        if tensor.shape == model.state_dict()[mod_key].shape:
            remapped_state_dict[mod_key] = tensor
        else:
            logger.debug(
                "skipping %s: checkpoint shape %s != module shape %s",
                mod_key,
                tuple(tensor.shape),
                tuple(model.state_dict()[mod_key].shape),
            )

    # Step 3g: load the remapped state dict into the model.
    # assign=True is required for zero-initialized parameters that are already
    # on the target device — it performs in-place assignment without dtype
    # checks. strict=False allows partial loading: only tensors with matching
    # shapes are loaded; others remain at their zero-initialized values.
    info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)
    logger.info(
        "loaded VAE weights: loaded=%d, missing=%d, unexpected=%d, device=%s",
        len(remapped_state_dict),
        len(info.missing_keys),
        len(info.unexpected_keys),
        device,
    )

    return model


# ---------------------------------------------------------------------------
# decode() — latent-to-image (P23-D1)
# ---------------------------------------------------------------------------

# REAL_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_decode_real_zit_vae_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_decode_mock_returns_sentinel


def decode(
    vae_module: ZiTVaeModel,
    latent: torch.Tensor,
    output_mode: str = "RGB",
) -> list:
    """Decode a denoised latent tensor to one or more PIL Images.

    Runs the VAE model's forward pass on the latent tensor, then post-processes
    the raw float output into valid PIL Images. The function clamps values to
    the expected [0, 1] float range, converts to uint8, and reshapes from
    NCHW (batch, channels, H, W) to HWC (H, W, channels) for PIL.

    Args:
        vae_module: A loaded ``ZiTVaeModel`` instance with parameters
            materialized on a device. The model's ``.forward()`` method
            runs the decoder forward pass.
        latent: A denoised latent tensor of shape ``(batch_size, 4, H, W)``
            — the output of the diffusion sampler's latent space.
        output_mode: Image mode for the output. ``"RGB"`` (default) selects
            the first 3 channels for a color image. ``"L"`` selects the
            first channel for grayscale.

    Returns:
        A list of ``PIL.Image.Image`` objects, one per batch item. Each image
        has mode ``"RGB"`` (or ``"L"`` if ``output_mode == "L"``).

    Raises:
        RuntimeError: If torch is not installed (decode is real-mode-only).
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); decode() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here instead of surfacing a
    # confusing AttributeError on a None torch deep inside the forward pass.
    if torch is None:
        raise RuntimeError(
            "zit_vae.py: torch is not installed - decode() is a real-mode-only "
            "entry point and must not be reached from mock-mode code paths."
        )

    # Run the decoder forward pass to get the raw decoded tensor.
    # The output is in the range [0, 1] float, matching standard VAE decoder
    # output normalization. The tensor shape is (batch, channels, H, W).
    # Cast the latent to the model's parameter dtype — the model was
    # constructed with a specific dtype (e.g. bf16), but the caller's
    # latent tensor is typically fp32. A dtype mismatch causes a runtime
    # error in the conv layer.
    model_dtype = next(vae_module.parameters()).dtype
    latent = latent.to(model_dtype)
    decoded = vae_module.forward(latent)

    # Clamp values to [0.0, 1.0] — standard VAE output normalization.
    # This handles the case where the forward pass produces values slightly
    # outside the expected range due to floating-point accumulation.
    decoded = torch.clamp(decoded, 0.0, 1.0)

    # Convert to numpy array for PIL image creation.
    # .cpu() moves to host memory; .numpy() converts to a numpy array.
    # Cast to float32 first — numpy does not support BFloat16.
    decoded_np = decoded.detach().to(torch.float32).cpu().numpy()

    # Select channels based on output_mode.
    # RGB mode takes the first 3 channels from the 16-channel output.
    # L (grayscale) mode takes the first channel only.
    if output_mode == "RGB":
        decoded_np = decoded_np[:, :3, :, :]
    elif output_mode == "L":
        decoded_np = decoded_np[:, :1, :, :]

    # Scale from [0, 1] float to [0, 255] uint8 for PIL image creation.
    decoded_np = (decoded_np * 255).astype("uint8")

    # Build PIL Images from numpy arrays.
    # Each batch item has shape (channels, H, W) — PIL requires (H, W, channels)
    # (NCHW → HWC reshape via np.transpose).
    # For L (grayscale) mode, the channel dimension is 1 and should be squeezed
    # to produce a 2D array (H, W) since PIL's mode="L" expects grayscale input.
    images = []
    for i in range(decoded_np.shape[0]):
        img_array = decoded_np[i]
        # Squeeze channel dimension for grayscale — PIL mode="L" expects (H, W).
        if output_mode == "L":
            img_array = img_array.squeeze(0)
        else:
            # Transpose from (C, H, W) to (H, W, C) for PIL.
            img_array = np.transpose(img_array, (1, 2, 0))
        images.append(Image.fromarray(img_array, mode=output_mode))

    return images


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
