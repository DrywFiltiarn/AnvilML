"""ClipTextEncode node — encodes text prompts using a loaded CLIP-compatible encoder.

This node takes a CLIP encoder (already loaded by LoadClip) and positive/negative
text prompts, then produces CONDITIONING output for downstream use by a Sampler node.
In mock mode it returns a sentinel dict; the real branch tokenizes the text using the
encoder's attached tokenizer and runs the encoder's forward pass to produce conditioning
tensors with ``text_embeds`` and optionally ``negative_text_embeds``.
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
    the positive_text value. The real branch tokenizes the text using the
    encoder's attached tokenizer (max_length=77) and runs the encoder's
    forward pass to produce conditioning tensors with ``text_embeds`` and
    optionally ``negative_text_embeds``.

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

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_positive_only
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the ClipTextEncode node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with the propagated positive_text; the
        real branch tokenizes the text using the CLIP encoder's tokenizer
        (max_length=77) and runs the encoder's forward pass to produce
        conditioning tensors.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain "clip" (a CLIP encoder object) and
                "positive_text" (a string). "negative_text" is optional.

        Returns:
            In mock mode: Dict with key "conditioning" containing a
            sentinel dict {"mock": True, "positive_text": <positive_text>}.
            In real mode: Dict with key "conditioning" containing
            {"text_embeds": Tensor(1, 77, hidden_dim)} and optionally
            {"negative_text_embeds": Tensor(1, 77, hidden_dim)}.

        Raises:
            RuntimeError: If the clip encoder lacks a .tokenizer attribute
                (should not happen — the tokenizer is attached by LoadClip's
                real branch via qwen3.py's load() function).
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
            # Real branch: tokenize and encode text prompts using the
            # CLIP encoder's tokenizer and forward pass.
            # The clip encoder is already loaded and has an attached
            # .tokenizer attribute (set by LoadClip's real branch).
            clip_encoder = inputs["clip"]
            tokenizer = clip_encoder.tokenizer

            # Tokenize positive text with standard CLIP context window
            # (77 tokens). padding="max_length" ensures consistent shape;
            # truncation=True handles prompts longer than 77 tokens.
            positive_tokens = tokenizer(
                inputs["positive_text"],
                padding="max_length",
                max_length=77,
                truncation=True,
                return_tensors="pt",
            )

            # Run encoder forward pass to get hidden states.
            # Shape: (batch=1, seq_len=77, hidden_dim).
            positive_embeds = clip_encoder.forward(positive_tokens["input_ids"])

            # Build the conditioning dict with positive text embeds.
            # This is the standard ComfyUI conditioning format that
            # downstream Sampler nodes expect.
            conditioning: dict = {"text_embeds": positive_embeds}

            # Tokenize and encode negative text if provided.
            # The negative_text slot is optional; when omitted, only
            # text_embeds is included in the conditioning dict.
            negative_text = inputs.get("negative_text")
            if negative_text is not None:
                negative_tokens = tokenizer(
                    negative_text,
                    padding="max_length",
                    max_length=77,
                    truncation=True,
                    return_tensors="pt",
                )
                negative_embeds = clip_encoder.forward(negative_tokens["input_ids"])
                conditioning["negative_text_embeds"] = negative_embeds

            logger.debug(
                "ClipTextEncode: real mode, text_embeds.shape=%s, "
                "has_negative=%s",
                tuple(positive_embeds.shape),
                "negative_text_embeds" in conditioning,
            )

            return {"conditioning": conditioning}
