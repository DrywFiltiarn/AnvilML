"""VaeDecode node — decodes a denoised latent to a PIL image using the explicitly
provided VAE.

In mock mode it returns a sentinel dict; the real branch dispatches to the
registered VAE architecture module (currently "zit_vae") via
``arch.vae.get_module()`` and ``module.decode()``.
"""

from worker.nodes import arch
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
            ValueError: When 'vae' or 'latent' inputs are missing or None, or when
                the vae input lacks an .arch attribute.
            RuntimeError: When no registered VAE module handles the vae's arch key,
                or when torch is not installed in the decode() call.
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
            # Real branch: dispatch to the registered VAE architecture module.
            # The vae input is the fully-loaded module from LoadVae.execute();
            # it carries an .arch attribute set by the arch module's load()
            # function (Phase 23).
            vae = inputs.get("vae")
            latent = inputs.get("latent")

            if vae is None:
                raise ValueError(
                    "VaeDecode: 'vae' input is required (missing or None)"
                )
            if latent is None:
                raise ValueError(
                    "VaeDecode: 'latent' input is required (missing or None)"
                )

            # Get the architecture key from the loaded VAE module.
            # Using getattr with a default is safer than direct attribute
            # access in case a test passes a dict-like object.
            arch_key = getattr(vae, "arch", None)
            if arch_key is None:
                raise ValueError(
                    f"VaeDecode: vae input has no .arch attribute "
                    f"(type={type(vae).__name__}); expected a loaded arch module"
                )

            # Dispatch to the registered VAE architecture module.
            # get_module returns None for unregistered keys — this is the
            # correct failure mode: if a new arch module is registered
            # without a corresponding node update, the error is explicit
            # rather than a silent crash.
            vae_module = arch.vae.get_module(arch_key)
            if vae_module is None:
                raise RuntimeError(
                    f"VaeDecode: no registered VAE module handles arch={arch_key!r}; "
                    f"check that the arch module is importable and can_handle() returns True"
                )

            # Call the architecture-specific decode function.
            # decode(vae_module, latent) returns list[PIL.Image.Image].
            images = vae_module.decode(vae, latent)

            logger.debug("VaeDecode: real mode, decoded %d image(s)", len(images))
            return {"image": images}
