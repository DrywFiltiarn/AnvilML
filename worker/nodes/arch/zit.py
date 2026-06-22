"""Z-Image Turbo (ZiT) FP8 architecture dispatch module.

This module provides architecture-specific dispatch for ZiT models,
including model detection via ``can_handle()`` and a mockable sampling
entry point via ``sample()``.

In mock mode (``ANVILML_WORKER_MOCK=1``), the ``sample()`` function
returns a lightweight ``MockLatent`` sentinel immediately without
importing torch, diffusers, or safetensors. The real sampling path
is stubbed with ``NotImplementedError`` until the full pipeline
integration is implemented in a future phase.

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

__all__ = ["can_handle", "sample", "MockLatent", "VAE_SCALE_FACTOR"]

# Z-Image-Turbo's published VAE config has block_out_channels=[128,256,512,512]
# (4 entries), giving 2**(4-1)=8 per ZImagePipeline.__init__'s vae_scale_factor
# formula; independently corroborated as 8x spatial compression
# (1024x1024 image -> 128x128 latent grid).
VAE_SCALE_FACTOR: int = 8


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
) -> tuple[Any, int]:
    """Run ZiT sampling: mock path returns sentinel, real path is stubbed.

    This is the entry point for ZiT-specific diffusion sampling. In
    mock mode, it returns immediately with a ``MockLatent`` sentinel
    and the resolved seed. In real mode, it is stubbed with
    ``NotImplementedError`` — the full implementation will assemble
    a ``ZImagePipeline`` from cached components via the pipeline
    cache, run the denoising loop with cancel and progress hooks,
    and keep the transformer at ``float8`` dtype when ``InferenceCaps.fp8``
    is enabled.

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

    Returns:
        A tuple of ``(latent_output, seed)`` where ``latent_output``
        is either a ``MockLatent`` sentinel (mock mode) or a denoised
        latent tensor (real mode, not yet implemented), and ``seed``
        is the resolved seed value.

    Raises:
        NotImplementedError: If called in non-mock mode. The real
            ZiT sampling path is stubbed until the full pipeline
            integration is implemented in a future phase.
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

    # Real mode: ZiT sampling path not yet implemented.
    # The full implementation will:
    #   1. Import torch, diffusers, and safetensors here (lazy imports).
    #   2. Check cancel_flag.is_set() at every step via callback_on_step_end.
    #   3. Call emit_progress(step, total_steps) per step.
    #   4. Assemble ZImagePipeline from cached components via
    #      pipeline_cache.get_or_load(f"{model_id}:pipeline", ...).
    #   5. Keep transformer at float8 dtype (no upcast) when
    #      InferenceCaps.fp8=True; text_encoder/vae stay bf16.
    # TODO(P18-D1): Implement real ZiT sampling path.
    raise NotImplementedError(
        "Real ZiT sampling path not yet implemented — "
        "use ANVILML_WORKER_MOCK=1 for testing"
    )
