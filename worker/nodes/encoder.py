"""ClipTextEncode node — encodes a text prompt using a loaded CLIP encoder.

This module defines the ``ClipTextEncode`` node, which accepts a ``CLIP``
object, a prompt string, and an optional negative prompt string, then returns
a ``CONDITIONING`` slot. In mock mode (``ANVILML_WORKER_MOCK=1``), it returns
a lightweight ``MockConditioning`` sentinel carrying the encoded text.

In real mode, the CLIP object's ``encode()`` method is called to produce
positive and negative embedding lists (matching ``ZImagePipeline.__call__``'s
``prompt_embeds``/``negative_prompt_embeds`` contract), wrapped in a
``Conditioning`` object.

The ``torch``, ``diffusers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Instead, any real-mode encoding code must import these packages lazily
inside the non-mock code path, which is unreachable when
``ANVILML_WORKER_MOCK=1``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["ClipTextEncode", "MockConditioning", "Conditioning"]


class Conditioning:
    """Conditioning object carrying positive and negative embedding lists.

    Produced by ``ClipTextEncode.execute()`` in real mode. The
    ``.positive`` and ``.negative`` attributes each hold a
    ``list[torch.FloatTensor]`` — one tensor per hidden state
    layer — that downstream nodes (Sampler, VAEDecode, etc.) use
    as classifier-free guidance signals.

    In classifier-free guidance, the sampler processes both the
    positive and negative embeddings in parallel and interpolates
    between them using a guidance scale parameter. The negative
    embedding typically encodes an empty prompt (or a deliberately
    contrasting prompt) to steer generation away from undesired
    features.

    Args:
        positive: List of positive prompt embedding tensors.
            Each tensor is shaped ``(seq_len, hidden_dim)`` where
            ``seq_len`` is the number of non-padding tokens and
            ``hidden_dim`` is the text encoder's hidden size.
        negative: List of negative prompt embedding tensors with
            the same shape as ``positive``.
    """

    def __init__(
        self, positive: list[Any], negative: list[Any]
    ) -> None:
        """Initialise a conditioning object.

        Args:
            positive: List of positive prompt embedding tensors.
            negative: List of negative prompt embedding tensors.
        """
        self.positive = positive
        self.negative = negative


class MockConditioning:
    """Sentinel conditioning object for mock mode.

    Carries the ``text`` attribute so that downstream nodes
    (Sampler, VAEDecode, etc.) can inspect the encoded prompt text
    without needing a real conditioning object from a CLIP encoder.

    In real mode, the actual conditioning object produced by the
    CLIP text encoder will have its own structure defined when
    the real encoding path is implemented (future phase).

    Args:
        text: The text prompt that was encoded.
    """

    def __init__(self, text: str) -> None:
        """Initialise a mock conditioning sentinel.

        Args:
            text: The text prompt that was encoded.
        """
        self.text = text


@register
class ClipTextEncode(BaseNode):
    """Encode a text prompt using a loaded CLIP encoder.

    Accepts a ``CLIP`` object, a prompt string, and an optional
    negative prompt string, then returns a ``CONDITIONING`` slot
    containing either a real conditioning object (in non-mock mode)
    or a ``MockConditioning`` sentinel (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: Three slots — ``clip`` (CLIP, required),
            ``text`` (STRING, required), and ``negative_text``
            (STRING, optional).
        OUTPUT_SLOTS: One ``CONDITIONING`` slot named ``conditioning``.
    """

    NODE_TYPE = "ClipTextEncode"
    CATEGORY = "Conditioning"
    DISPLAY_NAME = "Clip Text Encode"
    DESCRIPTION = "Encode a text prompt using a loaded CLIP encoder"
    INPUT_SLOTS = [
        SlotSpec("clip", "CLIP"),
        SlotSpec("text", "STRING"),
        SlotSpec("negative_text", "STRING", optional=True),
    ]
    OUTPUT_SLOTS = [SlotSpec("conditioning", "CONDITIONING")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the ClipTextEncode node.

        Reads the ``clip``, ``text``, and optional ``negative_text``
        inputs, checks mock mode, and either returns a
        ``MockConditioning`` sentinel or encodes via the real CLIP
        text encoder.

        Args:
            **inputs: Must contain ``"clip"`` (a CLIP object) and
                ``"text"`` (the prompt string). May contain an
                optional ``"negative_text"`` for negative prompt
                encoding.

        Returns:
            Dict with key ``"conditioning"`` containing either a
            ``MockConditioning`` (mock mode) or a ``Conditioning``
            object (real mode).

        Raises:
            Exception: Propagates errors from the CLIP encoder's
                ``encode()`` method (e.g. ``OSError`` for missing
                model files, ``RuntimeError`` for shape mismatches).
        """
        # Read the clip and text inputs from the job graph.
        # The clip object was produced by a prior LoadClip node;
        # the text is the positive prompt string from the job graph.
        clip = inputs.get("clip")
        text = inputs.get("text", "")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # carrying the text instead of encoding via a real
            # CLIP text encoder. This keeps tests fast and avoids
            # requiring GPU hardware or torch. The negative_text
            # input is accepted but unused in mock mode — the real
            # implementation will pass both text and negative_text
            # to clip.encode() for dual-conditioning output.
            return {"conditioning": MockConditioning(text=text)}

        # Real mode: read the negative_text input and call clip.encode()
        # to produce positive and negative embedding lists. The result
        # is wrapped in a Conditioning object for downstream consumers.
        negative_text = inputs.get("negative_text", "")

        # The clip object is a RealClip (or MockClip) with an encode()
        # method that handles both the chat template application,
        # tokenisation, text encoder inference, and attention mask
        # filtering. In mock mode, encode() returns empty lists.
        positive_embeds, negative_embeds = clip.encode(text, negative_text)

        # Dual-conditioning: ZImagePipeline uses classifier-free
        # guidance (always enabled), so both positive and negative
        # embeddings are required. The negative embeds are produced
        # by encoding the negative_text string through the same text
        # encoder pipeline.
        return {"conditioning": Conditioning(positive_embeds, negative_embeds)}
