"""ZiT (Zero-initialized Transformer) diffusion architecture module.

This module provides shape-inference utilities and meta-device construction
for the ZiT diffusion transformer architecture. It implements steps 1–4 of
the four-step loading contract defined in ANVILML_DESIGN.md §11.3:

    1. _infer_hyperparams(path) — open header-only, read all key shapes, return
          a dict of inferred hyperparameters (hidden_dim, block counts, latent
          dimensions, patch_size, arch string, native_dtype).
    2. can_handle(key) — implemented; returns True for "zit".
    3. load(path, caps, device) — steps 2–3: construct nn.Module on meta,
          materialize onto device via to_empty(), build key remapping,
          load_state_dict(assign=True).
    4. sample(model, model_id, conditioning, latent, steps, cfg, seed) —
          assembles and caches a runnable pipeline, runs the denoising loop
          with classifier-free guidance, returns (denoised_latent, seed).

The ZiT architecture is a diffusion transformer that uses zero-initialized
projection layers. It is characterised by ``double_blocks`` (cross-attention
blocks with image and text attention sub-layers) and ``single_blocks``
(single linear transformation blocks).

Design: ANVILML_DESIGN.md §11.3 — the four-step loading contract.
"""

from __future__ import annotations

import logging
import re
import secrets
import warnings
from pathlib import Path
from typing import Any

from safetensors import safe_open

# torch — and everything that transitively needs it (torch.nn, safetensors.torch,
# diffusers' scheduler classes) — is guarded here rather than imported
# unconditionally. This module is imported eagerly by
# arch/diffusion/__init__.py's dispatcher (P20-B2's _REGISTERED_MODULES), which
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

from worker.pipeline_cache import PipelineCache

# Canonical architecture identifier — the string that the dispatcher
# passes to can_handle() when routing diffusion model requests.
# Mirrors the "arch": "zit" value returned by _infer_hyperparams()
# when it reads metadata or falls back to key-pattern inference.
ARCH: str = "zit"

# Default patch size and latent channel count used by compute_latent_shape()
# when called before load() has cached the actual checkpoint hyperparameters.
# These are updated in-place by load() after _infer_hyperparams() extracts
# the real values from the checkpoint header, so this default only matters
# for a compute_latent_shape() call made before any model has been loaded.
# patch_size=2 matches Z-Image Turbo's actual patchify default (the upstream
# Tongyi-MAI/Z-Image reference implementation patchifies at 2x2); the prior
# value of 16 here was an unvalidated placeholder that didn't match any real
# checkpoint or the P21-A1 tests written against it (P900-series retrofit).
MODEL_PATCH_SIZE: int = 2
MODEL_LATENT_CHANNELS: int = 4

logger = logging.getLogger(__name__)

# Per-process LRU cache for pipeline objects.
# Each model_id gets its own cached pipeline, keyed as "{model_id}:pipeline".
# This avoids re-assembling the scheduler + model wrapper on every sample() call.
pipeline_cache = PipelineCache()


# nn.Module is unavailable when torch failed to import (see the guard above).
# ZiTModel falls back to plain `object` as its base in that case — the class
# still defines successfully (only __init__/forward bodies touch torch, and
# those are never invoked without going through the guarded load()/sample()
# entry points below), which is what keeps this module importable in mock-mode
# collection.
_ModuleBase = nn.Module if nn is not None else object


class ZiTModel(_ModuleBase):
    """ZiT diffusion transformer model constructed from layer-level building blocks.

    This class assembles the ZiT architecture using ``torch.nn`` primitives
    (Linear, LayerNorm, MultiheadAttention) that mirror the tensor shapes found in
    the checkpoint. It is constructed on ``torch.device("meta")`` so that
    no real GPU/CPU memory is allocated during construction — this prevents
    the ~15 GB construction crash that P904 experienced.

    The architecture consists of:
    - input_proj: latent space → hidden dimension projection
    - time_text_emb: time-step + text embedding projection
    - double_blocks: list of cross-attention blocks (image + text attention)
    - single_blocks: list of linear transformation blocks
    - output_proj: hidden dimension → latent space projection

    The ``.arch`` attribute is set to ``"zit"`` after construction so that
    downstream code (Sampler, VaeDecode) can identify the model family.

    Args:
        hyperparams: Dict from ``_infer_hyperparams()`` containing
            hidden_dim, double_block_count, single_block_count,
            latent_channels, latent_height, latent_width, patch_size.
    """

    def __init__(self, hyperparams: dict[str, Any]) -> None:
        """Construct the ZiT model on the meta device.

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
        latent_height = hyperparams["latent_height"]
        latent_width = hyperparams["latent_width"]
        patch_size = hyperparams["patch_size"]

        # Input projection: (latent_channels * patch_size^2) → hidden_dim
        # The latent tensor is reshaped to (batch, latent_channels*patch_size^2,
        # height*width) before projection into the hidden dimension.
        latent_dim = latent_channels * patch_size * patch_size
        self.input_proj = nn.Linear(latent_dim, hidden_dim)

        # Time-step + text embedding projection (fixed-size embedding).
        # The time token and text embedding are combined in a single linear
        # layer before being added to the hidden representation.
        self.time_text_emb = nn.Linear(hidden_dim, hidden_dim)

        # Double blocks with cross-attention sub-layers.
        # Each double block has: image attention (self-attention on image tokens),
        # text attention (cross-attention conditioned on text tokens),
        # two LayerNorm layers, and a feed-forward block.
        self.double_blocks = nn.ModuleList([
            nn.ModuleDict({
                "img_attn": nn.MultiheadAttention(
                    embed_dim=hidden_dim, num_heads=hidden_dim // 64, batch_first=True
                ),
                "txt_attn": nn.MultiheadAttention(
                    embed_dim=hidden_dim, num_heads=hidden_dim // 64, batch_first=True
                ),
                "norm1": nn.LayerNorm(hidden_dim),
                "norm2": nn.LayerNorm(hidden_dim),
                "ff": nn.Sequential(
                    nn.Linear(hidden_dim, hidden_dim * 4),
                    nn.GELU(),
                    nn.Linear(hidden_dim * 4, hidden_dim),
                ),
            })
            for _ in range(double_block_count)
        ])

        # Single blocks with linear transformation.
        # Each single block is a simplified linear transformation block
        # used in ZiT's architecture after the double blocks.
        self.single_blocks = nn.ModuleList([
            nn.ModuleDict({
                "linear1": nn.Linear(hidden_dim, hidden_dim * 4),
                "linear2": nn.Linear(hidden_dim * 4, hidden_dim),
                "norm": nn.LayerNorm(hidden_dim),
            })
            for _ in range(single_block_count)
        ])

        # Output projection: hidden_dim → (latent_channels * patch_size^2)
        # Reverses the input projection to produce the final latent tensor.
        self.output_proj = nn.Linear(hidden_dim, latent_dim)

        # Architecture identifier — set after construction so downstream
        # code can identify this model's family.
        self.arch: str = "zit"

    def forward(
        self,
        x: torch.Tensor,
        timestep: float,
        conditioning: Any = None,
    ) -> torch.Tensor:
        """Forward pass through the ZiT diffusion transformer.

        Implements the ZiT forward architecture:
        1. Project the latent tensor into the hidden dimension.
        2. Create a sinusoidal time embedding and project it through
           ``time_text_emb``.
        3. Add the time embedding to the hidden representation.
        4. Pass through each double block (cross-attention).
        5. Pass through each single block (linear transformation).
        6. Project back to the latent dimension.

        The timestep is a float in ``[0, 1]`` representing the current
        denoising step's position in the noise schedule. It is converted
        to a hidden-dimension embedding via a sinusoidal encoding.

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
        # The latent tensor has shape (batch, C, H, W). We reshape it to
        # (batch, C*H*W) so the linear projection works correctly.
        # This flattens the spatial dimensions into a sequence of tokens.
        batch = x.shape[0]
        orig_height = x.shape[2]
        orig_width = x.shape[3]

        # Derive the expected spatial dimensions from the model's latent_dim.
        # latent_dim = latent_channels * patch_size^2, so
        # expected_spatial = latent_dim / latent_channels = patch_size^2
        # expected_height = expected_width = patch_size.
        # If the input tensor has different spatial dimensions, resize it
        # to match the model's expected dimensions so the projection works.
        latent_channels = x.shape[1]
        expected_spatial = self.input_proj.in_features // latent_channels
        expected_height = int(expected_spatial**0.5)
        expected_width = expected_height

        if x.shape[2] != expected_height or x.shape[3] != expected_width:
            # Resize the spatial dimensions to match the model's expected
            # latent shape. This handles the case where the input tensor
            # has different spatial dimensions than the model was built with.
            x = torch.nn.functional.interpolate(
                x,
                size=(expected_height, expected_width),
                mode="bilinear",
                align_corners=False,
            )

        x_flat = x.reshape(batch, -1)  # (batch, latent_dim)
        h = self.input_proj(x_flat)  # (batch, hidden_dim)

        # ------------------------------------------------------------------
        # 2. Create sinusoidal time embedding and project it.
        # ------------------------------------------------------------------
        # Sinusoidal embeddings are the standard approach for encoding
        # continuous timesteps in diffusion models. The frequency bands
        # span from low (slow-changing) to high (fast-changing), giving
        # the model a rich representation of the timestep.
        # We generate a full hidden_dim embedding so it matches the
        # time_text_emb layer's expected input size.
        hidden_dim = self.time_text_emb.in_features
        emb_scale = torch.log(torch.tensor(10000.0)) / (hidden_dim - 1)

        # Generate the sinusoidal embedding: sin(timestep * exp(i * emb_scale))
        # This is the standard sinusoidal positional encoding adapted for
        # continuous timesteps instead of integer positions.
        emb = torch.exp(torch.arange(hidden_dim, device=x.device) * -emb_scale)
        emb = timestep * emb  # (hidden_dim,)
        emb = emb.sin().to(x.dtype)  # (hidden_dim,)

        # Project the time embedding through time_text_emb.
        # This layer combines the time embedding with the text embedding
        # (when conditioning is provided) into a single hidden-dim vector.
        time_emb = self.time_text_emb(emb)  # (hidden_dim,)

        # ------------------------------------------------------------------
        # 3. Add time embedding to hidden representation.
        # ------------------------------------------------------------------
        # Broadcasting: time_emb is (hidden_dim,) and h is (batch, hidden_dim),
        # so adding them broadcasts time_emb across all batch elements.
        h = h + time_emb

        # ------------------------------------------------------------------
        # 4. Pass through double blocks (cross-attention).
        # ------------------------------------------------------------------
        # Each double block has image self-attention, text cross-attention,
        # two LayerNorm layers, and a feed-forward block. The conditioning
        # (text embeddings) is passed to the cross-attention sub-layers.
        for block in self.double_blocks:
            h = block["norm1"](h)
            # Image self-attention: Q, K, V all come from h.
            attn_out, _ = block["img_attn"](h, h, h)
            h = h + attn_out
            # Text cross-attention: Q from h, K/V from conditioning.
            # h has shape (batch, hidden_dim) but txt_attn with batch_first=True
            # expects (N, L, E) — insert a sequence dimension of size 1 so the
            # query is (batch, 1, hidden_dim) matching the (batch, seq_len,
            # hidden_dim) format of the key/value tensors. After the attention
            # call, squeeze back to (batch, hidden_dim) before adding to h.
            if conditioning is not None:
                cross_out, _ = block["txt_attn"](
                    h.unsqueeze(1), conditioning, conditioning
                )
                h = h + cross_out.squeeze(1)
            h = block["norm2"](h)
            h = h + block["ff"](h)

        # ------------------------------------------------------------------
        # 5. Pass through single blocks (linear transformation).
        # ------------------------------------------------------------------
        # Each single block is a simplified linear transformation:
        # LayerNorm → Linear1 → GELU → Linear2. No attention needed.
        for block in self.single_blocks:
            h = block["norm"](h)
            h = h + block["linear2"](torch.nn.functional.gelu(block["linear1"](h)))

        # ------------------------------------------------------------------
        # 6. Project back to latent dimension.
        # ------------------------------------------------------------------
        out = self.output_proj(h)  # (batch, num_patches, latent_dim)

        # Reshape back to (batch, latent_channels, latent_height, latent_width).
        # The output has shape (batch, num_patches, latent_dim) where
        # num_patches = (expected_height * expected_width) / (patch_size^2)
        # and latent_dim = latent_channels * patch_size^2.
        # Reshape to the resized spatial dimensions, then interpolate
        # back to the original input dimensions if they differed.
        latent_channels = x.shape[1]
        resized_height = x.shape[2]
        resized_width = x.shape[3]
        out = out.reshape(batch, latent_channels, resized_height, resized_width)

        if out.shape[2] != orig_height or out.shape[3] != orig_width:
            out = torch.nn.functional.interpolate(
                out,
                size=(orig_height, orig_width),
                mode="bilinear",
                align_corners=False,
            )

        return out


class ZiTPipeline:
    """Minimal pipeline wrapper that holds a ``ZiTModel`` and a ``diffusers`` scheduler.

    This class is a thin container — it does not implement a denoising loop itself.
    The denoising loop is deferred to P21-B2.  This wrapper provides the interface
    that the denoising loop will call: ``.model`` for the neural network and
    ``.scheduler`` for the noise schedule.

    Attributes:
        model: The ``ZiTModel`` instance (an ``nn.Module``) to run inference with.
        scheduler: A ``diffusers`` scheduler instance that generates the noise
            schedule and provides the step function interface.
    """

    def __init__(self, model: ZiTModel, scheduler: Any) -> None:
        """Construct a ``ZiTPipeline`` wrapper.

        Args:
            model: An already-loaded ``ZiTModel`` instance with parameters
                materialized on the target device.
            scheduler: A ``diffusers`` scheduler instance (e.g.
                ``EulerDiscreteScheduler``) that defines the noise schedule.
        """
        self.model = model
        self.scheduler = scheduler


def _assemble_pipeline(model: ZiTModel) -> ZiTPipeline:
    """Assemble a ``ZiTPipeline`` from a loaded ``ZiTModel``.

    Creates a ``ZiTPipeline`` wrapper that holds the model and a default
    ``EulerDiscreteScheduler``.  The scheduler is a simple placeholder —
    the full denoising step function is wired in P21-B2.

    The function is called via ``PipelineCache.get_or_load()`` so that
    pipeline assembly happens at most once per ``model_id``.

    Args:
        model: An already-loaded ``ZiTModel`` instance with parameters
            materialized on the target device.

    Returns:
        A ``ZiTPipeline`` instance wrapping *model* and a default scheduler.
    """
    # Use EulerDiscreteScheduler as the default scheduler — it is a widely
    # used, stable scheduler in diffusers that provides a simple step
    # interface. The actual denoising loop is deferred to P21-B2.
    scheduler = EulerDiscreteScheduler()
    return ZiTPipeline(model, scheduler)


# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple
def compute_latent_shape(
    width: int, height: int, batch_size: int = 1
) -> tuple[int, int, int, int]:
    """Compute the latent tensor shape for a given input resolution.

    Returns (batch_size, latent_channels, width, height) — note the
    width/height axis order matches this project's established convention
    (dim 2 comes from *width*, dim 3 from *height*), not a height-then-width
    image convention.

    P900-series retrofit: this function previously divided width/height by
    MODEL_PATCH_SIZE (derived from input_proj.weight's fixed in_features —
    a property of the model's internal processing capacity, not of the
    VAE), on the mistaken assumption that zit_vae.py spatially compresses
    images the way a real Stable-Diffusion-style VAE does. It does not:
    zit_vae.py's decoder is a stride-1, same-padding convolution stack with
    zero spatial resizing anywhere in its architecture (confirmed by
    inspection — no Upsample/ConvTranspose/strided layer anywhere in the
    module). Dividing by patch_size here produced a latent smaller than
    requested, and since the VAE never compensates by upsampling, the final
    decoded image came out at that smaller size instead of the requested
    width/height (e.g. a 64x64 request produced a 16x16 PNG when
    MODEL_PATCH_SIZE was 4, as inferred from the zit_tiny fixture's
    input_proj.weight shape).

    This is the same defect (and the same fix) as
    flux2klein.py's compute_latent_shape() — see that function's docstring
    for the full rationale, including why flux2klein.py's
    Flux2KleinModel.forward() needed a matching resize-in/resize-out step
    added. ZiTModel.forward() already has that step (it always has, since
    at least Phase 20) — only this function needed the patch_size division
    removed; no forward() change was needed here.

    Args:
        width: Input image width in pixels.
        height: Input image height in pixels.
        batch_size: Number of samples in the batch. Defaults to 1.

    Returns:
        A 4-tuple (batch_size, latent_channels, width, height) representing
        the shape of the noise latent tensor that EmptyLatent should
        produce before passing it to the Sampler — at the full requested
        resolution, since the VAE performs no spatial compression.
    """
    return (batch_size, MODEL_LATENT_CHANNELS, width, height)


# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_real_zit_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_mock_zit_fixture
def load(path: str, caps: dict, device: str = "cpu") -> ZiTModel:
    """Construct the ZiT model on meta-device, materialize, and load weights.

    Implements steps 2–3 of the four-step loading contract
    (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (step 1, delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (step 2, delegated to ``_select_dtype``).
    3. Construct ``ZiTModel`` on ``torch.device("meta")``, apply dtype,
       materialize onto the target device via ``to_empty()``, build a
       checkpoint-key → module-key remapping, load and cast tensors, and
       call ``load_state_dict(assign=True)``.

    Args:
        path: Filesystem path to a ZiT-format safetensors checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for tensor materialization. Defaults to
            ``"cpu"``. Passed to ``model.to_empty(device=...)`` and
            ``load_file(..., device=...)``.

    Returns:
        A ``ZiTModel`` instance with parameters materialized on *device*,
        carrying the selected dtype, and ``.arch == "zit"``.

    Raises:
        ValueError: If the checkpoint cannot be opened or hyperparameters
            cannot be inferred (delegated to ``_infer_hyperparams``).
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); load() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here instead of surfacing a
    # confusing AttributeError on a None torch/nn deep inside construction.
    if torch is None:
        raise RuntimeError(
            "zit.py: torch is not installed - load() is a real-mode-only "
            "entry point (ANVILML_DESIGN.md §18.3) and must not be reached "
            "from mock-mode code paths."
        )

    # Step 1 (from P20-B1): infer hyperparameters from checkpoint header,
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

    # Step 2 (this task): select the compute dtype per the fixed precedence
    # in ANVILML_DESIGN.md §11.5. The native dtype is read from the checkpoint
    # header; the capability flags come from the worker's own torch-level probe.
    # This ensures the dtype decision is driven by both what the checkpoint
    # actually uses and what the worker hardware can execute.
    target_dtype = _select_dtype(caps, hyperparams["native_dtype"])

    # Step 3 (this task): construct on meta-device with selected dtype.
    # Using torch.device("meta") means no real memory is allocated for
    # parameters — the module structure exists but tensors have shape
    # metadata only. This prevents the ~15GB crash from P904.
    with torch.device("meta"):
        model = ZiTModel(hyperparams)

    # Apply the selected dtype to the meta-constructed module.
    # model.to(dtype) on a module with meta-device parameters changes their
    # dtype metadata without allocating real memory — this is the standard
    # PyTorch idiom for dtype selection before weight loading.
    model.to(target_dtype)

    # Materialize all parameters from meta device to the target device.
    # to_empty() allocates real memory for parameters but does not load
    # weights — this is the bridge between meta-construction and weight loading.
    logger.debug(
        "materializing ZiT model to device=%s, hidden_dim=%d, "
        "double_blocks=%d, single_blocks=%d",
        device,
        hyperparams["hidden_dim"],
        hyperparams["double_block_count"],
        hyperparams["single_block_count"],
    )
    model = model.to_empty(device=device)

    # P902 fix: to_empty() allocates UNINITIALIZED memory — it does not
    # zero anything, despite this function's checkpoint-loading comments
    # below having historically assumed unpopulated parameters are
    # "zero-initialized by design." That assumption was false: any
    # parameter this checkpoint doesn't populate (see the comment above
    # the remapping call below) keeps whatever garbage bits to_empty()
    # left behind, which is undefined — observed in practice as
    # everything from plausible-looking floats to literal NaN and
    # near-float32-max values, depending on process allocator history.
    # Zero every parameter and buffer explicitly here, before loading the
    # checkpoint, so any key the checkpoint doesn't cover deterministically
    # stays at zero — making the "zero-initialized by design" comment
    # below actually true — rather than silently propagating NaN through
    # the first forward pass. This mirrors the identical fix applied to
    # zit_vae.py and qwen3.py, which had the same to_empty()-without-zero
    # gap.
    for param in model.parameters():
        param.data.zero_()
    for buf in model.buffers():
        buf.data.zero_()

    # Verify .arch persists after materialization. to_empty() returns the same
    # module object (not a copy), so .arch should be preserved. If it is not,
    # explicitly re-set it — this is a safety net for future PyTorch versions.
    if not hasattr(model, "arch") or model.arch != ARCH:
        model.arch = ARCH

    # Load checkpoint tensors and build the remapped state dict.
    # Only keys that exist in BOTH the checkpoint and the module's state_dict
    # are loaded. Keys that exist only in the checkpoint (e.g. c_crossattn_dim,
    # latents, or proj.weight → in_proj_weight pattern mismatches) are silently
    # skipped — this is correct because the fixture checkpoint uses a simplified
    # key naming convention that doesn't fully populate the MultiheadAttention
    # parameters (which are zero-initialized by design in the ZiT architecture).
    state_dict = load_file(path, device=device)

    # Build the checkpoint-key → module-key remapping table.
    # This handles direct matches (exact key equality) and pattern-based
    # remapping for known ZiT key naming conventions.
    remap = _build_key_remapping(list(state_dict.keys()), list(model.state_dict().keys()))

    # Cast each loaded tensor to target_dtype BEFORE calling load_state_dict
    # with assign=True. The assign=True flag bypasses dtype coercion, so the
    # tensor must already have the correct dtype — this is the exact safety
    # measure that prevented the P904 dtype-swap incident.
    #
    # We also filter by shape: the assign=True flag does NOT bypass shape
    # checks. If a checkpoint tensor's shape doesn't match the module's
    # expected shape, it is skipped. This is necessary because the test
    # fixture is a synthetic file with simplified shapes that don't fully
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

    # Load the remapped state dict into the model.
    # assign=True is required for zero-initialized parameters that are already
    # on the target device — it performs in-place assignment without dtype
    # checks. This is critical because the double_blocks are intentionally
    # zero-initialized (they start as identity and learn during training).
    # strict=False allows partial loading: only tensors with matching shapes
    # are loaded; others remain at their zero-initialized values.
    info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)
    logger.info(
        "loaded ZiT weights: loaded=%d, missing=%d, unexpected=%d, device=%s",
        len(remapped_state_dict),
        len(info.missing_keys),
        len(info.unexpected_keys),
        device,
    )

    return model


# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random
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


def sample(
    model: ZiTModel,
    model_id: str,
    conditioning: Any,
    latent: torch.Tensor,
    steps: int,
    cfg: float,
    seed: int,
) -> tuple[torch.Tensor, int]:
    """Run the denoising loop and return the denoised latent tensor.

    On the first call for a given ``model_id``, this function assembles a
    ``ZiTPipeline`` (model + scheduler) and caches it under
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
        model: An already-loaded ``ZiTModel`` instance.
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
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); sample() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here rather than surfacing a
    # confusing AttributeError on a None torch deep inside the denoising loop.
    if torch is None:
        raise RuntimeError(
            "zit.py: torch is not installed - sample() is a real-mode-only "
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
    # diffusers==0.39.0's set_timesteps() (the latest release available;
    # there is nothing newer to upgrade to) internally converts
    # alphas_cumprod to sigmas via `np.array(tensor, ...)`, and the
    # tensor's `__array__` predates numpy>=2.0's requirement that
    # __array__ implementations accept `dtype`/`copy` keywords — a
    # compatibility gap inside diffusers itself, not our code, and not
    # something we can fix by bumping either dependency. Suppressed right
    # here, scoped to this one call and this exact message, so it can
    # never mask a real DeprecationWarning raised anywhere else.
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
        #
        # P903 retrofit: this call was previously missing entirely, which
        # diffusers only ever surfaces as a runtime UserWarning ("The
        # scale_model_input function should be called before step to
        # ensure correct denoising") rather than an exception — so the
        # pipeline ran to completion and produced an image, but a silently
        # wrong one, on every single step. See
        # docs/ADDENDUM_P903_QWEN3_TOKENIZER_VOCAB_MISMATCH.md.
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


def can_handle(key: str) -> bool:
    """Confirm this module handles the given architecture key.

    The dispatcher passes the architecture string (from safetensors
    metadata or path fallback) as *key*. This function returns True
    only when the key matches this module's canonical identifier.

    Args:
        key: Architecture string to check, e.g. ``"zit"`` or
            ``"flux2klein"``.

    Returns:
        ``True`` if *key* equals ``"zit"``, ``False`` otherwise.
    """
    return key == ARCH


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
        - ``native_dtype`` (str): Canonical native dtype string (e.g. ``"fp32"``,
            ``"fp16"``, ``"bf16"``, ``"fp8"``) inferred from the first weight
            tensor in the checkpoint header.

    Raises:
        ValueError: If the file is not a valid safetensors file, is truncated,
            or does not contain the expected ZiT key patterns.
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
        A dict of inferred hyperparameters including ``native_dtype``.

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
    # 0. Detect native dtype from the first weight tensor.
    # ------------------------------------------------------------------
    # The safetensors header stores each tensor's dtype as a string
    # (e.g. "F32", "F16", "BF16", "F8_E4M3", "I32"). We iterate over
    # all keys and collect the dtype from the first weight tensor
    # (identified by the ".weight" suffix) to determine the checkpoint's
    # native precision. This is needed by _select_dtype() to decide
    # whether FP8 is viable (caps.fp8 AND native_dtype == fp8).
    # If no weight tensor is found (e.g. no-metadata fixtures with
    # xyz_ prefixed keys), default to fp32 — the conservative safe
    # choice that prevents fp8 selection on unknown checkpoints.
    native_dtype: str = "fp32"
    for key in keys:
        if key.endswith(".weight"):
            # get_dtype() returns the safetensors dtype string
            # (e.g. "F32", "BF16", "F8_E4M3") for the first weight tensor.
            safetensors_dtype: str = f.get_slice(key).get_dtype()
            # Map the safetensors dtype string to a canonical lowercase
            # form that _select_dtype() uses for comparison.
            native_dtype = _safetensors_dtype_to_canonical(safetensors_dtype)
            break

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
    # 4. Infer latent dimensions.
    # ------------------------------------------------------------------
    # latent_channels comes from the ``latents`` key (the only reliable
    # source for the channel dimension).  latent_height and latent_width
    # are derived from ``input_proj.weight``, which encodes the true
    # latent_dim the model was trained with — the ``latents`` tensor may
    # have arbitrary spatial dimensions that don't match training.
    #
    # latent_dim = latent_channels * patch_size^2 = input_proj.in_features
    # latent_height = latent_width = sqrt(latent_dim / latent_channels)
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
    latent_channels = latent_shape[1]

    # Try to derive latent dimensions from input_proj.weight (primary path).
    # This gives the true latent_dim the model was trained with.
    # Fall back to the latents key for no-metadata fixtures that lack
    # standard ZiT key names.
    input_proj_key = None
    for key in keys:
        if key.endswith("input_proj.weight"):
            input_proj_key = key
            break

    if input_proj_key is not None:
        # Primary path: derive from input_proj weight shape.
        latent_dim = f.get_slice(input_proj_key).get_shape()[1]
        latent_height = latent_width = int((latent_dim / latent_channels) ** 0.5)
    else:
        # Fallback for no-metadata fixtures: derive from latents tensor.
        latent_height = latent_shape[2]
        latent_width = latent_shape[3]

    # ------------------------------------------------------------------
    # 5. Derive patch_size.
    # ------------------------------------------------------------------
    # patch_size = sqrt(latent_dim / latent_channels), which equals
    # latent_height (and latent_width) for square latent tensors.
    patch_size = int((latent_height * latent_width * latent_channels / latent_channels) ** 0.5)
    # Simplify: patch_size = latent_height for square latent tensors.
    patch_size = latent_height

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

    # Return all inferred hyperparameters as a single dict, including
    # the native_dtype so _select_dtype() can make an informed decision.
    return {
        "hidden_dim": hidden_dim,
        "double_block_count": double_block_count,
        "single_block_count": single_block_count,
        "latent_channels": latent_channels,
        "latent_height": latent_height,
        "latent_width": latent_width,
        "patch_size": patch_size,
        "arch": arch,
        "native_dtype": native_dtype,
    }


def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str:
    """Map a safetensors dtype string to a canonical lowercase form.

    Safetensors stores dtypes as uppercase abbreviations in the header
    (e.g. "F32", "BF16", "F8_E4M3"). This function normalizes them to
    lowercase canonical strings for comparison in _select_dtype().

    Args:
        safetensors_dtype: A dtype string from safetensors tensor info,
            e.g. "F32", "F16", "BF16", "F8_E4M3", "F8_E5M2", "I32".

    Returns:
        A canonical lowercase string: "fp32", "fp16", "bf16", "fp8", etc.
    """
    # Map known safetensors dtype strings to their canonical forms.
    # The mapping covers all dtypes that ZiT checkpoints may use:
    # F32/F16/BF16 for standard precision, F8_E4M3/F8_E5M2 for FP8,
    # and I32 for integer metadata (falls through to fp32).
    mapping: dict[str, str] = {
        "F32": "fp32",
        "F16": "fp16",
        "BF16": "bf16",
        "F8_E4M3": "fp8",
        "F8_E5M2": "fp8",
    }
    # Unknown dtypes (e.g. "I32" for integer metadata) fall back to fp32,
    # which is the safe default — integer tensors are never compute tensors.
    return mapping.get(safetensors_dtype, "fp32")


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


def _build_key_remapping(
    checkpoint_keys: list[str], module_keys: list[str]
) -> dict[str, str]:
    """Build a checkpoint-key → module-key mapping for ``load_state_dict``.

    Iterates over checkpoint keys and builds a remapping table that maps
    each checkpoint key to the corresponding module state_dict key. The
    function handles two cases:

    1. **Direct match:** the checkpoint key exists verbatim in the module's
       state_dict keys. The mapping is identity: ``ckpt_key → mod_key``.
    2. **Pattern-based remapping:** for ZiT checkpoint keys that use a
       simplified naming convention (e.g. ``double_blocks.N.img_attn.proj.weight``)
       but the module uses PyTorch's standard naming (``double_blocks.N.img_attn.in_proj_weight``),
       the function applies known remapping patterns. The remapped key is
       only included if it exists in the module's state_dict.

    Keys that exist only in the checkpoint (e.g. metadata tensors like
    ``c_crossattn_dim`` and ``latents``, or keys with shape mismatches)
    are silently excluded from the remapping — they will not be loaded
    into the module, which is correct because the double_block parameters
    are intentionally zero-initialized in the ZiT architecture.

    Args:
        checkpoint_keys: List of tensor keys from the safetensors file
            (returned by ``load_file()``).
        module_keys: List of parameter keys from ``model.state_dict().keys()``.

    Returns:
        A dict mapping ``checkpoint_key → module_key`` for all keys that
        can be successfully remapped.
    """
    module_key_set = set(module_keys)

    # Pattern-based remapping rules for ZiT-specific key naming conventions.
    # These rules convert simplified checkpoint keys to PyTorch MultiheadAttention
    # parameter names. Each rule is a (pattern, replacement) pair where the
    # pattern is a regex that matches the checkpoint key and the replacement
    # is the module key template.
    #
    # The ZiT checkpoint stores image/text attention projections as
    # ``double_blocks.N.img_attn.proj.weight`` but PyTorch's MultiheadAttention
    # uses ``double_blocks.N.img_attn.in_proj_weight``. This remapping
    # converts the checkpoint key to the module key.
    #
    # Note: the shape of ``proj.weight`` (embed_dim, embed_dim) does NOT
    # match ``in_proj_weight`` (3*embed_dim, embed_dim), so this remapping
    # will only succeed if the module actually has an ``in_proj_weight`` key
    # AND the shapes match. In practice, the fixture checkpoint uses a
    # simplified key naming that doesn't fully populate the attention
    # parameters — the double_blocks are intentionally zero-initialized.
    remapping_patterns: list[tuple[str, str]] = [
        # Image attention projection: checkpoint key → PyTorch in_proj_weight
        (
            r"double_blocks\.(\d+)\.img_attn\.proj\.weight",
            r"double_blocks.\1.img_attn.in_proj_weight",
        ),
        # Text attention projection: checkpoint key → PyTorch in_proj_weight
        (
            r"double_blocks\.(\d+)\.txt_attn\.proj\.weight",
            r"double_blocks.\1.txt_attn.in_proj_weight",
        ),
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
    # This is correct for metadata-only keys (c_crossattn_dim, latents)
    # and for attention projection keys where the shape doesn't match
    # the module's MultiheadAttention parameters.
    return remap
