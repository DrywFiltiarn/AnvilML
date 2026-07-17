"""ClipTextEncode node — encodes text prompts using a loaded CLIP-compatible encoder.

This node takes a CLIP encoder (already loaded by LoadClip) and positive/negative
text prompts, then produces CONDITIONING output for downstream use by a Sampler node.
In mock mode it returns a sentinel dict; the real branch (P24-A2) will tokenize and
encode the text using the CLIP encoder.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

import logging

logger = logging.getLogger(__name__)


@register
class ClipTextEncode(BaseNode):
    """Encode a text prompt using a loaded CLIP-compatible encoder.

    This node takes a CLIP encoder object (produced by LoadClip) along with
    positive and optional negative text prompts, and produces CONDITIONING
    output for use by downstream nodes such as Sampler.

    In mock mode (ANVILML_WORKER_MOCK=1) it returns a sentinel dict carrying
    the positive_text value. The real branch, implemented in P24-A2, will
    call the CLIP encoder's tokenizer and forward pass to produce
    conditioning tensors.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to in the node graph.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: "clip" (CLIP, required), "positive_text" (STRING, required),
            and "negative_text" (STRING, optional).
        OUTPUT_SLOTS: Single output slot named "conditioning" with type "CONDITIONING".
    """

    NODE_TYPE = "ClipTextEncode"
    CATEGORY = "Conditioning"
    DISPLAY_NAME = "Clip Text Encode"
    DESCRIPTION = "Encodes a text prompt using a loaded CLIP-compatible encoder."
    INPUT_SLOTS = [
        SlotSpec("clip", "CLIP"),
        SlotSpec("positive_text", "STRING"),
        SlotSpec("negative_text", "STRING", optional=True),
    ]
    OUTPUT_SLOTS = [SlotSpec("conditioning", "CONDITIONING")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_raises_placeholder
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the ClipTextEncode node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with the propagated positive_text; the
        real branch (P24-A2) will tokenize and encode the text using
        the loaded CLIP encoder.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain "clip" (a CLIP encoder object) and
                "positive_text" (a string). "negative_text" is optional.

        Returns:
            In mock mode: Dict with key "conditioning" containing a
            sentinel dict {"mock": True, "positive_text": <positive_text>}.
            In real mode: Dict with key "conditioning" containing
            conditioning tensors (P24-A2).

        Raises:
            NotImplementedError: In real mode — the real branch is
                deferred to P24-A2 which will implement actual
                tokenization and encoding.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real encoding.
            # The sentinel carries the positive_text so downstream tests
            # can verify the correct value was propagated through the
            # node system. negative_text is not included in the sentinel
            # because the CONDITIONING output in real mode is determined
            # by the positive prompt; negative conditioning is a separate
            # downstream concern handled by the graph executor.
            logger.debug(
                "ClipTextEncode: mock mode, positive_text=%s",
                inputs["positive_text"],
            )
            return {
                "conditioning": {
                    "mock": True,
                    "positive_text": inputs["positive_text"],
                }
            }
        else:
            # defers_to: P24-A1 — real branch implementation deferred to
            # P24-A2 which will tokenize and encode the text prompts using
            # the CLIP encoder's tokenizer and forward pass.
            raise NotImplementedError("real branch in P24-A2")
