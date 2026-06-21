"""ClipTextEncode node — encodes a text prompt using a loaded CLIP encoder.

This module defines the ``ClipTextEncode`` node, which accepts a ``CLIP``
object, a prompt string, and an optional negative prompt string, then returns
a ``CONDITIONING`` slot. In mock mode (``ANVILML_WORKER_MOCK=1``), it returns
a lightweight ``MockConditioning`` sentinel carrying the encoded text.

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

__all__ = ["ClipTextEncode", "MockConditioning"]


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
            ``MockConditioning`` (mock mode) or a real conditioning
            object (real mode).

        Raises:
            NotImplementedError: If called in non-mock mode. The real
                CLIP encoding path is stubbed until the real encoder
                implementation is added in a future phase.
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

        # Real mode: encode text using the loaded CLIP encoder.
        # This path is stubbed — the real implementation will call
        # clip.encode(text, negative_text) to produce positive and
        # negative conditioning objects, then merge them into a
        # single CONDITIONING output. The CLIP encoding uses
        # safetensors-loaded weights via the pipeline_cache module.
        # TODO: Implement real CLIP encoding path.
        raise NotImplementedError(
            "Real ClipTextEncode path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )
