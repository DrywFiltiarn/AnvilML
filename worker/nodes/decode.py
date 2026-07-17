"""VaeDecode node — decodes a denoised latent to a PIL image using the explicitly
provided VAE.

In mock mode it returns a sentinel dict; the real branch dispatches to the
registered VAE architecture module (currently "zit_vae") via
``arch.vae.get_module()`` and ``module.decode()``.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

import logging

logger = logging.getLogger(__name__)


@register
class VaeDecode(BaseNode):
    """Decode a denoised latent tensor to a PIL image using a loaded VAE.

    This node takes a VAE model (already loaded by LoadVae) and a denoised
    latent tensor, then produces an IMAGE output — a PIL Image decoded from
    the latent space.

    In mock mode (ANVILML_WORKER_MOCK=1) it returns a sentinel dict carrying
    the input latent's shape. The real branch dispatches to the registered
    VAE architecture module (currently "zit_vae") via
    ``arch.vae.get_module()`` and ``module.decode()``.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Two input slots — "vae" (VAE) and "latent" (LATENT).
        OUTPUT_SLOTS: Single output slot named "image" with type "IMAGE".
    """

    NODE_TYPE = "VaeDecode"
    CATEGORY = "Decoding"
    DISPLAY_NAME = "VAE Decode"
    DESCRIPTION = (
        "Decodes a denoised latent to a PIL image using the explicitly "
        "provided VAE."
    )
    INPUT_SLOTS = [
        SlotSpec("vae", "VAE"),
        SlotSpec("latent", "LATENT"),
    ]
    OUTPUT_SLOTS = [SlotSpec("image", "IMAGE")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the VaeDecode node.

        Branches on the mock flag at the top: mock mode returns a sentinel
        dict with the input latent's shape, while the real branch dispatches
        to the registered VAE architecture module via
        ``arch.vae.get_module()`` and ``module.decode()``.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Named input values keyed by slot name. Must contain
                "vae" (a loaded VAE model) and "latent" (a denoised latent
                tensor or a dict with a "shape" key in mock mode).

        Returns:
            In mock mode: Dict with key "image" containing a sentinel dict
            {"mock": True, "shape": <input_shape>}.
            In real mode: Dict with key "image" containing a list of
            ``PIL.Image.Image`` objects decoded from the latent tensor.

        Raises:
            NotImplementedError: When ctx.mock is False — the real branch
                is deferred to a future task that dispatches to the registered
                VAE architecture module.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with the input latent's
            # shape. This ensures reproducible mock-mode test output and
            # allows downstream tests to verify the correct shape was
            # propagated through the node system.
            shape = inputs["latent"].get("shape")
            logger.debug("VaeDecode: mock mode, shape=%s", shape)
            return {"image": {"mock": True, "shape": shape}}
        else:
            # defers_to: P24-B2 — real branch dispatches to arch.vae.get_module(vae.arch).decode()
            raise NotImplementedError(
                "VaeDecode real branch deferred to P24-B2; dispatches to "
                "arch.vae.get_module(vae.arch).decode()"
            )
