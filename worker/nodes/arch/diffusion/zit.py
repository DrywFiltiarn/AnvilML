"""Z-Image Turbo (ZiT) FP8 architecture dispatch module.

This module provides architecture-specific dispatch for ZiT models,
including model detection via ``can_handle()`` and a mockable sampling
entry point via ``sample()``.

In mock mode (``ANVILML_WORKER_MOCK=1``), the ``sample()`` function
returns a lightweight ``MockLatent`` sentinel immediately without
importing torch, diffusers, or safetensors. The real sampling path
assembles a ``ZImagePipeline`` from cached components via
``pipeline_cache.get_or_load()`` and invokes it with
``callback_on_step_end(self, i, t, callback_kwargs)`` via the
pipeline's ``callback_on_step_end`` hook — the callback shape
matches the ``diffusers`` API, not the 2-arg ``emit_progress``
shape that ``sample()``'s own signature exposes (the adapter
between them is provided by ``_make_callback``).

The ``torch``, ``diffusers``, and ``safetensors`` packages must never
be imported at the top level of this module. Importing them here would
cause the worker to fail on systems without GPU hardware or these
libraries. Any real-mode imports must be inside the ``if not _mock:``
guard.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any, Callable

__all__ = [
    "can_handle",
    "compute_latent_shape",
    "load_transformer",
    "load_vae",
    "sample",
    "MockLatent",
    "VAE_SCALE_FACTOR",
]

# Z-Image-Turbo's published VAE config has block_out_channels=[128,256,512,512]
# (4 entries), giving 2**(4-1)=8 per ZImagePipeline.__init__'s vae_scale_factor
# formula; independently corroborated as 8x spatial compression
# (1024x1024 image -> 128x128 latent grid).
VAE_SCALE_FACTOR: int = 8


class _SamplingCancelled(Exception):
    """Sentinel exception raised when the sampling callback detects cancellation.

    This is a module-private exception (underscore-prefixed, not in ``__all__``).
    The ``_make_callback`` adapter raises it when ``cancel_flag.is_set()`` is true;
    P18-D18c's ``try/except _SamplingCancelled`` block around the pipeline call
    will catch it and propagate the cancellation to the caller.
    """

    pass


def _make_callback(
    emit_progress: Callable[[int, int], None],
    cancel_flag: Any,
    total_steps: int,
) -> Callable[[Any, int, int, dict[str, Any]], dict[str, Any]]:
    """Build a ``callback_on_step_end`` adapter for the diffusers pipeline.

    Bridges ``diffusers``' real ``callback_on_step_end`` signature
    ``(self, i, t, callback_kwargs) -> dict`` to the simpler
    2-argument ``emit_progress(step, total)`` interface that ``sample()``
    exposes to the rest of the codebase. The adapter calls
    ``emit_progress`` per step, checks a cancellation flag, and raises
    ``_SamplingCancelled`` if the sampling has been cancelled.

    The ``cancel_flag`` is expected to be a ``threading.Event`` (as
    specified in ``ANVILML_DESIGN.md §1550``); ``.is_set()`` is used
    to check whether cancellation was requested.

    Args:
        emit_progress: Callback invoked as ``emit_progress(step, total)``
            after each denoising step for progress reporting.
        cancel_flag: A ``threading.Event`` that is set when the job is
            cancelled. The adapter checks ``.is_set()`` on each step.
        total_steps: Total number of denoising steps (used as the second
            argument to ``emit_progress``).

    Returns:
        A closure with the signature
        ``(self, i, t, callback_kwargs) -> dict`` that matches
        ``diffusers``' ``callback_on_step_end`` API. On each call
        it emits progress, checks for cancellation, and returns
        ``callback_kwargs`` unchanged so that ``diffusers`` proceeds
        with its internal state unmodified.
    """

    def callback(
        self: Any,  # noqa: ARG001 — accepted for API compatibility with
        # diffusers (it passes the pipeline instance as self), but unused.
        i: int,
        t: Any,  # noqa: ARG001 — diffusers passes the current timestamp;
        # the adapter only needs the step index for progress reporting.
        callback_kwargs: dict[str, Any],
    ) -> dict[str, Any]:
        # Emit progress before checking cancellation — the caller needs
        # to see progress up to (and including) the step where cancellation
        # was detected.
        emit_progress(i, total_steps)

        # Check cancellation flag using threading.Event.is_set() as
        # specified in ANVILML_DESIGN.md §1550. The cancel_flag is a
        # threading.Event shared across all steps of a single sampling
        # run; the closure captures it by reference so changes made
        # between steps are observed.
        if cancel_flag.is_set():
            raise _SamplingCancelled("sampling cancelled at step {}".format(i))

        # Return callback_kwargs unchanged — diffusers expects the
        # callback to return the kwargs dict; returning it unmodified
        # means diffusers proceeds with its internal state unchanged.
        return callback_kwargs

    return callback


def compute_latent_shape(
    batch_size: int,
    height: int,
    width: int,
    num_channels_latents: int,
) -> tuple[int, ...]:
    """Compute the latent tensor shape for a ZiT diffusion pipeline.

    Implements the exact formula from ``ZImagePipeline.prepare_latents``:
    spatial dimensions are compressed by ``VAE_SCALE_FACTOR`` (8x), then
    the result is doubled to account for the 2× upsampling inherent in
    the VAE decoder's latent grid layout. Uses integer floor division
    so that non-divisible input dimensions silently floor rather than
    raising — matching ``ZImagePipeline.prepare_latents``'s behaviour.

    This function is architecture-specific; Flux 2 Klein (P18-D3) has a
    structurally different formula that packs latents into 2×2 patches.

    Args:
        batch_size: Number of independent generations in this batch.
        height: Input image height in pixels.
        width: Input image width in pixels.
        num_channels_latents: Number of latent channels (4 for standard
            SD-style models).

    Returns:
        A 4-tuple ``(batch_size, num_channels_latents, h, w)`` that
        corresponds to the shape validated by
        ``ZImagePipeline.prepare_latents``. The spatial dimensions
        ``h`` and ``w`` may be zero for very small inputs (e.g.
        height < 16) — the caller must validate that the result is
        a usable latent shape before passing it to the pipeline.

    # defers_to: P18-D17 — consumed by EmptyLatent real path
    """
    # Compute spatial dimensions using the VAE's 8× spatial
    # compression factor. The formula doubles the floor-divided
    # result to match ZImagePipeline.prepare_latents' latent grid
    # layout (the VAE decoder upsamples by 2× from latent to pixel).
    h = 2 * (height // (VAE_SCALE_FACTOR * 2))
    w = 2 * (width // (VAE_SCALE_FACTOR * 2))
    return (batch_size, num_channels_latents, h, w)


class MockLatent:
    """Sentinel latent object for ZiT mock mode.

    A lightweight placeholder used by the ZiT arch module's mock path.
    Unlike ``worker.nodes.sampler.MockLatent``, this class carries no
    dimension attributes — it is a bare sentinel because the arch
    module operates at a higher abstraction level where dimensions
    are already resolved by the node graph.

    Arch modules must not import from ``sampler.py`` to maintain
    architectural isolation: arch modules are architecture-specific
    and should remain independent of node-level concerns.
    """

    pass


def can_handle(model_obj: Any) -> bool:
    """Check whether this model uses the ZiT architecture.

    Inspects the ``arch`` attribute on the model object and returns
    ``True`` if it matches the string ``"zit"``, indicating a
    Z-Image Turbo model that uses the FP8 diffusion pipeline.

    Args:
        model_obj: A model descriptor object. Must have an ``arch``
            attribute set to a string identifying the model's
            architecture type.

    Returns:
        ``True`` if ``model_obj.arch == "zit"``, ``False`` otherwise.
        Returns ``False`` if the model object lacks an ``arch``
        attribute entirely.
    """
    # Check if the model object has an arch attribute before
    # accessing it. getattr returns None if the attribute is
    # absent, preventing AttributeError on arbitrary model objects.
    arch = getattr(model_obj, "arch", None)

    # Match against the ZiT architecture string. This is the
    # canonical architecture identifier for Z-Image Turbo models
    # that use the FP8 diffusion pipeline with ZImagePipeline.
    return arch == "zit"


def _infer_config_from_checkpoint(checkpoint: dict[str, Any]) -> dict[str, Any]:
    """Infer the ``ZImageTransformer2DModel`` config from raw checkpoint tensor shapes.

    Derives architecture parameters directly from tensor shapes in the checkpoint,
    following the same pattern used by ComfyUI's ZiT loader. This eliminates the
    need for a ``config.json`` or any HuggingFace network access.

    Shape inference rules:

    * ``dim`` — first dimension of ``attention.out.weight`` (e.g. ``[3840, 3840]`` → ``3840``).
    * ``head_dim`` — first dimension of ``attention.q_norm.weight`` (e.g. ``[128]`` → ``128``).
    * ``n_heads`` — ``dim // head_dim`` (e.g. ``3840 // 128 = 30``).
    * ``n_kv_heads`` — same as ``n_heads``; ZiT has no grouped-query attention.
    * ``n_layers`` — count of unique ``layers.N.`` key prefixes (e.g. ``layers.0`` through ``layers.29`` → ``30``).
    * ``n_refiner_layers`` — count of unique ``context_refiner.N.`` prefixes (e.g. ``context_refiner.0``, ``context_refiner.1`` → ``2``).
    * ``cap_feat_dim`` — first dimension of ``cap_embedder.0.weight`` (e.g. ``[2560]`` → ``2560``).
    * ``in_channels`` — ``final_layer.linear.weight.shape[0] // (patch_size**2 * f_patch_size)``.
      The registered defaults are ``all_patch_size=(2,)`` and ``all_f_patch_size=(1,)``,
      so ``in_channels = weight_dim // 4`` (e.g. ``64 // 4 = 16``).
    * ``all_patch_size``, ``all_f_patch_size`` — registered defaults ``(2,)`` and ``(1,)``.

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file.
            Keys are in the raw ComfyUI format (e.g. ``model.diffusion_model.layers.0...``).

    Returns:
        A dict with keys ``dim``, ``head_dim``, ``n_heads``, ``n_kv_heads``,
        ``n_layers``, ``n_refiner_layers``, ``cap_feat_dim``, ``in_channels``,
        ``all_patch_size``, and ``all_f_patch_size``.

    Raises:
        ValueError: If required keys are absent or have unexpected shapes.
    """
    # Find attention.out.weight to derive dim (model hidden dimension).
    # The first dimension is the model dim (3840 for the 6B ZiT).
    attention_out_key = None
    for key in checkpoint:
        if "attention.out.weight" in key:
            attention_out_key = key
            break

    if attention_out_key is None:
        raise ValueError(
            "Cannot infer dim: no 'attention.out.weight' key found in checkpoint"
        )

    dim = checkpoint[attention_out_key].shape[0]

    # Find attention.q_norm.weight to derive head_dim (per-head dimension).
    # The first dimension is the head dim (128 for the 6B ZiT).
    q_norm_key = None
    for key in checkpoint:
        if "q_norm.weight" in key:
            q_norm_key = key
            break

    if q_norm_key is None:
        raise ValueError(
            "Cannot infer head_dim: no 'q_norm.weight' key found in checkpoint"
        )

    head_dim = checkpoint[q_norm_key].shape[0]

    # n_heads and n_kv_heads are derived from dim / head_dim.
    # ZiT uses standard multi-head attention (no grouped-query attention),
    # so n_kv_heads == n_heads.
    n_heads = dim // head_dim
    n_kv_heads = n_heads

    # Count transformer layers by scanning for unique layer indices.
    # Keys like "layers.0.attn.qkv.weight" → extract "0" → layer 0.
    layer_indices: set[int] = set()
    refiner_indices: set[int] = set()

    for key in checkpoint:
        # Extract layer index from "layers.N." key pattern.
        if ".layers." in key:
            parts = key.split(".layers.")
            if len(parts) > 1:
                idx_str = parts[1].split(".")[0]
                try:
                    layer_indices.add(int(idx_str))
                except ValueError:
                    pass
        # Extract refiner index from "context_refiner.N." key pattern.
        elif ".context_refiner." in key:
            parts = key.split(".context_refiner.")
            if len(parts) > 1:
                idx_str = parts[1].split(".")[0]
                try:
                    refiner_indices.add(int(idx_str))
                except ValueError:
                    pass

    n_layers = max(layer_indices) + 1 if layer_indices else 30
    n_refiner_layers = max(refiner_indices) + 1 if refiner_indices else 2

    # Find cap_embedder.0.weight to derive cap_feat_dim (conditioning feature dimension).
    cap_feat_dim = None
    for key in checkpoint:
        if "cap_embedder.0.weight" in key:
            cap_feat_dim = checkpoint[key].shape[0]
            break

    if cap_feat_dim is None:
        raise ValueError(
            "Cannot infer cap_feat_dim: no 'cap_embedder.0.weight' key found in checkpoint"
        )

    # Find final_layer.linear.weight to derive in_channels.
    # The output dimension equals in_channels * patch_size^2 * f_patch_size.
    # With registered defaults all_patch_size=(2,) and all_f_patch_size=(1,),
    # in_channels = weight_dim // (2^2 * 1) = weight_dim // 4.
    linear_key = None
    for key in checkpoint:
        if "final_layer.linear.weight" in key:
            linear_key = key
            break

    if linear_key is None:
        raise ValueError(
            "Cannot infer in_channels: no 'final_layer.linear.weight' key found in checkpoint"
        )

    all_patch_size = (2,)
    all_f_patch_size = (1,)
    in_channels = checkpoint[linear_key].shape[0] // (all_patch_size[0] ** 2 * all_f_patch_size[0])

    return {
        "dim": dim,
        "head_dim": head_dim,
        "n_heads": n_heads,
        "n_kv_heads": n_kv_heads,
        "n_layers": n_layers,
        "n_refiner_layers": n_refiner_layers,
        "cap_feat_dim": cap_feat_dim,
        "in_channels": in_channels,
        "all_patch_size": all_patch_size,
        "all_f_patch_size": all_f_patch_size,
    }


def _remap_z_image_keys(checkpoint: dict[str, Any]) -> dict[str, Any]:
    """Remap raw ZiT checkpoint keys to the diffusers ``ZImageTransformer2DModel`` convention.

    Applies three transformations to the checkpoint state dict:

    1. **Key renaming** — sequential string replacement per key using the
       same dictionary as the diffusers source:

       * ``final_layer.`` → ``all_final_layer.2-1.``
       * ``x_embedder.`` → ``all_x_embedder.2-1.``
       * ``.attention.out.bias`` → ``.attention.to_out.0.bias``
       * ``.attention.k_norm.weight`` → ``.attention.norm_k.weight``
       * ``.attention.q_norm.weight`` → ``.attention.norm_q.weight``
       * ``.attention.out.weight`` → ``.attention.to_out.0.weight``
       * ``model.diffusion_model.`` → ``""`` (stripped)

    2. **``norm_final.weight`` removal** — popped if present.

    3. **QKV defuse** — any key containing ``.attention.qkv.weight`` is popped,
       its tensor is split via ``torch.chunk(..., 3, dim=0)`` into three tensors
       named ``to_q.weight``, ``to_k.weight``, ``to_v.weight``.

    Operates on a copy of the checkpoint to avoid mutating the original.

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file.

    Returns:
        A new dict with remapped keys suitable for
        ``ZImageTransformer2DModel.load_state_dict()``.
    """
    # Work on a shallow copy so the original checkpoint is never mutated.
    state_dict = dict(checkpoint)

    # Apply key-remap dictionary: sequential string replacement per key.
    # Each replacement is applied to every key — the order matches
    # diffusers' Z_IMAGE_KEYS_RENAME_DICT exactly.
    Z_IMAGE_KEYS_RENAME_DICT: list[tuple[str, str]] = [
        ("final_layer.", "all_final_layer.2-1."),
        ("x_embedder.", "all_x_embedder.2-1."),
        (".attention.out.bias", ".attention.to_out.0.bias"),
        (".attention.k_norm.weight", ".attention.norm_k.weight"),
        (".attention.q_norm.weight", ".attention.norm_q.weight"),
        (".attention.out.weight", ".attention.to_out.0.weight"),
        ("model.diffusion_model.", ""),
    ]

    renamed: dict[str, Any] = {}
    for key, value in state_dict.items():
        new_key = key
        # Apply each replacement sequentially to the same key.
        for old, new in Z_IMAGE_KEYS_RENAME_DICT:
            new_key = new_key.replace(old, new)
        renamed[new_key] = value

    # Remove norm_final.weight if present — it is not used by
    # ZImageTransformer2DModel and was removed in the diffusers source.
    renamed.pop("norm_final.weight", None)

    # QKV defuse: split fused qkv tensors into separate to_q/to_k/to_v.
    # This handles any key containing ".attention.qkv.weight" (the key
    # may have additional prefixes like "layers.0." or "model.diffusion_model.").
    # torch.chunk is needed here; import it locally to avoid top-level torch
    # import (which would break mock-mode import isolation).
    import torch

    for key, value in list(renamed.items()):
        if ".attention.qkv.weight" not in key:
            continue
        # Pop the fused tensor and split into three equal parts along dim 0.
        fused_qkv = renamed.pop(key)
        to_q_weight, to_k_weight, to_v_weight = torch.chunk(fused_qkv, 3, dim=0)
        renamed[key.replace(".attention.qkv.weight", ".attention.to_q.weight")] = to_q_weight
        renamed[key.replace(".attention.qkv.weight", ".attention.to_k.weight")] = to_k_weight
        renamed[key.replace(".attention.qkv.weight", ".attention.to_v.weight")] = to_v_weight

    return renamed


def _infer_vae_config_from_checkpoint(checkpoint: dict[str, Any]) -> dict[str, Any]:
    """Infer the ``AutoencoderKL`` config from raw LDM-format checkpoint tensor shapes.

    Derives architecture parameters directly from tensor shapes in the checkpoint,
    following the same shape-inference pattern used by ``_infer_config_from_checkpoint``
    for the transformer. This eliminates the need for a hardcoded ``block_out_channels``
    list and makes the loader robust to VAE variants with different stage counts.

    Shape inference rules (confirmed against real VAE checkpoint scans):

    * ``latent_channels`` — second dimension of ``decoder.conv_in.weight``
      (e.g. ``[512, 16, 3, 3]`` → ``16``).
    * ``block_out_channels`` — first dimension of ``decoder.up.{N}.block.{M}.conv1.weight``
      for each unique stage index ``N``, sorted ascending (e.g. ``[128, 256, 512, 512]``).
    * ``in_channels`` — second dimension of ``encoder.conv_in.weight``
      (e.g. ``[64, 3, 3, 3]`` → ``3``).
    * ``out_channels`` — first dimension of ``decoder.conv_out.weight``
      (e.g. ``[64, 64, 3, 3]`` → ``64``).
    * ``layers_per_block`` — count of unique block indices ``M`` in
      ``decoder.up.{N}.block.{M}.conv1.weight`` for any ``N``, minus 1
      (diffusers' ``Decoder`` uses ``num_layers = layers_per_block + 1`` for up-blocks;
      3 observed resnets → ``layers_per_block = 2``).

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file.
            Keys are in the raw LDM format (e.g. ``vae.encoder.down.0...``).

    Returns:
        A dict with keys ``latent_channels``, ``block_out_channels``,
        ``in_channels``, ``out_channels``, and ``layers_per_block``.

    Raises:
        ValueError: If any required key is absent from the checkpoint.
    """
    # --- latent_channels: decoder.conv_in.weight[1] ---
    # The decoder conv_in projects from the latent space to the first decoder stage.
    # Shape is [out_channels, latent_channels, kernel_h, kernel_w], e.g. [512, 16, 3, 3].
    latent_channels = None
    for key in checkpoint:
        if "decoder.conv_in.weight" in key:
            latent_channels = checkpoint[key].shape[1]
            break

    if latent_channels is None:
        raise ValueError(
            "Cannot infer latent_channels: "
            "no 'decoder.conv_in.weight' key found in checkpoint"
        )

    # --- block_out_channels: decoder.up.{N}.block.{M}.conv1.weight ---
    # Scan for unique stage indices N and extract the channel count for each.
    # This dynamically discovers the number of stages rather than hardcoding 4.
    stage_channels: dict[int, int] = {}
    for key in checkpoint:
        # Match keys like "decoder.up.0.block.0.conv1.weight"
        if "decoder.up." in key and ".block." in key and ".conv1.weight" in key:
            parts = key.split("decoder.up.")
            if len(parts) < 2:
                continue
            remainder = parts[1]
            stage_idx_str = remainder.split(".")[0]
            try:
                stage_idx = int(stage_idx_str)
            except ValueError:
                continue
            # First dimension of conv1 weight is the output channels for this stage
            stage_channels[stage_idx] = checkpoint[key].shape[0]

    if not stage_channels:
        raise ValueError(
            "Cannot infer block_out_channels: "
            "no 'decoder.up.*.block.*.conv1.weight' keys found in checkpoint"
        )

    block_out_channels = [
        stage_channels[n] for n in sorted(stage_channels.keys())
    ]

    # --- in_channels: encoder.conv_in.weight[1] ---
    # The encoder conv_in projects from the input image channels to the first stage.
    # Shape is [out_channels, in_channels, kernel_h, kernel_w], e.g. [64, 3, 3, 3].
    in_channels = None
    for key in checkpoint:
        if "encoder.conv_in.weight" in key:
            in_channels = checkpoint[key].shape[1]
            break

    if in_channels is None:
        raise ValueError(
            "Cannot infer in_channels: "
            "no 'encoder.conv_in.weight' key found in checkpoint"
        )

    # --- out_channels: decoder.conv_out.weight[0] ---
    # The decoder final conv projects from the last stage back to image channels.
    # Shape is [out_channels, in_channels, kernel_h, kernel_w], e.g. [64, 64, 3, 3].
    out_channels = None
    for key in checkpoint:
        if "decoder.conv_out.weight" in key:
            out_channels = checkpoint[key].shape[0]
            break

    if out_channels is None:
        raise ValueError(
            "Cannot infer out_channels: "
            "no 'decoder.conv_out.weight' key found in checkpoint"
        )

    # --- layers_per_block: unique block indices in decoder up-blocks, minus 1 ---
    # Diffusers' Decoder class uses num_layers = layers_per_block + 1 for up-blocks.
    # So if we observe 3 resnet blocks per stage, layers_per_block = 3 - 1 = 2.
    block_indices: set[int] = set()
    for key in checkpoint:
        if "decoder.up." in key and ".block." in key and ".conv1.weight" in key:
            parts = key.split("decoder.up.")
            if len(parts) < 2:
                continue
            remainder = parts[1]
            # remainder is like "0.block.2.conv1.weight"
            block_parts = remainder.split(".block.")
            if len(block_parts) < 2:
                continue
            block_idx_str = block_parts[1].split(".")[0]
            try:
                block_idx = int(block_idx_str)
            except ValueError:
                continue
            block_indices.add(block_idx)

    if not block_indices:
        raise ValueError(
            "Cannot infer layers_per_block: "
            "no 'decoder.up.*.block.*.conv1.weight' keys found in checkpoint"
        )

    layers_per_block = max(block_indices) + 1 - 1  # -1 offset for diffusers convention

    return {
        "latent_channels": latent_channels,
        "block_out_channels": block_out_channels,
        "in_channels": in_channels,
        "out_channels": out_channels,
        "layers_per_block": layers_per_block,
    }


def _remap_ldm_vae_keys(checkpoint: dict[str, Any]) -> dict[str, Any]:
    """Remap raw LDM-format VAE checkpoint keys to the diffusers ``AutoencoderKL`` convention.

    Applies three transformations to the checkpoint state dict:

    1. **Prefix stripping** — removes leading ``vae.`` and ``first_stage_model.``
       prefixes (both are common LDM checkpoint prefix styles).
    2. **Encoder down-block remap** — converts ``encoder.down.{N}.block.{M}.conv{1,2}.weight``
       to ``encoder.down_blocks.{N}.resnets.{M}.conv{1,2}.weight``. Similarly for norm
       layers. Also remaps ``encoder.down.{N}.block.{M}.conv_down.weight`` to
       ``encoder.down_blocks.{N}.conv_down.weight``.
    3. **Mid block remap** — converts ``decoder.mid.block_{1,2}.conv{1,2}.weight`` to
       ``decoder.mid_block.resnets.{0,1}.conv{1,2}.weight``.
    4. **Decoder up-block remap** — converts ``decoder.up.{N}.block.{M}.conv{1,2}.weight``
       to ``decoder.up_blocks.{N}.resnets.{M}.conv{1,2}.weight``. Also remaps
       ``decoder.up.{N}.block.{M}.conv_up.weight`` to
       ``decoder.up_blocks.{N}.conv_upsample.weight``.
    5. **Final conv** — ``decoder.conv_out.weight`` passes through unchanged
       (already in diffusers format).

    Operates on a shallow copy to avoid mutating the original checkpoint.

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file.
            Keys are in the raw LDM format (e.g. ``vae.encoder.down.0...``).

    Returns:
        A new dict with remapped keys suitable for
        ``AutoencoderKL.load_state_dict()``.
   """
    import re

    # Work on a shallow copy so the original checkpoint is never mutated.
    state_dict = dict(checkpoint)
    remapped: dict[str, Any] = {}

    for key, value in state_dict.items():
        new_key = key

        # Strip leading 'vae.' prefix (common LDM checkpoint prefix style).
        if new_key.startswith("vae."):
            new_key = new_key[4:]

        # Strip leading 'first_stage_model.' prefix (alternative LDM prefix style).
        if new_key.startswith("first_stage_model."):
            new_key = new_key[len("first_stage_model."):]

        # Encoder down-block conv/norm: encoder.down.{N}.block.{M}.conv{1,2}.norm{1,2}.<suffix>
        # Match: encoder.down.{N}.block.{M}.conv{1,2}.norm{1,2}.<suffix>
        if re.match(r"^encoder\.down\.\d+\.block\.\d+\.conv[12]\.norm[12]\.", new_key):
            # Replace encoder.down.N.block.M with encoder.down_blocks.N.resnets.M
            new_key = re.sub(
                r"^encoder\.down\.(\d+)\.block\.(\d+)",
                r"encoder.down_blocks.\1.resnets.\2",
                new_key,
            )

        # Match: encoder.down.{N}.block.{M}.conv{1,2}.<suffix> (not norm)
        elif re.match(r"^encoder\.down\.\d+\.block\.\d+\.conv[12]\.", new_key):
            new_key = re.sub(
                r"^encoder\.down\.(\d+)\.block\.(\d+)",
                r"encoder.down_blocks.\1.resnets.\2",
                new_key,
            )

        # Match: encoder.down.{N}.block.{M}.conv_down.<suffix>
        elif re.match(r"^encoder\.down\.\d+\.block\.\d+\.conv_down\.", new_key):
            new_key = re.sub(
                r"^encoder\.down\.(\d+)\.block\.\d+",
                r"encoder.down_blocks.\1",
                new_key,
            )

        # Mid block: decoder.mid.block_{1,2}.conv{1,2}.<suffix>
        # → decoder.mid_block.resnets.{0,1}.conv{1,2}.<suffix>
        elif re.match(r"^decoder\.mid\.block_(\d+)\.conv([12])\.", new_key):
            block_num = int(re.match(r"^decoder\.mid\.block_(\d+)", new_key).group(1))
            # block_1 → resnet 0, block_2 → resnet 1
            resnet_idx = block_num - 1
            new_key = re.sub(
                r"^decoder\.mid\.block_\d+",
                f"decoder.mid_block.resnets.{resnet_idx}",
                new_key,
            )

        # Decoder up-block conv/norm: decoder.up.{N}.block.{M}.conv{1,2}.norm{1,2}.<suffix>
        elif re.match(r"^decoder\.up\.\d+\.block\.\d+\.conv[12]\.norm[12]\.", new_key):
            new_key = re.sub(
                r"^decoder\.up\.(\d+)\.block\.(\d+)",
                r"decoder.up_blocks.\1.resnets.\2",
                new_key,
            )

        # Decoder up-block conv: decoder.up.{N}.block.{M}.conv{1,2}.<suffix> (not norm)
        elif re.match(r"^decoder\.up\.\d+\.block\.\d+\.conv[12]\.", new_key):
            new_key = re.sub(
                r"^decoder\.up\.(\d+)\.block\.(\d+)",
                r"decoder.up_blocks.\1.resnets.\2",
                new_key,
            )

        # Decoder upsample: decoder.up.{N}.block.{M}.conv_up.<suffix>
        # → decoder.up_blocks.{N}.conv_upsample.<suffix>
        elif re.match(r"^decoder\.up\.\d+\.block\.\d+\.conv_up\.", new_key):
            new_key = re.sub(
                r"^decoder\.up\.(\d+)\.block\.\d+\.conv_up",
                r"decoder.up_blocks.\1.conv_upsample",
                new_key,
            )

        # All other keys pass through unchanged (including decoder.conv_out.weight)
        remapped[new_key] = value

    return remapped


def load_transformer(model_id: str) -> Any:
    """Load a Z-Image Turbo (ZiT) transformer from a raw ``.safetensors`` file.

    Infers the ``ZImageTransformer2DModel`` configuration directly from the raw
    checkpoint's tensor shapes (the ComfyUI pattern) — no ``config.json`` or
    HuggingFace network access required. The six shape-inferable parameters
    (``dim``, ``in_channels``, ``n_layers``, ``n_refiner_layers``, ``n_heads``,
    ``n_kv_heads``, ``cap_feat_dim``) are derived from tensor dimensions; the
    remaining scalar hyperparameters (``norm_eps``, ``rope_theta``, ``t_scale``,
    ``axes_dims``, ``axes_lens``, ``qk_norm``) are hardcoded constants.

    Keys are remapped from the raw ComfyUI format to the diffusers convention
    using a local key-remap dictionary and QKV-defuse logic, matching the
    transformations that the diffusers source performs — implemented manually
    to avoid any dependency on diffusers internals.

    This function performs **zero network calls**. It never queries HuggingFace
    for a ``config.json`` or any other remote resource — all architecture
    parameters are derived from the checkpoint file, and the file is read locally.

    In mock mode (``ANVILML_WORKER_MOCK=1``), returns ``None`` immediately
    without importing torch, diffusers, or safetensors.

    Args:
        model_id: Path to a ``.safetensors`` file containing raw ZiT
            transformer weights (the format produced by
            ``ZImageTransformer2DModel.state_dict()`` — fused QKV keys,
            ``model.diffusion_model.`` prefix, etc.).

    Returns:
        A ``ZImageTransformer2DModel`` instance with weights loaded and
        remapped, or ``None`` in mock mode.

    Raises:
        OSError: If the file at ``model_id`` does not exist or is
            inaccessible.
        ValueError: If the checkpoint is malformed and cannot be remapped
            or loaded into the model's state dict.
    """
    # Check mock mode by inspecting the environment variable.
    # This must be a runtime check (not a module-level import)
    # so that CI tests running with ANVILML_WORKER_MOCK=1
    # never touch torch/diffusers/safetensors at import time.
    _mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

    if _mock:
        # In mock mode, return None immediately without importing
        # torch, diffusers, or safetensors. This keeps tests fast
        # and avoids requiring GPU hardware or these heavy deps.
        return None

    # Real mode: lazy-import all heavy dependencies inside the
    # real-mode branch to preserve mock-mode import isolation.
    import torch
    from diffusers import ZImageTransformer2DModel
    from safetensors.torch import load_file as safetensors_load_file

    # Load the raw checkpoint from the .safetensors file.
    # The raw format contains fused QKV keys and the
    # model.diffusion_model. prefix — our manual remap handles both.
    checkpoint = safetensors_load_file(model_id)

    # Infer model config directly from checkpoint tensor shapes.
    # This replaces the previous approach of relying on the class's
    # registered defaults — now we derive dim, n_layers, etc. from
    # the actual checkpoint, making the loader robust to different
    # ZiT variants without hardcoding architecture constants.
    config = _infer_config_from_checkpoint(checkpoint)

    # Construct the model with explicitly-inferred parameters.
    # Hardcoded scalar constants that are never stored as weights.
    #
    # axes_dims must sum to exactly head_dim (ZImageTransformer2DModel's
    # own __init__ asserts this) — the real model's registered default
    # [32, 48, 48] only sums to 128, which only matches a real-scale
    # checkpoint's head_dim by coincidence. For any other head_dim (every
    # tiny test fixture), the unscaled real-model default raises
    # AssertionError on construction. Scale proportionally to the real
    # model's own ratio (32:48:48 == 0.25:0.375:0.375) so the sum always
    # matches the inferred head_dim. This is a best-effort heuristic, not
    # a verified derivation — the *ratio* between the three axes may
    # encode something architecturally meaningful (e.g. separate
    # temporal/height/width RoPE allocations) that proportional scaling
    # does not necessarily preserve correctly; this has not been checked
    # against real multi-axis-RoPE behavior at inference time, only
    # confirmed to make construction succeed instead of crash. Revisit
    # if/when this becomes a verified, not heuristic, derivation.
    # axes_lens is left untouched — it governs maximum sequence length
    # per axis, not head dimension, so it is not coupled to head_dim and
    # the real model's hardcoded values remain appropriate regardless.
    _real_axes_dims = (32, 48, 48)
    _real_head_dim = sum(_real_axes_dims)
    axes_dims = [
        round(d * config["head_dim"] / _real_head_dim) for d in _real_axes_dims
    ]
    # Rounding can leave the sum off by one; correct the last entry so
    # the construction-time assertion (sum(axes_dims) == head_dim) holds
    # exactly rather than failing on an off-by-one from float rounding.
    axes_dims[-1] += config["head_dim"] - sum(axes_dims)

    # Memory-safe construction: build on torch.device("meta") with the
    # default dtype temporarily set to bfloat16, so to_empty() materializes
    # real storage directly at bf16 — never at fp32 first. The real ZiT
    # checkpoint is FP8 (e4m3) on disk; CPU PyTorch does not support FP8
    # compute (confirmed: nn.Linear at float8_e4m3fn raises
    # NotImplementedError on this platform), so the model itself must be
    # bf16, with the FP8-stored checkpoint values cast on load — this
    # preserves the already-quantized values bit-for-bit (FP8 -> bf16 is
    # a widening cast, not a re-quantization) while making the weights
    # usable for real CPU compute.
    original_default_dtype = torch.get_default_dtype()
    torch.set_default_dtype(torch.bfloat16)
    try:
        with torch.device("meta"):
            model = ZImageTransformer2DModel(
                dim=config["dim"],
                in_channels=config["in_channels"],
                n_layers=config["n_layers"],
                n_refiner_layers=config["n_refiner_layers"],
                n_heads=config["n_heads"],
                n_kv_heads=config["n_kv_heads"],
                cap_feat_dim=config["cap_feat_dim"],
                all_patch_size=config["all_patch_size"],
                all_f_patch_size=config["all_f_patch_size"],
                norm_eps=1e-5,
                rope_theta=256.0,
                t_scale=1000.0,
                axes_dims=axes_dims,
                axes_lens=[1024, 512, 512],
                qk_norm=True,
            )
    finally:
        torch.set_default_dtype(original_default_dtype)
    model = model.to_empty(device="cpu")

    # Remap keys from the raw ComfyUI format to the diffusers convention.
    # This handles prefix renaming, norm_final.weight removal, and QKV
    # defusion — the same transformations that the diffusers source performs.
    remapped = _remap_z_image_keys(checkpoint)

    # Cast every tensor to bf16 before loading. assign=True bypasses
    # dtype coercion (it adopts the loaded tensor's dtype as-is rather
    # than copying into the model's existing dtype), so the cast must
    # happen here, not after load_state_dict().
    remapped_bf16 = {k: v.to(torch.bfloat16) for k, v in remapped.items()}

    # Load the remapped, bf16-cast state dict into the model.
    model.load_state_dict(remapped_bf16, assign=True)

    return model


def load_vae(model_id: str) -> Any:
    """Load a VAE from a raw ``.safetensors`` file.

    Constructs an ``AutoencoderKL`` with configuration inferred directly
    from the raw checkpoint's tensor shapes — no hardcoded
    ``block_out_channels`` list and no ``config.json`` required.
    Keys are remapped from the raw LDM format to the diffusers convention
    using a local key-remap function, eliminating the dependency on
    ``diffusers.loaders.single_file_utils``.

    The ``scaling_factor`` is hardcoded to ``0.18215`` (the SD1.x default).
    This value is unconfirmable from checkpoint shapes alone. An incorrect
    value here produces visible brightness/contrast issues in decoded
    images, not a crash — the latent formula is
    ``(latents / scaling_factor) + shift_factor``.

    This function performs **zero network calls**. It never queries
    HuggingFace for a ``config.json`` or any other remote resource —
    all architecture parameters are derived from the checkpoint file,
    and the file is read locally.

    In mock mode (``ANVILML_WORKER_MOCK=1``), returns ``None``
    immediately without importing torch, diffusers, or safetensors.

    Args:
        model_id: Path to a ``.safetensors`` file containing raw VAE
            weights (the format produced by
            ``AutoencoderKL.state_dict()``).

    Returns:
        An ``AutoencoderKL`` instance with weights loaded and remapped,
        or ``None`` in mock mode.

    Raises:
        OSError: If the file at ``model_id`` does not exist or is
            inaccessible.
        ValueError: If the checkpoint is malformed and cannot be remapped
            or loaded into the model's state dict.
    """
    # Check mock mode by inspecting the environment variable.
    # This must be a runtime check (not a module-level import)
    # so that CI tests running with ANVILML_WORKER_MOCK=1
    # never touch torch/diffusers/safetensors at import time.
    _mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

    if _mock:
        # In mock mode, return None immediately without importing
        # torch, diffusers, or safetensors. This keeps tests fast
        # and avoids requiring GPU hardware or these heavy deps.
        return None

    # Real mode: lazy-import all heavy dependencies inside the
    # real-mode branch to preserve mock-mode import isolation.
    from diffusers import AutoencoderKL
    from safetensors.torch import load_file as safetensors_load_file

    # Load the raw checkpoint from the .safetensors file.
    # The raw format contains LDM-style keys (e.g. vae.encoder.down.0...)
    # that need remapping to the diffusers convention.
    checkpoint = safetensors_load_file(model_id)

    # Infer the AutoencoderKL config directly from checkpoint tensor shapes.
    # This replaces the previous approach of hardcoding block_out_channels
    # and the number of stages — now we derive the actual config from the
    # checkpoint, making the loader robust to VAE variants with different
    # stage counts or channel configurations.
    config = _infer_vae_config_from_checkpoint(checkpoint)

    # Construct the VAE model with explicitly-inferred parameters.
    # All other parameters use AutoencoderKL's registered defaults, which
    # are compatible with the checkpoint format produced by the original
    # model. The inferred block_out_channels determines the number of
    # down/up blocks, matching the checkpoint's structure.
    model = AutoencoderKL(
        block_out_channels=config["block_out_channels"],
        latent_channels=config["latent_channels"],
        in_channels=config["in_channels"],
        out_channels=config["out_channels"],
        layers_per_block=config["layers_per_block"],
    )

    # Remap keys from the raw LDM checkpoint format to the diffusers
    # convention. This strips LDM prefixes and maps encoder down blocks,
    # mid block, and decoder up blocks to the AutoencoderKL state dict layout.
    remapped = _remap_ldm_vae_keys(checkpoint)

    # Load the remapped state dict into the model. This applies
    # the weights to the constructed model instance.
    model.load_state_dict(remapped)

    # Set the spatial compression factor (8x: 1024x1024 → 128x128 latent grid).
    # This is the spatial factor used by ZImagePipeline, not the scaling_factor
    # used in the latent formula (latents / scaling_factor) + shift_factor.
    model.vae_scale_factor = VAE_SCALE_FACTOR

    return model


def sample(
    model: Any,
    conditioning: Any,
    clip: Any = None,
    latent: Any = None,
    steps: int = 4,
    cfg: float = 7.0,
    seed: int = 42,
    device: str = "cpu",
    cancel_flag: Any = None,
    emit_progress: Callable[[int, int], None] | None = None,
    *,
    pipeline_cache: Any = None,
) -> tuple[Any, int]:
    """Run ZiT sampling: mock path returns sentinel, real path invokes pipeline.

    This is the entry point for ZiT-specific diffusion sampling. In
    mock mode, it returns immediately with a ``MockLatent`` sentinel
    and the resolved seed. In real mode, it assembles a
    ``ZImagePipeline`` from cached components via the pipeline cache,
    invokes the pipeline with ``output_type="latent"``, and returns
    the denoised latent tensor.

    The ``torch``, ``diffusers``, and ``safetensors`` packages are
    imported lazily (inside the real-mode branch) to preserve mock-mode
    import isolation.

    Args:
        model: The model descriptor object (carries ``arch``,
            ``model_id``, and other metadata).
        conditioning: Conditioning tensor or prompt encoding.
        clip: The CLIP object (``RealClip``) carrying ``tokenizer``
            and ``text_encoder`` attributes. Produced by
            ``LoadClip``. ``None`` in mock-mode tests.
        latent: The initial latent tensor or ``MockLatent`` sentinel.
        steps: Number of denoising iterations.
        cfg: Classifier-free guidance scale.
        seed: Random seed for reproducibility.
        device: Device string (e.g. ``"cuda"`` or ``"cpu"``).
        cancel_flag: A ``threading.Event`` that is set when the job
            is cancelled. The sampling callback checks ``.is_set()``
            on each step.
        emit_progress: Callback invoked as ``emit_progress(step, total)``
            after each denoising step for progress reporting.
        pipeline_cache: Pipeline cache instance for loading cached
            components. Passed by the calling node's context;
            keyword-only argument for future extensibility.

    Returns:
        A tuple of ``(latent_output, seed)`` where ``latent_output``
        is either a ``MockLatent`` sentinel (mock mode) or a denoised
        latent tensor (real mode), and ``seed`` is the resolved seed
        value.

    Raises:
        _SamplingCancelled: When the ``cancel_flag`` is set during
            sampling — the callback detects cancellation and raises
            this sentinel exception, which propagates to the caller.
    """
    # Check mock mode by inspecting the environment variable.
    # This must be a runtime check (not a module-level import)
    # so that CI tests running with ANVILML_WORKER_MOCK=1
    # never touch torch/diffusers/safetensors at import time.
    _mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

    if _mock:
        # In mock mode, return a lightweight sentinel object
        # immediately without importing torch, diffusers, or
        # safetensors. This keeps tests fast and avoids requiring
        # GPU hardware or these heavy dependencies.
        return (MockLatent(), seed)

    # Real mode: assemble ZImagePipeline from cached components.
    # Imports are lazy (inside the real-mode branch) to preserve
    # mock-mode import isolation.
    from diffusers import FlowMatchEulerDiscreteScheduler
    from diffusers import ZImagePipeline

    # Extract model_id for the cache key. Real models (P18-D4)
    # carry a model_id attribute; for objects that don't, fall
    # back to str(model) so the cache key is still unique.
    model_id = getattr(model, "model_id", str(model))

    # Construct the loader_fn closure that builds a ZImagePipeline
    # from the model, conditioning, and vae arguments. This closure
    # is passed to pipeline_cache.get_or_load() which will call it
    # only when the component is not already cached.
    def loader_fn():
        # Pull the transformer from model — RealModel (P18-D4)
        # carries _transformer; if absent the model object itself
        # is the transformer.
        transformer = getattr(model, "_transformer", None)
        if transformer is None:
            transformer = model

        # Pull tokenizer and text_encoder from clip (RealClip).
        # The clip object is produced by LoadClip and carries these
        # attributes. Guard against None for mock-mode tests that
        # call sample() without a clip argument.
        tokenizer = getattr(clip, "tokenizer", None) if clip else None
        text_encoder = getattr(clip, "text_encoder", None) if clip else None

        # The scheduler is constructed fresh each time since it is
        # deterministic and lightweight.
        scheduler = FlowMatchEulerDiscreteScheduler()
        return ZImagePipeline(
            scheduler=scheduler,
            text_encoder=text_encoder,
            tokenizer=tokenizer,
            transformer=transformer,
        )

    # Load or cache the assembled pipeline. Uses "fp8" dtype to
    # match the convention used by LoadModel (which calls
    # get_or_load(model_id, "fp8", ...)). The dtype hint tells
    # the cache which cached variant to return.
    pipeline = pipeline_cache.get_or_load(
        f"{model_id}:pipeline",
        "fp8",
        loader_fn,
    )

    # Invoke the assembled pipeline with output_type="latent" so it
    # returns the raw denoised latent tensor (not a decoded image).
    # return_dict=False means diffusers returns a list; index 0 is
    # the latent tensor. Wrap in try/except to propagate cancellation.
    try:
        result = pipeline(
            prompt_embeds=conditioning.positive,
            negative_prompt_embeds=conditioning.negative,
            latents=latent,
            num_inference_steps=steps,
            guidance_scale=cfg,
            output_type="latent",
            callback_on_step_end=_make_callback(
                emit_progress, cancel_flag, steps
            ),
            return_dict=False,
        )
    except _SamplingCancelled:
        # Re-raise the cancellation sentinel so it propagates to
        # worker_main.py's exception handler. The CancelJob handler
        # already sends a Cancelled event; this exception propagates
        # as a Failed event with the cancellation reason in the error.
        raise

    return (result[0], seed)
