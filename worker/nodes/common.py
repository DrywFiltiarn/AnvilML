"""Common worker nodes shared across diffusion backends.

Currently provides ``SaveImage`` which encodes a PIL Image to PNG and emits
an ``ImageReady`` IPC event via ``ctx.emit_fn``.
"""

from __future__ import annotations

import base64
import logging
import os
import random
from io import BytesIO
from typing import Any

from PIL import Image

from worker.nodes.base import BaseNode, register

logger = logging.getLogger(__name__)

# ── Conditional torch import (same guard as executor.py) ──────────────────────

_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

if _mock:
    torch: Any = None  # type: ignore[name-defined]
else:
    import torch  # noqa: E402  # type: ignore[assignment]


@register
class SaveImage(BaseNode):
    """Encode a PIL Image to PNG and emit an ``ImageReady`` IPC event.

    INPUT_SLOTS:  ``["image", "prompt", "seed", "steps"]``
    OUTPUT_SLOTS: ``[]``
    """

    NODE_TYPE = "SaveImage"
    INPUT_SLOTS = ["image", "prompt", "seed", "steps"]
    OUTPUT_SLOTS = []

    def execute(
        self,
        image: Any = None,
        prompt: str = "",
        seed: int = -1,
        steps: int = 1,
    ) -> dict[str, Any]:
        """Encode image to PNG, emit ImageReady, return empty dict."""
        if seed == -1:
            seed = random.randint(0, 2**63 - 1)

        # Encode PIL image to base64 PNG.
        if image is None or isinstance(image, str):
            # Missing input or mock sentinel — encode a 64×64 black PNG.
            logger.debug("SaveImage: encoding black placeholder")
            img = Image.new("RGB", (64, 64), (0, 0, 0))
        elif isinstance(image, Image.Image):
            img = image
        else:
            # Real tensor path — convert to PIL.
            img = Image.from_tensor(image)

        buf = BytesIO()
        img.save(buf, format="PNG")
        image_b64 = base64.b64encode(buf.getvalue()).decode("ascii")

        event = {
            "_type": "ImageReady",
            "job_id": self.ctx.job_id,
            "image_b64": image_b64,
            "width": img.width,
            "height": img.height,
            "format": "png",
            "seed": seed,
            "steps": steps,
            "prompt": prompt,
        }
        self.ctx.emit_fn(event)
        logger.debug(
            "SaveImage: job=%s size=%dx%d b64_len=%d",
            self.ctx.job_id, img.width, img.height,
            len(image_b64),
        )
        return {}
