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
between them is provided by ``_make_callback`` in a downstream task).

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


def sample(
    model: Any,
    conditioning: Any,
    latent: Any,
    steps: int,
    cfg: float,
    seed: int,
    device: str,
    cancel_flag: Any,
    emit_progress: Callable[[int, int], None],
    vae: Any = None,
    *,
    pipeline_cache: Any = None,
) -> tuple[Any, int]:
    """Run ZiT sampling: mock path returns sentinel, real path assembles pipeline.

    This is the entry point for ZiT-specific diffusion sampling. In
    mock mode, it returns immediately with a ``MockLatent`` sentinel
    and the resolved seed. In real mode, it assembles a
    ``ZImagePipeline`` from cached components via the pipeline
    cache and stores it as a local variable; invocation is deferred
    to the downstream task (P18-D18c).

    The ``torch``, ``diffusers``, and ``safetensors`` packages are
    imported lazily (inside the real-mode branch) to preserve mock-mode
    import isolation.

    Args:
        model: The model descriptor object (carries ``arch``,
            ``model_id``, and other metadata).
        conditioning: Conditioning tensor or prompt encoding.
        latent: The initial latent tensor or ``MockLatent`` sentinel.
        steps: Number of denoising iterations.
        cfg: Classifier-free guidance scale.
        seed: Random seed for reproducibility.
        device: Device string (e.g. ``"cuda"`` or ``"cpu"``).
        cancel_flag: A mutable container (e.g. ``list[bool]``) that
            the sampling loop checks to detect cancellation requests.
        emit_progress: Callback invoked as ``emit_progress(step, total)``
            after each denoising step for progress reporting.
        vae: The VAE component used by the pipeline. Passed by the
            calling node; ``None`` in mock-mode tests.
        pipeline_cache: Pipeline cache instance for loading cached
            components. Passed by the calling node's context;
            keyword-only argument for future extensibility.

    Returns:
        A tuple of ``(latent_output, seed)`` where ``latent_output``
        is either a ``MockLatent`` sentinel (mock mode) or a denoised
        latent tensor (real mode, not yet invoked), and ``seed``
        is the resolved seed value.

    Raises:
        NotImplementedError: If called in non-mock mode without a
            ``pipeline_cache``. The real ZiT sampling path requires
            a pipeline cache to load cached transformer, VAE, text
            encoder, tokenizer, and scheduler components.
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

        # Pull tokenizer and text_encoder from conditioning.
        # Conditioning objects carry these from the encoder node
        # (P18-D16) that produced them.
        tokenizer = getattr(conditioning, "tokenizer", None)
        text_encoder = getattr(conditioning, "text_encoder", None)

        # VAE is passed directly to sample() as a separate argument.
        # The scheduler is constructed fresh each time since it is
        # deterministic and lightweight.
        scheduler = FlowMatchEulerDiscreteScheduler()
        return ZImagePipeline(
            scheduler=scheduler,
            vae=vae,
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

    # defers_to: P18-D18c -- pipeline assembled, not yet invoked
    raise NotImplementedError(
        "Real ZiT sampling path: pipeline assembled but not yet "
        "invoked — use ANVILML_WORKER_MOCK=1 for testing"
    )
