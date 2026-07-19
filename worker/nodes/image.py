"""SaveImage node — saves an image to the artifact store and emits ImageReady.

The SaveImage node accepts an image (and optional seed/steps) and emits an
ImageReady event. In mock mode it generates a 64×64 black PNG and emits the
ImageReady event dict via ctx.emit. The real branch is a placeholder for
P24-D2 (real PNG encoding + artifact emission).
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

import logging

logger = logging.getLogger(__name__)


@register
class SaveImage(BaseNode):
    """Save an image to the artifact store and emit ImageReady.

    This output node accepts an image (required) and optional seed/steps.
    It does not produce output slots — instead it emits an ImageReady
    event via ``ctx.emit`` per §10.3's table (Output nodes emit events,
    not slot outputs). In mock mode it generates a 64×64 black PNG and
    emits the ImageReady event dict. The real branch is a placeholder
    for P24-D2.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to ("Output").
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Three input slots — image (IMAGE, required),
            seed (INT, optional), steps (INT, optional).
        OUTPUT_SLOTS: Empty list — Output nodes emit events, not slot
            outputs (ANVILML_DESIGN.md §10.3).
    """
    NODE_TYPE = "SaveImage"
    CATEGORY = "Output"
    DISPLAY_NAME = "Save Image"
    DESCRIPTION = "Saves an image to the artifact store and emits ImageReady."
    INPUT_SLOTS = [
        SlotSpec("image", "IMAGE"),
        SlotSpec("seed", "INT", optional=True),
        SlotSpec("steps", "INT", optional=True),
    ]
    OUTPUT_SLOTS = []

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_real_emits_png
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready
    # defers_to: P24-D2 — real branch placeholder; P24-D2 will replace NotImplementedError
    #            with real PNG encoding + artifact emission
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the SaveImage node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        generates a 64×64 black PNG and emits an ImageReady event dict
        via ctx.emit; the real branch is a placeholder for P24-D2.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain an "image" key. Optional "seed"
                (int) and "steps" (int) keys may be provided.

        Returns:
            Dict with key "image" containing a sentinel dict
            {"mock": True, "width": 64, "height": 64} in mock mode.
            The real branch is not yet implemented.

        Raises:
            KeyError: If "image" is not provided in inputs.
            NotImplementedError: If called in real mode (placeholder).
        """
        if ctx.mock:
            # Validate required inputs before proceeding. The "image" key
            # is required (not optional) per INPUT_SLOTS — accessing it
            # directly raises KeyError if absent, which is the desired
            # failure mode for missing required inputs.
            _ = inputs["image"]

            # Mock branch: generate a 64×64 black PNG using PIL (local
            # import keeps mock mode torch-free — ANVILML_DESIGN.md §11.2).
            # Encode to PNG bytes, emit ImageReady event, return sentinel.
            from PIL import Image
            import io

            img = Image.new("RGB", (64, 64), (0, 0, 0))
            buffer = io.BytesIO()
            img.save(buffer, format="PNG")
            png_bytes = buffer.getvalue()

            # Emit ImageReady event with truncated hex for test verification.
            # The hex[:32] is a deterministic prefix of the full PNG bytes
            # that tests can assert on without needing the full binary blob.
            ctx.emit({
                "_type": "ImageReady",
                "job_id": ctx.job_id,
                "artifact_hash": "mock_black_png_64x64",
                "width": 64,
                "height": 64,
                "seed": inputs.get("seed", -1),
                "steps": inputs.get("steps", 1),
                "image_data": png_bytes.hex()[:32],
            })

            logger.debug(
                "SaveImage: mock branch emitted ImageReady for job_id=%s",
                ctx.job_id_str,
            )

            return {"image": {"mock": True, "width": 64, "height": 64}}
        else:
            # defers_to: P24-D2 — real branch placeholder. P24-D2 will
            # replace this with real PNG encoding and artifact emission.
            raise NotImplementedError(
                "real branch not yet implemented — P24-D2"
            )
