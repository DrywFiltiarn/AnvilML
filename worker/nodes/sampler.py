"""EmptyLatent and Sampler nodes — latent creation and denoising sampling.

This module defines the ``EmptyLatent`` node, which creates a blank noise
latent tensor at the requested resolution, and the ``Sampler`` node, which
runs the denoising sampling loop.

In mock mode (``ANVILML_WORKER_MOCK=1``), both nodes return lightweight
sentinel objects (``MockLatent``) instead of real tensor computations.
The ``Sampler`` node sets ``EMITS_PROGRESS = True`` so the executor's
progress-emission path activates during graph execution.

The ``torch``, ``diffusers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Instead, any real-mode sampling code must import these packages lazily
inside the non-mock code path, which is unreachable when
``ANVILML_WORKER_MOCK=1``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
import random
from typing import Any

from worker.nodes import arch
from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["EmptyLatent", "Sampler", "MockLatent"]


class MockLatent:
    """Sentinel latent object for mock mode.

    A lightweight placeholder that carries the spatial dimensions
    (width, height, batch_size) of a real latent tensor so that
    downstream nodes (Sampler, VAEDecode, etc.) can inspect the
    resolution without needing a real torch.Tensor.

    In real mode, the actual latent tensor produced by the VAE
    encoder or EmptyLatent node will have its own structure defined
    when the real tensor paths are implemented (future phase).

    Args:
        width: The width dimension of the latent tensor.
        height: The height dimension of the latent tensor.
        batch_size: The batch dimension of the latent tensor.
            Defaults to 1.
    """

    def __init__(self, width: int, height: int, batch_size: int = 1) -> None:
        """Initialise a mock latent sentinel.

        Args:
            width: The width dimension of the latent tensor.
            height: The height dimension of the latent tensor.
            batch_size: The batch dimension. Defaults to 1.
        """
        self.width = width
        self.height = height
        self.batch_size = batch_size


@register
class EmptyLatent(BaseNode):
    """Create a blank noise latent tensor at the requested resolution.

    Accepts width, height, and an optional batch_size input, then
    returns a ``LATENT`` slot containing either a ``MockLatent``
    sentinel (in mock mode) or a real noise tensor (in real mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: Four slots — ``width`` (INT, required),
            ``height`` (INT, required), ``batch_size``
            (INT, optional, defaults to 1), and ``model``
            (MODEL, optional, used in real mode for architecture dispatch).
        OUTPUT_SLOTS: One ``LATENT`` slot named ``latent``.
    """

    NODE_TYPE = "EmptyLatent"
    CATEGORY = "Latents"
    DISPLAY_NAME = "Empty Latent"
    DESCRIPTION = "Create a blank noise latent tensor at the requested resolution"
    INPUT_SLOTS = [
        SlotSpec("width", "INT"),
        SlotSpec("height", "INT"),
        SlotSpec("batch_size", "INT", optional=True),
        SlotSpec("model", "MODEL", optional=True),
    ]
    OUTPUT_SLOTS = [SlotSpec("latent", "LATENT")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the EmptyLatent node.

        Reads the ``width``, ``height``, and optional ``batch_size``
        inputs, checks mock mode, and either returns a ``MockLatent``
        sentinel or creates a real noise tensor.

        Args:
            **inputs: Must contain ``"width"`` and ``"height"``.
                May contain an optional ``"batch_size"`` (defaults
                to 1).

        Returns:
            Dict with key ``"latent"`` containing either a
            ``MockLatent`` (mock mode) or a real noise tensor
            (real mode).

        Raises:
            ValueError: If called in non-mock mode without a ``model``
                input — the real noise tensor path requires architecture
                dispatch to compute the latent shape.
        """
        # Read the width and height inputs from the job graph.
        # These define the spatial resolution of the latent tensor.
        width = inputs.get("width")
        height = inputs.get("height")

        # Read the optional batch_size. Defaults to 1 when not
        # provided — this is the standard ComfyUI convention for
        # single-image generation.
        batch_size = inputs.get("batch_size", 1)

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # carrying the requested dimensions instead of creating
            # a real noise tensor. This keeps tests fast and avoids
            # requiring GPU hardware or torch.
            return {"latent": MockLatent(width, height, batch_size)}

        # Real mode: dispatch to the architecture module to compute
        # the latent shape, then create a noise tensor via torch.randn.
        model = inputs.get("model")

        # The real path requires a model descriptor to identify the
        # architecture (e.g. "zit" for DiffusionTransformer). Without
        # it we cannot determine the correct latent packing scheme.
        if model is None:
            raise ValueError(
                "EmptyLatent real path requires a model input"
            )

        # Look up the architecture module that handles this model type.
        # get_module() scans loaded arch modules and returns the first
        # one whose can_handle() returns True for the model object.
        mod = arch.get_module(model)
        if mod is None:
            raise ValueError(
                f"EmptyLatent: unsupported model architecture for {model}"
            )

        # Read the latent channel count from the model descriptor.
        # This attribute is set by LoadModel's real path (P18-D4/P18-D13)
        # and represents the number of channels in the latent space.
        num_channels_latents = model.in_channels

        # Delegate shape computation to the architecture module.
        # Different architectures use structurally different packing
        # schemes — e.g. Flux 2 Klein uses a fundamentally different
        # tensor layout than standard DiT models. The arch module
        # knows the correct formula for each architecture.
        shape = mod.compute_latent_shape(
            batch_size, height, width, num_channels_latents
        )

        # Import torch lazily inside the real-mode branch only.
        # Top-level imports would cause the worker to fail on systems
        # without torch installed (no GPU hardware).
        import torch

        return {"latent": torch.randn(
            shape, dtype=torch.float32, device=self.ctx.device
        )}


@register
class Sampler(BaseNode):
    """Run the denoising sampling loop.

    Accepts a model, conditioning, latent tensor, and sampling
    parameters (steps, cfg, seed), then returns a ``LATENT`` slot
    containing the denoised result and the resolved seed value.

    This node sets ``EMITS_PROGRESS = True`` so the executor's
    progress-emission path activates during graph execution,
    sending Progress events back to the Rust supervisor.

    In mock mode, the node returns a ``MockLatent`` sentinel
    carrying the input latent's dimensions and the resolved seed.

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: Six slots — ``model`` (MODEL, required),
            ``conditioning`` (CONDITIONING, required), ``latent``
            (LATENT, required), ``steps`` (INT, required),
            ``cfg`` (FLOAT, required), and ``seed`` (INT, required).
        OUTPUT_SLOTS: Two slots — ``latent`` (LATENT) and ``seed``
            (INT, the resolved seed value).
        EMITS_PROGRESS: Set to True so the executor emits Progress
            events during node execution.
    """

    NODE_TYPE = "Sampler"
    CATEGORY = "Sampling"
    DISPLAY_NAME = "Sampler"
    DESCRIPTION = "Run the denoising sampling loop"
    INPUT_SLOTS = [
        SlotSpec("model", "MODEL"),
        SlotSpec("conditioning", "CONDITIONING"),
        SlotSpec("latent", "LATENT"),
        SlotSpec("steps", "INT"),
        SlotSpec("cfg", "FLOAT"),
        SlotSpec("seed", "INT"),
    ]
    OUTPUT_SLOTS = [
        SlotSpec("latent", "LATENT"),
        SlotSpec("seed", "INT"),
    ]
    EMITS_PROGRESS = True

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the Sampler node.

        Reads all six input slots, resolves the seed value (seed=-1
        means random), checks mock mode, and either returns a
        ``MockLatent`` sentinel with the resolved seed or runs the
        real denoising loop.

        Args:
            **inputs: Must contain ``"model"``, ``"conditioning"``,
                ``"latent"``, ``"steps"``, ``"cfg"``, and ``"seed"``.

        Returns:
            Dict with keys ``"latent"`` (a ``MockLatent`` in mock
            mode or a denoised tensor in real mode) and ``"seed"``
            (the resolved seed value, always a positive integer).

        Raises:
            ValueError: If called in non-mock mode and the model's
                architecture is not supported by any loaded arch module.
        """
        # Read all six input slots from the job graph.
        # These are the standard ComfyUI sampler parameters:
        # model (the diffusion model), conditioning (positive/negative
        # prompts), latent (the initial noise or intermediate latent),
        # steps (number of denoising iterations), cfg (classifier-free
        # guidance scale), and seed (random seed for reproducibility).
        model = inputs.get("model")
        conditioning = inputs.get("conditioning")
        latent = inputs.get("latent")
        steps = inputs.get("steps")
        cfg = inputs.get("cfg")
        seed = inputs.get("seed")

        # Resolve the seed value. In ComfyUI, seed=-1 means "pick a
        # random seed" — this is the standard convention. We use
        # random.randrange(0, 2**32) to match ComfyUI's exact range
        # [0, 2**32-1], which allows seed 0 to be a valid output.
        if seed == -1:
            seed = random.randrange(0, 2**32)

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # carrying the input latent's dimensions instead of
            # running a real denoising loop. The executor (not this
            # node) handles progress event emission when
            # EMITS_PROGRESS is True.
            # The latent's width, height, and batch_size are
            # preserved from the input — real sampling doesn't
            # change these dimensions.
            return {
                "latent": MockLatent(
                    latent.width,
                    latent.height,
                    latent.batch_size,
                ),
                "seed": seed,
            }

        # Real mode: dispatch to the architecture module for the
        # denoising sampling loop. Build the emit_progress wrapper
        # before dispatch so it is available to the arch module.
        def emit_progress(step: int, total: int) -> None:
            """Emit a Progress event for the executor's progress tracking.

            Args:
                step: The current denoising step (0-indexed).
                total: Total number of denoising steps.
            """
            self.ctx.emit({
                "_type": "Progress",
                "job_id": self.ctx.job_id,
                "step": step,
                "total_steps": total,
                "preview_b64": None,
            })

        # Look up the architecture module that handles this model type.
        # get_module() scans loaded arch modules and returns the first
        # one whose can_handle() returns True for the model object.
        # If no module claims this architecture, raise ValueError per
        # the dispatch contract (same pattern as EmptyLatent line 158).
        mod = arch.get_module(model)
        if mod is None:
            raise ValueError("unsupported model architecture")

        # Invoke the architecture module's sample() function with all
        # required arguments. The module resolves the pipeline from
        # cache, assembles it, and runs the denoising loop.
        result = mod.sample(
            model, conditioning, latent, steps, cfg, seed,
            self.ctx.device, self.ctx.cancel_flag, emit_progress,
            pipeline_cache=self.ctx.pipeline_cache,
        )

        # Extract the denoised latent tensor (index 0) and the
        # resolved seed value (index 1) from the sample() return tuple.
        return {"latent": result[0], "seed": result[1]}
