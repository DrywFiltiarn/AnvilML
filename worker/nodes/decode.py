"""VaeDecode node — decodes a latent tensor to an image using a VAE.

This module defines the ``VaeDecode`` node, which accepts a ``VAE``
object and a ``LATENT`` tensor input, then returns an ``IMAGE`` slot.
In mock mode (``ANVILML_WORKER_MOCK=1``), it returns a lightweight
``MockImage`` sentinel instead of running a real VAE decode.

The ``torch``, ``diffusers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Instead, any real-mode decoding code must import these packages lazily
inside the non-mock code path, which is unreachable when
``ANVILML_WORKER_MOCK=1``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["VaeDecode", "MockImage"]


class MockImage:
    """Sentinel image object for mock mode.

    A lightweight placeholder that stands in for a real decoded image
    (typically a ``torch.Tensor`` or ``PIL.Image``) during testing.
    Real images produced by the VAE decoder will have their own
    structure defined when the real decode path is implemented
    (future phase).
    """
    pass


@register
class VaeDecode(BaseNode):
    """Decode a latent tensor to an image using a VAE.

    Accepts a ``VAE`` object and a ``LATENT`` tensor input, then
    returns an ``IMAGE`` slot containing either a real decoded image
    (in non-mock mode) or a ``MockImage`` sentinel (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: Two slots — ``vae`` (VAE, required) and
            ``latent`` (LATENT, required).
        OUTPUT_SLOTS: One ``IMAGE`` slot named ``image``.
    """

    NODE_TYPE = "VaeDecode"
    CATEGORY = "Decoding"
    DISPLAY_NAME = "VAE Decode"
    DESCRIPTION = "Decode a latent tensor to an image using a VAE"
    INPUT_SLOTS = [SlotSpec("vae", "VAE"), SlotSpec("latent", "LATENT")]
    OUTPUT_SLOTS = [SlotSpec("image", "IMAGE")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the VaeDecode node.

        Reads the ``vae`` and ``latent`` inputs, checks mock mode,
        and either returns a ``MockImage`` sentinel or decodes
        via the real VAE pipeline.

        Args:
            **inputs: Must contain ``"vae"`` (a VAE object) and
                ``"latent"`` (a latent tensor).

        Returns:
            Dict with key ``"image"`` containing either a
            ``MockImage`` (mock mode) or a decoded image tensor
            (real mode).

        Raises:
            NotImplementedError: If called in non-mock mode. The real
                VAE decode path is stubbed until the real decode
                implementation is added in a future phase.
        """
        # Read the vae and latent inputs from the job graph.
        # The vae object was produced by a prior LoadVae node;
        # the latent tensor came from EmptyLatent + Sampler.
        vae = inputs.get("vae")
        latent = inputs.get("latent")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # instead of decoding via a real VAE pipeline. This
            # keeps tests fast and avoids requiring GPU hardware
            # or torch. The latent dimensions (width, height) are
            # preserved from the input — real VAE decode doesn't
            # change these.
            return {"image": MockImage()}

        # Real mode: decode latent tensor using the loaded VAE.
        # This path is stubbed — the real implementation will call
        # vae.decode(latent) to produce the final image tensor.
        # The VAE decoder uses safetensors-loaded weights via the
        # pipeline_cache module.
        # TODO: Implement real VAE decode path.
        raise NotImplementedError(
            "Real VaeDecode path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )
