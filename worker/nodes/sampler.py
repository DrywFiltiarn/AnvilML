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
        INPUT_SLOTS: Three slots — ``width`` (INT, required),
            ``height`` (INT, required), and ``batch_size``
            (INT, optional, defaults to 1).
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
            NotImplementedError: If called in non-mock mode. The real
                noise tensor creation path is stubbed until the real
                sampling implementation is added in a future phase.
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

        # Real mode: create actual noise tensor via torch.randn.
        # This path is stubbed — the real implementation will use
        # torch.randn((batch_size, channels, height, width)) to
        # create the initial noise latent for the diffusion process.
        # TODO(P18-B2): Implement real EmptyLatent path.
        raise NotImplementedError(
            "Real EmptyLatent path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )


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
            NotImplementedError: If called in non-mock mode. The real
                denoising loop using ``arch.sample()`` is stubbed until
                P18-C1 implements the architecture module.
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

        # Real mode: run the denoising sampling loop.
        # This path is stubbed — the real implementation will call
        # arch.sample(model, conditioning, latent, steps, cfg, seed)
        # to execute the actual diffusion process. The arch module
        # is implemented in task P18-C1.
        # TODO(P18-C1): Implement real sampling path via arch.sample().
        raise NotImplementedError(
            "Real Sampler path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )
