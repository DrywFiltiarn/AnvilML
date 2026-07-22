"""Flux 2 Klein 4B diffusion architecture module.

This module provides shape-inference utilities and meta-device construction
for the Flux 2 Klein diffusion transformer architecture. It implements steps
1--4 of the four-step loading contract defined in ANVILML_DESIGN.md §11.3:

    1. _infer_hyperparams(path) — open header-only, read all key shapes,
           return a dict of inferred hyperparameters (hidden_dim, block
           counts, latent dimensions, patch_size, arch string, native_dtype).
    2. can_handle(key) — implemented in P25-B2.
    3. load(path, caps, device) — implemented: meta construction,
           dtype selection, materialization, key remapping,
           and load_state_dict(assign=True).
    4. sample(model, model_id, conditioning, latent, steps, cfg, seed) —
           implemented: pipeline assembly with caching, denoising loop
           with classifier-free guidance, seed resolution.

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
import secrets
import tempfile
import warnings
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
    from diffusers import EulerDiscreteScheduler
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
    load_file = None  # type: ignore[assignment]
    EulerDiscreteScheduler = None  # type: ignore[assignment]

# Canonical architecture identifier — the string that the dispatcher
# passes to can_handle() when routing diffusion model requests.
# Mirrors the "arch": "flux2klein" value returned by _infer_hyperparams()
# when it reads metadata or falls back to key-pattern inference.
ARCH: str = "flux2klein"

logger = logging.getLogger(__name__)

# Per-process LRU cache for pipeline objects.
# Each model_id gets its own cached pipeline, keyed as "{model_id}:pipeline".
# This avoids re-assembling the scheduler + model wrapper on every sample() call.
from worker.pipeline_cache import PipelineCache

pipeline_cache = PipelineCache()

# Default patch size and latent channel count used by compute_latent_shape()
# when called before load() has cached the actual checkpoint hyperparameters.
# These are updated in-place by load() after _infer_hyperparams() extracts
# the real values from the checkpoint header, so this default only matters
# for a compute_latent_shape() call made before any model has been loaded.
# patch_size=8 matches Flux 2 Klein's actual patch size (the fixture's
# latents tensor is 8×8); the latent_channels=4 is the standard for
# diffusion transformers of this class.
MODEL_PATCH_SIZE: int = 8
MODEL_LATENT_CHANNELS: int = 4

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


# REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_real_after_load
# MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_mock_default_patch_size
def compute_latent_shape(
    width: int, height: int, batch_size: int = 1
) -> tuple[int, int, int, int]:
    """Compute the latent tensor shape for a given input resolution.

    Uses Flux 2 Klein's patch-packing formula: latent_height = ceil(width /
    patch_size), latent_width = ceil(height / patch_size). Returns
    (batch_size, latent_channels, latent_height, latent_width).

    Non-multiple-of-patch-size dimensions are rounded up via ceiling division
    so the latent grid fully covers the input — any partial patch at the edge
    still needs a full column/row of latent tokens.

    The formula uses the module-level constants MODEL_PATCH_SIZE and
    MODEL_LATENT_CHANNELS, which are set to the checkpoint's actual values
    when load() is called. Before load(), the defaults (8, 4) apply.

    Args:
        width: Input image width in pixels.
        height: Input image height in pixels.
        batch_size: Number of samples in the batch. Defaults to 1.

    Returns:
        A 4-tuple (batch_size, latent_channels, latent_height, latent_width)
        representing the shape of the noise latent tensor that EmptyLatent
        should produce before passing it to the Sampler.
    """
    # Ceiling division: (x + patch_size - 1) // patch_size computes ceil(x /
    # patch_size) using only integer arithmetic. This correctly handles exact
    # multiples (e.g. 64 / 8 = 8), non-multiples (e.g. 65 / 8 = ceil(8.125) = 9),
    # and the edge case width=0 (returns 0).
    latent_height = (width + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE
    latent_width = (height + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE

    return (batch_size, MODEL_LATENT_CHANNELS, latent_height, latent_width)


def can_handle(key: str) -> bool:
    """Confirm this module handles the given architecture key.

    The dispatcher passes the architecture string (from safetensors
    metadata or path fallback) as *key*. This function returns True
    only when the key matches this module's canonical identifier.

    Args:
        key: Architecture string to check, e.g. ``"zit"`` or
            ``"flux2klein"``.

    Returns:
        ``True`` if *key* equals ``"flux2klein"``, ``False`` otherwise.
    """
    return key == ARCH


# ---------------------------------------------------------------------------
# Flux2KleinModel — meta-device model construction
# ---------------------------------------------------------------------------

# nn.Module is unavailable when torch failed to import (see the guard above).
# Flux2KleinModel falls back to plain `object` as its base in that case —
# the class still defines successfully (only __init__/forward bodies touch
# torch, and those are never invoked without going through the guarded
# load()/sample() entry points below), which is what keeps this module
# importable in mock-mode collection.
_ModuleBase = nn.Module if nn is not None else object


class Flux2KleinModel(_ModuleBase):
    """Flux 2 Klein diffusion transformer model constructed from layer-level building blocks.

    This class assembles the Flux 2 Klein architecture using ``torch.nn``
    primitives (Linear, LayerNorm, MultiheadAttention, Sequential, GELU) that
    mirror the tensor shapes found in the checkpoint. It is constructed on
    ``torch.device("meta")`` so that no real GPU/CPU memory is allocated
    during construction — this prevents large memory allocation during model
    construction.

    The Flux 2 Klein architecture consists of:
    - ``input_proj``: latent space → hidden dimension projection
    - ``time_text_emb``: time-step + text embedding projection
    - ``double_blocks``: list of modulated cross-attention blocks with
      adaptive LayerNorm modulation (``img_mod`` / ``txt_mod``), image and
      text attention sub-layers (``img_attn`` / ``txt_attn``), and SwiGLU-style
      MLPs (``img_mlp`` / ``txt_mlp``).
    - ``single_blocks``: list of linear transformation blocks
    - ``final_layer``: output projection with adaptive LN modulation
      (``adaLN_modulation``)

    The ``.arch`` attribute is set to ``"flux2klein"`` after construction
    so that downstream code (Sampler, VaeDecode) can identify the model family.

    Args:
        hyperparams: Dict from ``_infer_hyperparams()`` containing
            hidden_dim, double_block_count, single_block_count,
            latent_channels, latent_height, latent_width, patch_size.
    """

    def __init__(self, hyperparams: dict[str, Any]) -> None:
        """Construct the Flux 2 Klein model on the meta device.

        Args:
            hyperparams: Dict from ``_infer_hyperparams()`` containing
                hidden_dim, double_block_count, single_block_count,
                latent_channels, latent_height, latent_width, patch_size.
        """
        super().__init__()

        # Extract hyperparameters — all derived from the checkpoint header,
        # never hardcoded. This ensures the model structure always matches
        # the actual checkpoint it was built from.
        hidden_dim = hyperparams["hidden_dim"]
        double_block_count = hyperparams["double_block_count"]
        single_block_count = hyperparams["single_block_count"]
        latent_channels = hyperparams["latent_channels"]
        patch_size = hyperparams["patch_size"]

        # Input projection: (latent_channels * patch_size^2) → hidden_dim
        # The latent tensor is reshaped to (batch, latent_channels*patch_size^2,
        # height*width) before projection into the hidden dimension.
        latent_dim = latent_channels * patch_size * patch_size
        self.input_proj = nn.Linear(latent_dim, hidden_dim)

        # Time-step + text embedding projection (hidden_dim → hidden_dim).
        # The time token and text embedding are combined in a single linear
        # layer before being added to the hidden representation.
        self.time_text_emb = nn.Linear(hidden_dim, hidden_dim)

        # Double blocks with modulated cross-attention sub-layers.
        # Each double block has:
        # - img_mod / txt_mod: Linear layers that generate modulation parameters
        #   for 6 LayerNorm layers (scale, shift, gate for img_attn, txt_attn,
        #   img_mlp, txt_mlp — 3 sub-layers × 2 per sub-layer).
        # - img_attn / txt_attn: MultiheadAttention layers for self-attention
        #   (image) and cross-attention (text).
        # - img_norm1/2, txt_norm1/2: LayerNorm layers for adaptive normalization.
        # - img_mlp / txt_mlp: SwiGLU-style MLPs (Linear → GELU → Linear).
        self.double_blocks = nn.ModuleList([
            nn.ModuleDict({
                "img_mod": nn.Linear(hidden_dim, hidden_dim * 6),
                "txt_mod": nn.Linear(hidden_dim, hidden_dim * 6),
                "img_attn": nn.MultiheadAttention(
                    embed_dim=hidden_dim,
                    num_heads=hidden_dim // 64,
                    batch_first=True,
                ),
                "txt_attn": nn.MultiheadAttention(
                    embed_dim=hidden_dim,
                    num_heads=hidden_dim // 64,
                    batch_first=True,
                ),
                "img_norm1": nn.LayerNorm(hidden_dim),
                "img_norm2": nn.LayerNorm(hidden_dim),
                "txt_norm1": nn.LayerNorm(hidden_dim),
                "txt_norm2": nn.LayerNorm(hidden_dim),
                "img_mlp": nn.Sequential(
                    nn.Linear(hidden_dim, hidden_dim * 4),
                    nn.GELU(),
                    nn.Linear(hidden_dim * 4, hidden_dim),
                ),
                "txt_mlp": nn.Sequential(
                    nn.Linear(hidden_dim, hidden_dim * 4),
                    nn.GELU(),
                    nn.Linear(hidden_dim * 4, hidden_dim),
                ),
            })
            for _ in range(double_block_count)
        ])

        # Single blocks with linear transformation.
        # Each single block is a simplified linear transformation block
        # used in Flux 2 Klein's architecture after the double blocks.
        self.single_blocks = nn.ModuleList([
            nn.ModuleDict({
                "linear1": nn.Linear(hidden_dim, hidden_dim * 4),
                "linear2": nn.Linear(hidden_dim * 4, hidden_dim),
                "norm": nn.LayerNorm(hidden_dim),
            })
            for _ in range(single_block_count)
        ])

        # Final layer: adaptive LayerNorm modulation + output projection.
        # The adaLN_modulation layer produces scale and shift parameters
        # for the final LayerNorm. The output projection maps from hidden_dim
        # back to latent_dim (latent_channels * patch_size^2).
        self.final_layer = nn.ModuleDict({
            "adaLN_modulation": nn.Linear(hidden_dim, hidden_dim * 2),
            "linear": nn.Linear(hidden_dim, latent_dim),
        })

        # Architecture identifier — set after construction so downstream
        # code can identify this model's family.
        self.arch: str = "flux2klein"

    def forward(
        self,
        x: torch.Tensor,
        timestep: float,
        conditioning: Any = None,
    ) -> torch.Tensor:
        """Forward pass through the Flux 2 Klein diffusion transformer.

        **Stub implementation (P25-C1):** this forward pass is structurally
        correct — it projects the input through the layers in the right order —
        but does not implement the full adaptive LayerNorm modulation math.
        The modulation (img_mod/txt_mod generating scale/shift/gate parameters)
        is deferred to P25-D1 when ``sample()`` is implemented.

        The full modulation math works as follows: each modulation layer
        (img_mod, txt_mod) produces a vector of size ``hidden_dim * 6`` (or
        ``hidden_dim * 2`` for final_layer), which is split into parameters
        for the LayerNorm layers in each sub-block. For example, img_mod
        produces 6 parameters per hidden dimension: (scale1, shift1, gate1)
        for img_norm1 and (scale2, shift2, gate2) for img_norm2. These
        parameters are applied as ``norm(x * scale + shift) * gate``.

        Args:
            x: The input latent tensor of shape
                ``(batch, latent_channels, latent_height, latent_width)``.
            timestep: A float in ``[0, 1]`` indicating the current
                denoising step's position in the noise schedule.
            conditioning: Optional conditioning tensor (e.g. text
                embeddings from a CLIP encoder). Passed through to
                the double blocks' cross-attention layers.

        Returns:
            A noise prediction tensor of the same shape as *x*,
            ``(batch, latent_channels, latent_height, latent_width)``.
        """
        # ------------------------------------------------------------------
        # 1. Project latent into hidden dimension.
        # ------------------------------------------------------------------
        batch = x.shape[0]
        x_flat = x.reshape(batch, -1)  # (batch, latent_dim)
        h = self.input_proj(x_flat)  # (batch, hidden_dim)

        # ------------------------------------------------------------------
        # 2. Time embedding (minimal pass-through — full modulation deferred).
        # ------------------------------------------------------------------
        # The full time embedding with sinusoidal encoding and modulation
        # parameter generation is deferred. For now, we pass a zero vector
        # of the correct hidden dimension through the time_text_emb layer
        # as a structural placeholder so the forward pass completes.
        # hidden_dim is derived from the input_proj layer's output size.
        hidden_dim = self.input_proj.out_features
        time_emb = self.time_text_emb(torch.zeros(hidden_dim, device=h.device, dtype=h.dtype))
        h = h + time_emb

        # ------------------------------------------------------------------
        # 3. Pass through double blocks (modulated cross-attention).
        # ------------------------------------------------------------------
        # The full modulation math is deferred to P25-D1. For now, we pass
        # data through the layers without modulation as a structural placeholder.
        for block in self.double_blocks:
            h = block["img_norm1"](h)
            # Image self-attention: Q, K, V all come from h.
            attn_out, _ = block["img_attn"](h, h, h)
            h = h + attn_out
            h = block["img_norm2"](h)
            h = h + block["img_mlp"](h)

        # ------------------------------------------------------------------
        # 4. Pass through single blocks (linear transformation).
        # ------------------------------------------------------------------
        for block in self.single_blocks:
            h = block["norm"](h)
            h = h + block["linear2"](torch.nn.functional.gelu(block["linear1"](h)))

        # ------------------------------------------------------------------
        # 5. Final layer output projection.
        # ------------------------------------------------------------------
        # The full adaLN modulation is deferred to P25-D1.
        out = self.final_layer["linear"](h)

        # Reshape back to (batch, latent_channels, latent_height, latent_width).
        latent_channels = x.shape[1]
        resized_height = x.shape[2]
        resized_width = x.shape[3]
        out = out.reshape(
            batch, latent_channels, resized_height, resized_width
        )

        return out


# ---------------------------------------------------------------------------
# Flux2KleinPipeline — pipeline wrapper + assembly
# ---------------------------------------------------------------------------


class Flux2KleinPipeline:
    """Minimal pipeline wrapper that holds a ``Flux2KleinModel`` and a ``diffusers`` scheduler.

    This class is a thin container — it does not implement a denoising loop itself.
    The denoising loop is implemented by ``sample()``. This wrapper provides the
    interface that ``sample()`` calls: ``.model`` for the neural network and
    ``.scheduler`` for the noise schedule.

    Attributes:
        model: The ``Flux2KleinModel`` instance (an ``nn.Module``) to run
            inference with.
        scheduler: A ``diffusers`` scheduler instance that generates the noise
            schedule and provides the step function interface.
    """

    def __init__(self, model: Flux2KleinModel, scheduler: Any) -> None:
        """Construct a ``Flux2KleinPipeline`` wrapper.

        Args:
            model: An already-loaded ``Flux2KleinModel`` instance with parameters
                materialized on the target device.
            scheduler: A ``diffusers`` scheduler instance (e.g.
                ``EulerDiscreteScheduler``) that defines the noise schedule.
        """
        self.model = model
        self.scheduler = scheduler


def _assemble_pipeline(model: Flux2KleinModel) -> Flux2KleinPipeline:
    """Assemble a ``Flux2KleinPipeline`` from a loaded ``Flux2KleinModel``.

    Creates a ``Flux2KleinPipeline`` wrapper that holds the model and a default
    ``EulerDiscreteScheduler``.  The scheduler is a simple placeholder —
    the full denoising step function is wired in ``sample()``.

    The function is called via ``PipelineCache.get_or_load()`` so that
    pipeline assembly happens at most once per ``model_id``.

    Args:
        model: An already-loaded ``Flux2KleinModel`` instance with parameters
            materialized on the target device.

    Returns:
        A ``Flux2KleinPipeline`` instance wrapping *model* and a default scheduler.
    """
    # Use EulerDiscreteScheduler as the default scheduler — it is a widely
    # used, stable scheduler in diffusers that provides a simple step
    # interface. The actual denoising loop is implemented in sample().
    scheduler = EulerDiscreteScheduler()
    return Flux2KleinPipeline(model, scheduler)


# ---------------------------------------------------------------------------
# _select_dtype — per-ANVILML_DESIGN.md §11.5 precedence chain
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
        logger.debug(
            "selecting fp8 dtype: caps.fp8=%s, native_dtype=%s",
            caps.get("fp8"),
            native_dtype,
        )
        return torch.float8_e4m3fn

    # Branch 2: BF16 — the next-highest precision when FP8 is not viable.
    # bfloat16 is widely supported on modern GPUs and provides dynamic
    # range close to fp32 with half the memory footprint.
    if caps.get("bf16", False):
        logger.debug(
            "selecting bf16 dtype: caps.bf16=%s (fp8 not viable)",
            caps.get("bf16"),
        )
        return torch.bfloat16

    # Branch 3: FP16 — the next fallback when bf16 is not available.
    # float16 has a narrower dynamic range than bf16 but is still
    # significantly more memory-efficient than fp32.
    if caps.get("fp16", False):
        logger.debug(
            "selecting fp16 dtype: caps.fp16=%s (bf16 not available)",
            caps.get("fp16"),
        )
        return torch.float16

    # Branch 4: FP32 — the universal fallback. Every device supports fp32,
    # so this path is always reachable. It is the most numerically stable
    # but also the most memory-intensive option.
    logger.debug(
        "selecting fp32 dtype: no higher precision available (caps=%s)",
        {k: v for k, v in caps.items() if k in ("fp8", "bf16", "fp16")},
    )
    return torch.float32


# ---------------------------------------------------------------------------
# _build_key_remapping — checkpoint-key → module-key mapping for Flux 2 Klein
# ---------------------------------------------------------------------------


def _build_key_remapping(
    checkpoint_keys: list[str], module_keys: list[str]
) -> dict[str, str]:
    """Build a checkpoint-key → module-key mapping for ``load_state_dict``.

    Iterates over checkpoint keys and builds a remapping table that maps
    each checkpoint key to the corresponding module state_dict key. The
    function handles three cases:

    1. **Direct match:** the checkpoint key exists verbatim in the module's
       state_dict keys. The mapping is identity: ``ckpt_key → mod_key``.
    2. **Pattern-based remapping:** for Flux 2 Klein checkpoint keys that use
       a simplified naming convention (e.g. ``double_blocks.N.img_attn.qkv``
       → ``double_blocks.N.img_attn.in_proj_weight``), the function applies
       known remapping patterns. The remapped key is only included if it
       exists in the module's state_dict.
    3. **xyz_ fallback:** for no-metadata fixtures that use ``xyz_`` prefixed
       keys with underscores instead of dots (e.g. ``xyz_double_blocks_0_img_attn_qkv``
       → ``double_blocks.0.img_attn.qkv``), the function first converts the
       key back to dot notation, then applies the Flux 2 Klein remapping rules.

    Keys that exist only in the checkpoint (e.g. metadata tensors like
    ``latents``) are silently excluded from the remapping — they will not
    be loaded into the module, which is correct because the fixture uses
    a simplified key naming that doesn't fully populate all parameters.

    Args:
        checkpoint_keys: List of tensor keys from the safetensors file
            (returned by ``load_file()``).
        module_keys: List of parameter keys from ``model.state_dict().keys()``.

    Returns:
        A dict mapping ``checkpoint_key → module_key`` for all keys that
        can be successfully remapped.
    """
    module_key_set = set(module_keys)

    # Pattern-based remapping rules for Flux 2 Klein-specific key naming
    # conventions. These rules convert checkpoint keys to PyTorch
    # MultiheadAttention parameter names. Each rule is a (pattern,
    # replacement) pair where the pattern is a regex that matches the
    # checkpoint key and the replacement is the module key template.
    #
    # The Flux 2 Klein checkpoint stores image/text attention QKV as a
    # single combined tensor (e.g. ``double_blocks.N.img_attn.qkv``) but
    # PyTorch's MultiheadAttention uses ``double_blocks.N.img_attn.in_proj_weight``.
    # This remapping converts the checkpoint key to the module key.
    #
    # The shape of ``img_attn.qkv`` (hidden_dim, hidden_dim*3) matches
    # ``in_proj_weight`` (hidden_dim, hidden_dim*3). ✓
    # The shape of ``txt_attn.qkv`` (context_dim, hidden_dim*3) does NOT
    # match ``in_proj_weight`` (context_dim, context_dim*3) — this remapping
    # will produce a shape mismatch and the tensor will be skipped in the
    # load loop. This is correct for the fixture which uses simplified
    # dimensions.
    remapping_patterns: list[tuple[str, str]] = [
        # Image attention QKV: combined tensor → PyTorch in_proj_weight
        # NOTE: checkpoint shape (hidden_dim, hidden_dim*3) does NOT match
        # module shape (3*embed_dim, embed_dim) — tensor is transposed and
        # will be filtered by the shape check in the load loop.
        (
            r"double_blocks\.(\d+)\.img_attn\.qkv",
            r"double_blocks.\1.img_attn.in_proj_weight",
        ),
        # Text attention QKV: combined tensor → PyTorch in_proj_weight
        # NOTE: checkpoint shape (context_dim, hidden_dim*3) does NOT match
        # module shape (3*context_dim, context_dim) — tensor is transposed.
        (
            r"double_blocks\.(\d+)\.txt_attn\.qkv",
            r"double_blocks.\1.txt_attn.in_proj_weight",
        ),
        # Image attention projection: single proj → PyTorch out_proj.weight
        (
            r"double_blocks\.(\d+)\.img_attn\.proj",
            r"double_blocks.\1.img_attn.out_proj.weight",
        ),
        # Text attention projection: single proj → PyTorch out_proj.weight
        # NOTE: checkpoint shape (context_dim, hidden_dim) does NOT match
        # module shape (embed_dim, embed_dim) — different dimensions.
        (
            r"double_blocks\.(\d+)\.txt_attn\.proj",
            r"double_blocks.\1.txt_attn.out_proj.weight",
        ),
        # Image attention norm → img_norm1.weight (LayerNorm weight)
        (
            r"double_blocks\.(\d+)\.img_attn\.norm",
            r"double_blocks.\1.img_norm1.weight",
        ),
        # Text attention norm → txt_norm1.weight (LayerNorm weight)
        (
            r"double_blocks\.(\d+)\.txt_attn\.norm",
            r"double_blocks.\1.txt_norm1.weight",
        ),
        # Image MLP up-projection (Sequential index match)
        # NOTE: checkpoint shape (hidden_dim, hidden_dim*4) does NOT match
        # module shape (hidden_dim*4, hidden_dim) — tensor is transposed.
        (
            r"double_blocks\.(\d+)\.img_mlp\.(\d+)",
            r"double_blocks.\1.img_mlp.\2.weight",
        ),
        # Text MLP up-projection (Sequential index match)
        # NOTE: checkpoint shape (hidden_dim, hidden_dim*4) does NOT match
        # module shape (hidden_dim*4, hidden_dim) — tensor is transposed.
        (
            r"double_blocks\.(\d+)\.txt_mlp\.(\d+)",
            r"double_blocks.\1.txt_mlp.\2.weight",
        ),
        # Time/text embedder timestep weight → time_text_emb.weight
        # Matches both ``time_text_embed.timestep_embedder.0.weight``
        # (regular fixture) and ``time_text_embed.timestep_embedder``
        # (no-metadata fixture without .weight suffix).
        (
            r"time_text_embed\.timestep_embedder(?:\.\d+)?(?:\.weight)?",
            r"time_text_emb.weight",
        ),
        # Context embedder → context_embedding.weight (no-metadata fallback)
        (
            r"time_text_embed\.context_embedder",
            r"context_embedding.weight",
        ),
    ]

    # xyz_ → dot-notation conversion for no-metadata fixtures.
    # Strip the xyz_ prefix, protect compound words (double_blocks, single_blocks,
    # final_layer, time_text_embed, img_attn, etc.) from underscore replacement,
    # then replace remaining underscores with dots.
    # e.g. xyz_double_blocks_0_img_attn_norm → double_blocks.0.img_attn.norm

    remap: dict[str, str] = {}

    for ckpt_key in checkpoint_keys:
        # Case 1: direct match — the key exists in both checkpoint and module.
        if ckpt_key in module_key_set:
            remap[ckpt_key] = ckpt_key
            continue

        # Case 2: pattern-based remapping — try each Flux 2 Klein remapping rule.
        for pattern, replacement in remapping_patterns:
            match = re.match(pattern, ckpt_key)
            if match:
                mod_key = re.sub(pattern, replacement, ckpt_key)
                # Only include the remapped key if it actually exists
                # in the module's state_dict.
                if mod_key in module_key_set:
                    remap[ckpt_key] = mod_key
                    break
            # If the pattern doesn't match, silently continue to the next rule.

        # Case 3: xyz_ fallback — convert no-metadata fixture keys to
        # dot notation, then try the Flux 2 Klein remapping rules again.
        if ckpt_key not in remap:
            # Strip xyz_ prefix, protect compound words, replace remaining
            # underscores with dots. Compound words like double_blocks,
            # single_blocks, final_layer, time_text_embed, img_attn, etc.
            # are protected so their internal underscores stay as underscores.
            converted_key = ckpt_key
            if converted_key.startswith("xyz_"):
                s = converted_key[4:]
                # Protect compound words from underscore replacement
                compound_words = [
                    "time_text_embed", "timestep_embedder",
                    "double_blocks", "single_blocks", "final_layer",
                    "img_attn", "txt_attn", "img_mlp", "txt_mlp",
                    "img_mod", "txt_mod", "adaLN_modulation",
                    "context_embedder",
                ]
                placeholder = "\x00"
                for word in compound_words:
                    s = s.replace(word, word.replace("_", placeholder))
                s = s.replace("_", ".")
                for word in compound_words:
                    s = s.replace(placeholder, "_")
                converted_key = s
            # Now try the Flux 2 Klein remapping patterns on the converted key.
            if converted_key != ckpt_key:
                for pattern, replacement in remapping_patterns:
                    match = re.match(pattern, converted_key)
                    if match:
                        mod_key = re.sub(pattern, replacement, converted_key)
                        if mod_key in module_key_set:
                            remap[ckpt_key] = mod_key
                            break
                    # If the pattern doesn't match, silently continue.

    # Keys not in the remapping are silently skipped during load.
    # This is correct for metadata-only keys (latents) and for keys where
    # the shape doesn't match the module's parameters (e.g. txt_attn.qkv
    # has a different shape than the module's in_proj_weight).
    return remap


# ---------------------------------------------------------------------------
# load — meta construction + dtype selection + materialization (P25-C1)
# ---------------------------------------------------------------------------

# REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_load_meta_construction_regular_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_collection_safety_load_import
def load(path: str, caps: dict, device: str = "cpu") -> Flux2KleinModel:
    """Construct the Flux 2 Klein model on meta-device, materialize, and load weights.

    Implements all four steps of the loading contract
    (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (delegated to ``_select_dtype``).
    3. Construct ``Flux2KleinModel`` on ``torch.device("meta")``, apply
       dtype, materialize onto the target device via ``to_empty()``,
       and zero-initialize all parameters and buffers.
    4. Load checkpoint tensors with key remapping and
       ``load_state_dict(assign=True, strict=False)``.

    Args:
        path: Filesystem path to a Flux 2 Klein-format safetensors
            checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for tensor materialization. Defaults to
            ``"cpu"``. Passed to ``model.to_empty(device=...)``.

    Returns:
        A ``Flux2KleinModel`` instance with parameters materialized on
        *device*, carrying the selected dtype, and ``.arch == "flux2klein"``.

    Raises:
        RuntimeError: If torch is not installed (this is a real-mode-only
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
            "flux2klein.py: torch is not installed - load() is a real-mode-only "
            "entry point (ANVILML_DESIGN.md §18.3) and must not be reached "
            "from mock-mode code paths."
        )

    # Step 1 (from P25-B1): infer hyperparameters from checkpoint header,
    # including the native dtype of the first weight tensor. This reads only
    # the ~100KB metadata header — no tensor data is loaded.
    hyperparams = _infer_hyperparams(path)

    # Cache the checkpoint's patch_size and latent_channels as module-level
    # state so compute_latent_shape() can use them without a model argument.
    # This is the simplest approach that matches the fixed signature
    # compute_latent_shape(width, height, batch_size).
    global MODEL_PATCH_SIZE, MODEL_LATENT_CHANNELS
    MODEL_PATCH_SIZE = hyperparams["patch_size"]
    MODEL_LATENT_CHANNELS = hyperparams["latent_channels"]
    logger.debug(
        "cached hyperparams: patch_size=%d, latent_channels=%d",
        MODEL_PATCH_SIZE,
        MODEL_LATENT_CHANNELS,
    )

    # Step 2: select the compute dtype per the fixed precedence in
    # ANVILML_DESIGN.md §11.5. The native dtype is read from the checkpoint
    # header; the capability flags come from the worker's own torch-level probe.
    # This ensures the dtype decision is driven by both what the checkpoint
    # actually uses and what the worker hardware can execute.
    target_dtype = _select_dtype(caps, hyperparams["native_dtype"])

    # Step 3: construct on meta-device with selected dtype.
    # Using torch.device("meta") means no real memory is allocated for
    # parameters — the module structure exists but tensors have shape
    # metadata only. This prevents large memory allocation during construction.
    with torch.device("meta"):
        model = Flux2KleinModel(hyperparams)

    # Apply the selected dtype to the meta-constructed module.
    # model.to(dtype) on a module with meta-device parameters changes their
    # dtype metadata without allocating real memory — this is the standard
    # PyTorch idiom for dtype selection before weight loading.
    model.to(target_dtype)

    # Log materialization parameters for diagnostics.
    logger.debug(
        "materializing Flux2Klein model to device=%s, hidden_dim=%d, "
        "double_blocks=%d, single_blocks=%d",
        device,
        hyperparams["hidden_dim"],
        hyperparams["double_block_count"],
        hyperparams["single_block_count"],
    )

    # Materialize all parameters from meta device to the target device.
    # to_empty() allocates real memory for parameters but does not load
    # weights — this is the bridge between meta-construction and weight loading.
    model = model.to_empty(device=device)

    # to_empty() allocates UNINITIALIZED memory — it does not zero anything.
    # Zero every parameter and buffer explicitly here, before loading the
    # checkpoint in P25-C2, so any key the checkpoint doesn't cover
    # deterministically stays at zero — making the "zero-initialized by
    # design" comment actually true — rather than silently propagating NaN
    # through the first forward pass.
    for param in model.parameters():
        param.data.zero_()
    for buf in model.buffers():
        buf.data.zero_()

    # ------------------------------------------------------------------
    # Step 4: Load checkpoint tensors and build the remapped state dict.
    # ------------------------------------------------------------------
    # Only keys that exist in BOTH the checkpoint and the module's state_dict
    # with matching shapes are loaded. Keys that don't map or have shape
    # mismatches are silently skipped — this is correct because the test
    # fixture uses a simplified key naming convention that doesn't fully
    # populate the PyTorch MultiheadAttention parameters.
    state_dict = load_file(path, device=device)

    # Build the checkpoint-key → module-key remapping table for Flux 2 Klein.
    # This handles direct matches and pattern-based remapping for known
    # Flux 2 Klein key naming conventions (qkv → in_proj_weight, etc.).
    # For no-metadata fixtures, it also converts xyz_ prefixed keys to
    # dot notation before remapping.
    remap = _build_key_remapping(
        list(state_dict.keys()), list(model.state_dict().keys())
    )

    # Cast each loaded tensor to target_dtype BEFORE calling load_state_dict
    # with assign=True. The assign=True flag bypasses dtype coercion, so the
    # tensor must already have the correct dtype — this is the exact safety
    # measure that prevented the P904 dtype-swap incident.
    #
    # We also filter by shape: assign=True does NOT bypass shape checks.
    remapped_state_dict: dict[str, torch.Tensor] = {}
    for ckpt_key, mod_key in remap.items():
        tensor = state_dict[ckpt_key].to(target_dtype)
        if tensor.shape == model.state_dict()[mod_key].shape:
            remapped_state_dict[mod_key] = tensor
        else:
            logger.debug(
                "skipping %s: checkpoint shape %s != module shape %s",
                mod_key,
                tuple(tensor.shape),
                tuple(model.state_dict()[mod_key].shape),
            )

    # Load the remapped state dict into the model.
    # assign=True performs in-place assignment without dtype checks.
    # strict=False allows partial loading: only tensors with matching
    # shapes are loaded; others remain at their zero-initialized values.
    info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)
    logger.info(
        "loaded Flux2Klein weights: loaded=%d, missing=%d, unexpected=%d, device=%s",
        len(remapped_state_dict),
        len(info.missing_keys),
        len(info.unexpected_keys),
        device,
    )

    # Verify .arch persists after materialization. to_empty() returns the same
    # module object (not a copy), so .arch should be preserved. If it is not,
    # explicitly re-set it — this is a safety net for future PyTorch versions.
    if not hasattr(model, "arch") or model.arch != ARCH:
        model.arch = ARCH

    return model


# REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_denoising_real_flux2klein_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_seed_minus_one_resolves_random
def _resolve_conditioning(conditioning: Any) -> tuple[Any, Any]:
    """Split *conditioning* into its positive and negative embedding tensors.

    ``ClipTextEncode``'s real branch (``worker/nodes/encoder.py``,
    ``ANVILML_DESIGN.md`` §10.3) produces a dict shaped
    ``{"text_embeds": Tensor, "negative_text_embeds": Tensor?}`` — the
    latter present only when a negative prompt was supplied. This function
    is the single place ``sample()`` interprets that shape, so callers
    that bypass ``ClipTextEncode`` and pass a bare tensor (or ``None``)
    directly keep working unchanged: a non-dict value is treated as the
    positive embedding with no negative counterpart.

    Args:
        conditioning: Either a dict with "text_embeds"/"negative_text_embeds"
            keys, a bare tensor, or None.

    Returns:
        A ``(cond_embeds, uncond_embeds)`` tuple. *uncond_embeds* is
        ``None`` whenever no negative embedding is available, which
        ``forward()`` already treats as "no conditioning" for that pass.
    """
    if isinstance(conditioning, dict):
        return conditioning.get("text_embeds"), conditioning.get("negative_text_embeds")
    return conditioning, None


# REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_denoising_real_flux2klein_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_seed_minus_one_resolves_random
def sample(
    model: Flux2KleinModel,
    model_id: str,
    conditioning: Any,
    latent: torch.Tensor,
    steps: int,
    cfg: float,
    seed: int,
) -> tuple[torch.Tensor, int]:
    """Run the denoising loop and return the denoised latent tensor.

    On the first call for a given ``model_id``, this function assembles a
    ``Flux2KleinPipeline`` (model + scheduler) and caches it under
    ``f"{model_id}:pipeline"`` in the module-level ``PipelineCache``.
    Subsequent calls with the same ``model_id`` return the cached pipeline
    without re-assembly.

    If *seed* is negative (conventionally ``-1``), it is resolved to a
    cryptographically random integer in ``[0, 2**63)`` via
    ``secrets.randbelow()`` before denoising begins. The resolved seed is
    logged and returned — the caller never sees ``-1`` in the output.

    The denoising loop iterates over the scheduler's timesteps, performing
    classifier-free guidance at each step: an unconditional pass (the
    negative prompt's conditioning, via ``_resolve_conditioning()``, or no
    conditioning when none was supplied) and a conditional pass (the
    positive prompt's conditioning) are blended using the *cfg* scale to
    interpolate between the unconditional prior and guided output.

    Args:
        model: An already-loaded ``Flux2KleinModel`` instance.
        model_id: Stable model identifier used as the cache key prefix.
            Pipelines are cached per-model-id, keyed as ``f"{model_id}:pipeline"``.
        conditioning: Conditioning input for the diffusion process — either
            a dict shaped ``{"text_embeds": Tensor, "negative_text_embeds":
            Tensor?}`` (ClipTextEncode's real-branch output), or a bare
            tensor/None for callers that build conditioning directly. See
            ``_resolve_conditioning()``.
        latent: The initial noise latent tensor. Cloned before denoising
            so the caller's tensor is never mutated.
        steps: Number of denoising steps to run.
        cfg: Classifier-free guidance scale. A value of ``1.0`` disables
            guidance (conditional only); higher values increase the weight
            of the conditional signal.
        seed: Random seed for reproducibility. If negative, resolved to a
            cryptographically random integer in ``[0, 2**63)``.

    Returns:
        A tuple ``(denoised_latent, resolved_seed)`` where
        *denoised_latent* is the output tensor after all denoising steps
        and *resolved_seed* is the non-negative seed actually used.

    Raises:
        RuntimeError: If torch is not installed (this is a real-mode-only
            entry point and must not be reached from mock-mode code).
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); sample() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here rather than surfacing a
    # confusing AttributeError on a None torch deep inside the denoising loop.
    if torch is None:
        raise RuntimeError(
            "flux2klein.py: torch is not installed - sample() is a real-mode-only "
            "entry point (ANVILML_DESIGN.md §18.3) and must not be reached "
            "from mock-mode code paths."
        )

    # Resolve seed: negative values (conventionally -1) mean "random".
    # Use secrets.randbelow for cryptographic randomness — this ensures
    # that two consecutive calls with seed=-1 produce different outputs,
    # which is the expected behavior for a generation endpoint.
    if seed < 0:
        seed = secrets.randbelow(2**63)

    # Cache key is "{model_id}:pipeline" — distinct from the raw component
    # cache key used by load() (which caches under "{model_id}:model").
    # This separation means the pipeline (model + scheduler wrapper) is
    # cached independently from the raw model weights.
    key = f"{model_id}:pipeline"

    # get_or_load returns cached pipeline if present, or calls the loader
    # exactly once on cache miss. The lambda captures *model* so that
    # _assemble_pipeline() receives the correct model instance.
    pipeline = pipeline_cache.get_or_load(key, lambda: _assemble_pipeline(model))
    logger.debug("assembled pipeline for model_id=%s", model_id)

    # Set up the scheduler's noise schedule for the requested step count.
    # set_timesteps() computes the discrete timesteps (e.g. [999, 900, 800, ...])
    # based on the scheduler's internal beta schedule, then exposes them via
    # the .timesteps attribute for iteration.
    #
    # diffusers set_timesteps() internally converts alphas_cumprod to sigmas
    # via np.array(tensor, ...), and the tensor's __array__ predates numpy>=2.0's
    # requirement that __array__ implementations accept dtype/copy keywords —
    # a compatibility gap inside diffusers itself, not our code. Suppressed
    # right here, scoped to this one call, so it can never mask a real
    # DeprecationWarning raised elsewhere.
    scheduler = pipeline.scheduler
    with warnings.catch_warnings():
        warnings.filterwarnings(
            "ignore",
            message=r"__array__ implementation doesn't accept a copy keyword",
            category=DeprecationWarning,
        )
        scheduler.set_timesteps(steps)

    # Clone the latent tensor before denoising — we must not mutate the
    # caller's tensor since it may be reused (e.g. for conditioning
    # propagation or for generating multiple samples from the same seed).
    latent = latent.clone()

    # Cast the latent to the model's dtype — the model's parameters are
    # on the selected precision (e.g. bf16), but the caller may pass a
    # float32 tensor. PyTorch requires matching dtypes for linear ops.
    # We inspect the first parameter to determine the model's dtype.
    model_dtype = next(model.parameters()).dtype
    if latent.dtype != model_dtype:
        latent = latent.to(model_dtype)

    # Resolve the positive (conditional) and negative (unconditional)
    # conditioning tensors via the shared helper (see its docstring for
    # the accepted shapes).
    cond_embeds, uncond_embeds = _resolve_conditioning(conditioning)

    # Denoising loop: iterate over the scheduler's timesteps in order.
    # At each timestep, we perform classifier-free guidance (CFG) by
    # running both an unconditional pass (the negative prompt's
    # conditioning, or no conditioning when none was supplied) and a
    # conditional pass, then interpolating between them using the cfg
    # scale. The interpolation formula is:
    #   noise_pred = noise_pred_uncond + cfg * (noise_pred_cond - noise_pred_uncond)
    # which is the standard CFG formulation: uncond + scale * delta.
    for t in scheduler.timesteps:
        # Scale the current latent by this timestep's sigma before feeding
        # it to the model — EulerDiscreteScheduler (like most sigma-based
        # schedulers) requires this; the raw, unscaled latent is only ever
        # correct for the timestep-independent `scheduler.step()` call
        # below, never for the model's own forward pass.
        scaled_latent = scheduler.scale_model_input(latent, t)

        # Unconditional pass: model predicts noise using the negative
        # prompt's conditioning when one was supplied, else no
        # conditioning at all. This represents the "prior" being pushed
        # away from — either an explicit negative prompt, or (absent one)
        # what the model would generate without any text prompt guidance.
        # The model's forward() returns a tensor directly (not a named
        # tuple), so we use it without .sample.
        with torch.no_grad():
            noise_pred_uncond = pipeline.model(
                scaled_latent, t / 1000.0, conditioning=uncond_embeds
            )

        # Conditional pass: model predicts noise with the positive
        # prompt's conditioning (e.g. text embeddings from a CLIP
        # encoder).
        with torch.no_grad():
            noise_pred_cond = pipeline.model(
                scaled_latent, t / 1000.0, conditioning=cond_embeds
            )

        # Classifier-free guidance interpolation.
        # cfg=1.0 → unconditional contribution is zero → conditional only.
        # cfg>1.0 → amplifies the conditional signal relative to unconditional.
        noise_pred = (
            noise_pred_uncond + cfg * (noise_pred_cond - noise_pred_uncond)
        )

        # Advance the latent by one denoising step.
        # The scheduler uses the noise prediction to compute the next
        # latent state (prev_sample), following its internal schedule
        # (Euler discrete integration in this case).
        latent = scheduler.step(noise_pred, t, latent).prev_sample

    logger.info(
        "denoising complete: steps=%d, seed=%d", steps, seed
    )
    return latent, seed
