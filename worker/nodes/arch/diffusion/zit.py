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


def load_transformer(model_id: str) -> Any:
    """Load a Z-Image Turbo (ZiT) transformer from a raw ``.safetensors`` file.

    Constructs a ``ZImageTransformer2DModel`` with zero arguments — the class's
    registered defaults (``dim=3840``, ``n_layers=30``, ``n_heads=30``,
    ``cap_feat_dim=2560``) match the published 6B ZiT architecture config.
    Weights are loaded from the provided ``.safetensors`` file via
    ``safetensors.torch.load_file``, and keys are remapped to the diffusers
    convention by reusing ``diffusers.loaders.single_file_utils``'s internal
    conversion function (the same logic that ``FromOriginalModelMixin`` uses
    internally, but applied manually to avoid any HuggingFace network access).

    This function performs **zero network calls**. It never queries HuggingFace
    for a ``config.json`` or any other remote resource — all architecture
    parameters are hard-coded defaults, and the checkpoint file is read locally.

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
    from diffusers import ZImageTransformer2DModel
    from diffusers.loaders.single_file_utils import (
        convert_z_image_transformer_checkpoint_to_diffusers,
    )
    from safetensors.torch import load_file as safetensors_load_file

    # Construct the model with zero arguments — the class's
    # registered defaults match the published 6B ZiT architecture
    # config (dim=3840, n_layers=30, n_heads=30, cap_feat_dim=2560).
    # We rely on these defaults rather than reading a config file,
    # which would require network access to HuggingFace.
    model = ZImageTransformer2DModel()

    # Load the raw checkpoint from the .safetensors file.
    # The raw format contains fused QKV keys (qkv.weight) and
    # the model.diffusion_model. prefix — diffusers' remap
    # function handles both transformations.
    checkpoint = safetensors_load_file(model_id)

    # Remap keys from the raw checkpoint format to the diffusers
    # convention. This function fuses separate to_q/to_k/to_v
    # weights into qkv.weight and removes the model.diffusion_model.
    # prefix — the exact inverse of what ZImageTransformer2DModel
    # produces in state_dict().
    remapped = convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)

    # Load the remapped state dict into the model. This applies
    # the weights to the zero-arg-constructed model instance.
    model.load_state_dict(remapped)

    return model


def load_vae(model_id: str) -> Any:
    """Load a VAE from a raw ``.safetensors`` file.

    Constructs an ``AutoencoderKL`` with the published Z-Image Turbo
    VAE config (``block_out_channels=[128, 256, 512, 512]``) using
    zero-argument defaults. Weights are loaded from the provided
    ``.safetensors`` file via ``safetensors.torch.load_file``, and
    keys are remapped to the diffusers convention by reusing
    ``diffusers.loaders.single_file_utils``'s internal conversion
    function.

    This function performs **zero network calls**. It never queries
    HuggingFace for a ``config.json`` or any other remote resource —
    all architecture parameters are hard-coded defaults, and the
    checkpoint file is read locally.

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
    from diffusers.loaders.single_file_utils import convert_ldm_vae_checkpoint
    from safetensors.torch import load_file as safetensors_load_file

    # Construct the VAE model with block_out_channels set to match
    # the published Z-Image Turbo VAE config (4 entries). All other
    # parameters use AutoencoderKL's registered defaults, which are
    # compatible with the checkpoint format produced by the original
    # model. The 4-entry block_out_channels determines the number of
    # down/up blocks, matching the checkpoint's structure.
    model = AutoencoderKL(block_out_channels=[128, 256, 512, 512])

    # Load the raw checkpoint from the .safetensors file.
    # The raw format contains LDM-style keys (e.g. vae.encoder.down.0...)
    # that need remapping to the diffusers convention.
    checkpoint = safetensors_load_file(model_id)

    # Build the config dict that convert_ldm_vae_checkpoint expects.
    # This function only reads the *length* of down_block_types and
    # up_block_types (to determine the number of encoder/decoder
    # blocks), not their exact content. The standard SD-style block
    # type strings are sufficient.
    config = {
        "down_block_types": ["DownEncoderBlock2D"] * 4,
        "up_block_types": ["UpDecoderBlock2D"] * 4,
    }

    # Remap keys from the raw LDM checkpoint format to the diffusers
    # convention. This strips the LDM key prefix and maps encoder
    # down blocks, mid block, and decoder up blocks to the
    # AutoencoderKL state dict layout.
    remapped = convert_ldm_vae_checkpoint(checkpoint, config)

    # Load the remapped state dict into the model. This applies
    # the weights to the zero-arg-constructed model instance.
    model.load_state_dict(remapped)

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
