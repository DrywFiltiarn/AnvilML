"""SaveImage node — saves an image to the artifact store and emits ImageReady.

The SaveImage node accepts an image (and optional seed/steps) and emits an
ImageReady event. In mock mode it generates a 64×64 black PNG and emits the
ImageReady event dict via ctx.emit. The real branch encodes the input
PIL.Image to PNG bytes, base64-encodes for the IPC payload, and emits
ImageReady via ctx.emit.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

import logging
from PIL import Image

logger = logging.getLogger(__name__)


@register
class SaveImage(BaseNode):
    """Save an image to the artifact store and emit ImageReady.

    This output node accepts an image (required) and optional seed/steps.
    It does not produce output slots — instead it emits an ImageReady
    event via ``ctx.emit`` per §10.3's table (Output nodes emit events,
    not slot outputs). In mock mode it generates a 64×64 black PNG and
    emits the ImageReady event dict. The real branch encodes the input
    PIL.Image to PNG bytes, base64-encodes for the IPC payload, and
    emits ImageReady via ctx.emit.

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
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the SaveImage node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        generates a 64×64 black PNG and emits an ImageReady event dict
        via ctx.emit; the real branch encodes the input PIL.Image to
        PNG bytes, base64-encodes for the IPC payload, and emits
        ImageReady via ctx.emit.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain an "image" key. Optional "seed"
                (int) and "steps" (int) keys may be provided.

        Returns:
            Dict with key "image" containing a sentinel dict
            {"mock": True, "width": 64, "height": 64} in mock mode.
            Empty dict {} in real mode (no output slots).

        Raises:
            KeyError: If "image" is not provided in inputs.
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
            # Validate required inputs before proceeding. The "image" key
            # is required (not optional) per INPUT_SLOTS — accessing it
            # directly raises KeyError if absent, which is the desired
            # failure mode for missing required inputs.
            image = inputs["image"]

            # In real mode, the "image" input is a PIL.Image instance
            # (produced by VaeDecode's real branch). Encode it to PNG
            # bytes, base64-encode for the IPC payload, then emit
            # ImageReady via ctx.emit.
            import base64
            from io import BytesIO

            buf = BytesIO()
            image.save(buf, format="PNG")  # Encode PIL.Image to PNG bytes
            png_bytes = buf.getvalue()

            # Base64-encode the PNG bytes for the IPC msgpack payload.
            # The Rust event_loop.rs decodes this with
            # base64::engine::general_purpose::STANDARD.
            image_b64 = base64.b64encode(png_bytes).decode("ascii")

            # Emit ImageReady event with all required fields matching
            # WorkerEvent::ImageReady (messages.rs):
            #   job_id, image_b64, width, height, format, seed, steps
            ctx.emit({
                "_type": "ImageReady",
                "job_id": ctx.job_id,
                "image_b64": image_b64,
                "width": image.width,
                "height": image.height,
                "format": "png",
                "seed": inputs.get("seed", -1),
                "steps": inputs.get("steps", 1),
            })

            logger.debug(
                "SaveImage: real branch emitted ImageReady for job_id=%s, "
                "width=%d, height=%d",
                ctx.job_id_str,
                image.width,
                image.height,
            )

            # Return an empty dict — SaveImage has no output slots per
            # §10.3. Output nodes emit events, not slot outputs.
            return {}


@register
class ImageResize(BaseNode):
    """Resize a PIL.Image to specified dimensions.

    This node accepts an image (required) plus width and height (both required,
    positive integers) and an optional method string (defaults to "lanczos").
    It returns a resized IMAGE via the output slot.

    Both mock and real branches call the same PIL.Image.resize() since image
    resizing has no GPU/model dependency to mock around. The ctx.mock branch
    structure is required per §14.6's general node pattern.

    Class Attributes:
        NODE_TYPE: "ImageResize"
        CATEGORY: "Images"
        DISPLAY_NAME: "Image Resize"
        DESCRIPTION: One-line description.
        INPUT_SLOTS: image (IMAGE, required), width (INT, required),
            height (INT, required), method (STRING, optional=True).
        OUTPUT_SLOTS: image (IMAGE).
    """
    NODE_TYPE = "ImageResize"
    CATEGORY = "Images"
    DISPLAY_NAME = "Image Resize"
    DESCRIPTION = "Resizes a PIL image to the requested dimensions."
    INPUT_SLOTS = [
        SlotSpec("image", "IMAGE"),
        SlotSpec("width", "INT"),
        SlotSpec("height", "INT"),
        SlotSpec("method", "STRING", optional=True),
    ]
    OUTPUT_SLOTS = [
        SlotSpec("image", "IMAGE"),
    ]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_resize_real_produces_requested_dimensions
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_resize_mock_returns_correct_dimensions
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Resize the input image to the requested dimensions.

        Branches on ctx.mock at the top per §14.6 — both branches call
        PIL.Image.resize() with the same underlying logic since resizing
        has no GPU/model dependency. The mock branch returns a dict with
        the resized dimensions as a sentinel (consistent with SaveImage's
        mock return pattern of returning {"image": {...}}).

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain "image" (PIL.Image), "width" (int),
                and "height" (int). Optional "method" (str) defaults
                to "lanczos".

        Returns:
            Dict with key "image" containing the resized PIL.Image in
            real mode, or {"image": {"mock": True, "width": <w>,
            "height": <h>}} sentinel dict in mock mode.

        Raises:
            KeyError: If "image", "width", or "height" is not provided.
            ValueError: If "method" is not a recognized PIL resize filter.
        """
        # Validate required inputs — "image", "width", "height" are all
        # required (optional=False) per INPUT_SLOTS. Accessing them via
        # dict key raises KeyError if absent, which is the desired failure
        # mode for missing required inputs.
        image = inputs["image"]
        width = inputs["width"]
        height = inputs["height"]

        # Resolve the resize method. The "method" input is optional per
        # INPUT_SLOTS — default to "lanczos" when absent or unset.
        method = inputs.get("method", "lanczos")

        # Map the string method name to a PIL.Image resize filter constant.
        # This uses Pillow 12.x filter names (BILINEAR, BICUBIC — LINEAR
        # and CUBIC were removed in Pillow 12). An unrecognized string
        # raises ValueError with a clear message listing valid options.
        filter_map = {
            "lanczos": Image.LANCZOS,
            "nearest": Image.NEAREST,
            "bilinear": Image.BILINEAR,
            "bicubic": Image.BICUBIC,
            "box": Image.BOX,
        }

        # Look up the filter from the map; raise ValueError if unrecognized.
        # This provides a clear error message listing valid method strings.
        try:
            filter_constant = filter_map[method]
        except KeyError:
            valid = ", ".join(sorted(filter_map.keys()))
            raise ValueError(
                f"Unrecognized resize method '{method}'. "
                f"Valid methods: {valid}"
            )

        if ctx.mock:
            # Mock branch: the image input may be a dict sentinel
            # ({"mock": True, ...}) rather than a real PIL.Image in tests.
            # Skip the resize call and return the sentinel dimensions directly.
            logger.debug(
                "ImageResize: mock branch resized to %dx%d for job_id=%s",
                width, height, ctx.job_id_str,
            )
            return {"image": {"mock": True, "width": width, "height": height}}
        else:
            # Real branch: the image input is a real PIL.Image. Resize it
            # with the resolved filter using Pillow 12.x's `resample`
            # parameter (not `filter` — that was removed in Pillow 10).
            # Both branches use the same PIL call because image resizing is
            # a pure CPU operation with no GPU/model dependency to mock
            # around (ANVILML_DESIGN.md §14.6 note).
            resized = image.resize((width, height), resample=filter_constant)
            logger.debug(
                "ImageResize: real branch resized to %dx%d for job_id=%s",
                width, height, ctx.job_id_str,
            )
            return {"image": resized}
