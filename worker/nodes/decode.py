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
    (a ``PIL.Image.Image`` produced by the VAE decoder) during testing.
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
            ``PIL.Image.Image`` (real mode) or a ``MockImage``
            sentinel (mock mode).
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
        # Inverse of the encode-time scaling: during encoding, latents
        # were scaled as z = z * scaling_factor + shift_factor
        # (conceptually); the decoder expects the original scale, so we
        # undo it here:
        #   latents = (latents / vae.config.scaling_factor) +
        #             vae.config.shift_factor
        # This reverses the normalization that compresses the latent
        # space to unit variance during VAE training (see Kingma &
        # Welling 2013, and the diffusers AutoencoderKL config default
        # scaling_factor=0.18215).
        latents = inputs.get("latent")

        # Lazy imports — torch/diffusers must never be imported at
        # module top level, or CI tests without GPU hardware will
        # fail on import.
        import torch
        from diffusers.image_processor import VaeImageProcessor

        # Apply the inverse-of-encode scaling.
        # The VAE was trained with latents normalised to unit variance
        # using scaling_factor (default 0.18215 for SD-style VAEs).
        # To decode, we undo this normalisation before passing to the
        # decoder.
        #
        # Guard: some VAE configs (older diffusers versions) do not
        # include a shift_factor attribute, or set it to None.  In
        # that case the additive shift is zero and the formula
        # simplifies to latents / scaling_factor.
        shift = vae.config.shift_factor if vae.config.shift_factor is not None else 0.0
        latents = (latents / vae.config.scaling_factor) + shift

        # Decode the latent to a raw image tensor.
        # return_dict=False returns a plain tuple; [0] extracts the
        # tensor. The tensor is in the VAE's output space (typically
        # [-1, 1] range).
        decoded = vae.decode(latents, return_dict=False)[0]

        # Postprocess the raw decoded tensor to a PIL Image.
        # VaeImageProcessor handles denormalization ([-1,1] -> [0,1]),
        # conversion to numpy, and conversion to PIL. The vae_scale_factor
        # of 16 matches ZImagePipeline's own image_processor construction
        # (self.vae_scale_factor=8 * 2 = 16 for ZiT's 4-block VAE).
        processor = VaeImageProcessor(vae_scale_factor=16)
        pil_images = processor.postprocess(decoded, output_type="pil")

        # Return the first (and typically only) PIL Image.
        return {"image": pil_images[0]}
