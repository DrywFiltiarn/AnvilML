"""SaveImage node — outputs an image via the IPC event channel.

This module defines the ``SaveImage`` node, which accepts an image
input and emits an ``ImageReady`` event containing a base64-encoded
PNG representation. In mock mode (the only mode implemented so far),
the node generates a minimal 64×64 black PNG using only Python
stdlib — no PIL, torch, or diffusers imports.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import base64
import io
import struct
import zlib
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["SaveImage"]


def _generate_black_png(width: int = 64, height: int = 64) -> bytes:
    """Generate a minimal black RGB PNG using only stdlib.

    Constructs the PNG binary from scratch: PNG signature, IHDR chunk
    (with width, height, bit depth, color type), and IDAT chunk
    (zlib-compressed scanlines of zero bytes). CRC checksums for IHDR
    and IDAT are computed by Python's ``zlib.crc32`` automatically.

    Args:
        width: Image width in pixels. Defaults to 64.
        height: Image height in pixels. Defaults to 64.

    Returns:
        Raw PNG binary data.
    """
    buf = io.BytesIO()

    # PNG magic bytes — fixed signature required by the PNG spec.
    buf.write(b"\x89PNG\r\n\x1a\n")

    # IHDR chunk: width (4B), height (4B), bit depth (1B),
    # color type (1B = 2 for RGB), compression (1B), filter (1B),
    # interlace (1B).
    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    ihdr_crc = zlib.crc32(b"IHDR" + ihdr_data)
    buf.write(struct.pack(">I", len(ihdr_data)))
    buf.write(b"IHDR")
    buf.write(ihdr_data)
    buf.write(struct.pack(">I", ihdr_crc & 0xFFFFFFFF))

    # IDAT chunk: zlib-compressed scanlines.
    # Each scanline starts with filter byte 0 (None), followed by
    # width * 3 bytes (RGB). All zeros = black image.
    raw_scanlines = b""
    for _ in range(height):
        raw_scanlines += b"\x00" + b"\x00" * (width * 3)

    compressed = zlib.compress(raw_scanlines)
    idat_crc = zlib.crc32(b"IDAT" + compressed)
    buf.write(struct.pack(">I", len(compressed)))
    buf.write(b"IDAT")
    buf.write(compressed)
    buf.write(struct.pack(">I", idat_crc & 0xFFFFFFFF))

    # IEND chunk: marks the end of the PNG stream.
    iend_crc = zlib.crc32(b"IEND")
    buf.write(struct.pack(">I", 0))
    buf.write(b"IEND")
    buf.write(struct.pack(">I", iend_crc & 0xFFFFFFFF))

    return buf.getvalue()


@register
class SaveImage(BaseNode):
    """Save an image by emitting an ``ImageReady`` IPC event.

    This node accepts a single image input and generates a minimal
    64×64 black PNG (in mock mode), encodes it as base64, and emits
    an ``ImageReady`` event via the node context's ``emit`` callable.

    No output slots are defined — the image is transmitted via the
    IPC event channel rather than a slot output.

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: One required ``IMAGE`` slot named ``"image"``.
        OUTPUT_SLOTS: Empty — output is via IPC event, not slot.
    """

    NODE_TYPE = "SaveImage"
    CATEGORY = "Output"
    DISPLAY_NAME = "Save Image"
    DESCRIPTION = "Emit an image via the IPC event channel"
    INPUT_SLOTS = [SlotSpec("image", "IMAGE")]
    OUTPUT_SLOTS: list[SlotSpec] = []

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the SaveImage node.

        Reads the ``"image"`` input (which will be ``None`` in mock
        mode since no upstream nodes exist yet), generates a minimal
        64×64 black PNG, encodes it as base64, and emits an
        ``ImageReady`` event with optional ``seed`` and ``steps``
        fields for reproducibility tracking.

        Args:
            **inputs: Keyword arguments keyed by input slot name.
                Must contain ``"image"``. May optionally contain
                ``"seed"`` (the resolved seed from the Sampler node)
                and ``"steps"`` (the number of denoising steps).

        Returns:
            Empty dict — this node has no output slots.
        """
        # Read the image input. In mock mode this will be None
        # since there are no upstream nodes producing images yet.
        # The node ignores the input and generates its own mock PNG.
        image = inputs.get("image")

        # Generate a minimal 64×64 black PNG using only stdlib.
        # This is the mock-mode implementation — no PIL, torch, or
        # diffusers imports. The PNG is constructed byte-by-byte
        # using struct.pack for the binary layout and zlib for
        # compression.
        png_data = _generate_black_png(64, 64)
        b64 = base64.b64encode(png_data).decode("ascii")

        # Read optional seed and steps from inputs. These arrive only
        # when wired from upstream nodes (e.g. Sampler passes its
        # resolved seed value). They are not declared as formal slot
        # specs — accepted via **inputs but not in INPUT_SLOTS.
        seed = inputs.get("seed")
        steps = inputs.get("steps")

        # Emit the ImageReady event via the IPC channel.
        # The event carries the job_id, base64-encoded PNG,
        # dimensions, and optional seed/steps so the supervisor
        # can forward them to the UI for reproducibility tracking.
        self.ctx.emit({
            "_type": "ImageReady",
            "job_id": self.ctx.job_id,
            "image_b64": b64,
            "width": 64,
            "height": 64,
            "seed": seed,
            "steps": steps,
        })

        # No output slots — the image is transmitted via IPC event.
        return {}
