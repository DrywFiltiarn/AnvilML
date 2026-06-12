"""ZiT (Zero-Iteration) diffusion pipeline nodes.

Four ``@register``-decorated node classes that implement the ZiT pipeline
stages: load, text encode, sample, and decode.

When ``ANVILML_WORKER_MOCK=1`` all nodes take the fast sentinel path
and never import ``torch`` or ``diffusers``.
"""

from __future__ import annotations

import logging
import os
import random
from dataclasses import dataclass
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, register

logger = logging.getLogger(__name__)

# ── Conditional torch / diffusers import ──────────────────────────────────────

_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

if _mock:
    torch: Any = None  # type: ignore[name-defined]
else:
    import torch  # noqa: E402  # type: ignore[assignment]


# ── CancelledError ─────────────────────────────────────────────────────────────
# Re-exported from executor for node-level cancellation signaling.


class CancelledError(Exception):
    """Raised by a node to signal that the current job was cancelled."""


# ── Sentinel types for mock mode ──────────────────────────────────────────────


@dataclass
class _MockPipeline:
    """Stub pipeline object returned by ``ZitLoadPipeline`` in mock mode."""

    name: str = "zit_pipeline_mock"


@dataclass
class _MockTensor:
    """Stub tensor object for mock-mode conditioning / latents."""

    shape: tuple[int, ...]


# ── Node implementations ──────────────────────────────────────────────────────


@register
class ZitLoadPipeline(BaseNode):
    """Load a ZiT diffusion pipeline from a model hub ID.

    INPUT_SLOTS:  ``["model_id"]``
    OUTPUT_SLOTS: ``["pipeline"]``
    """

    NODE_TYPE = "ZitLoadPipeline"
    INPUT_SLOTS = ["model_id"]
    OUTPUT_SLOTS = ["pipeline"]

    def execute(self, model_id: str) -> dict[str, Any]:
        """Load or return cached ZiT pipeline."""
        if _mock:
            logger.debug(
                "ZitLoadPipeline mock: model=%s", model_id,
            )
            return {"pipeline": _MockPipeline()}

        try:
            from diffusers import ZitsPipeline
        except ImportError:
            logger.warning(
                "diffusers.ZitsPipeline unavailable for model=%s — "
                "using mock sentinel", model_id,
            )
            return {"pipeline": _MockPipeline()}

        pipeline = self.ctx.pipeline_cache.get_or_load(
            model_id=model_id,
            dtype="bf16",
            loader=lambda: ZitsPipeline.from_pretrained(
                model_id, torch_dtype=torch.bfloat16,
            ),
        )
        logger.debug(
            "ZitLoadPipeline loaded: model=%s (cache_size=%d)",
            model_id, self.ctx.pipeline_cache.size,
        )
        return {"pipeline": pipeline}


@register
class ZitTextEncode(BaseNode):
    """Encode text prompts into conditioning embeddings for a ZiT pipeline.

    INPUT_SLOTS:  ``["pipeline", "prompt"]``
    OUTPUT_SLOTS: ``["conditioning"]``
    """

    NODE_TYPE = "ZitTextEncode"
    INPUT_SLOTS = ["pipeline", "prompt"]
    OUTPUT_SLOTS = ["conditioning"]

    def execute(self, pipeline: Any, prompt: str) -> dict[str, Any]:
        """Return conditioning tensor pair for the given prompt."""
        if _mock:
            logger.debug("ZitTextEncode mock: prompt=%s", prompt)
            return {"conditioning": (_MockTensor((1, 77, 768)), _MockTensor((1, 768)))}

        result = pipeline(text=prompt, return_dict=False)
        logger.debug(
            "ZitTextEncode: prompt_len=%d output_shapes=%s",
            len(prompt),
            tuple(t.shape for t in result),
        )
        return {"conditioning": result}


@register
class ZitSampler(BaseNode):
    """Sample latents from a ZiT pipeline using the given conditioning.

    INPUT_SLOTS:  ``["pipeline", "conditioning", "steps", "seed"]``
    OUTPUT_SLOTS: ``["latents", "seed"]``
    """

    NODE_TYPE = "ZitSampler"
    INPUT_SLOTS = ["pipeline", "conditioning", "steps", "seed"]
    OUTPUT_SLOTS = ["latents", "seed"]

    def execute(
        self,
        pipeline: Any,
        conditioning: tuple,
        steps: int,
        seed: int,
    ) -> dict[str, Any]:
        """Run the ZiT sampler and return latents + actual seed."""
        if _mock:
            actual_seed = random.randint(0, 2**63 - 1) if seed == -1 else seed
            logger.debug("ZitSampler mock: steps=%d seed=%d", steps, actual_seed)
            return {
                "latents": _MockTensor((1, 4, 64, 64)),
                "seed": actual_seed,
            }

        actual_seed = random.randint(0, 2**63 - 1) if seed == -1 else seed
        device_str = self.ctx.device_str
        generator = torch.Generator(device=device_str).manual_seed(actual_seed)

        def _callback(step: int, timestep: int, callback_dict: dict) -> None:
            if self.ctx.cancel_flag.is_set():
                raise CancelledError("job cancelled during sampling")

        latents = pipeline(
            prompt_embeds=conditioning[0],
            generator=generator,
            num_inference_steps=steps,
            callback_on_step_end=_callback,
            return_dict=False,
        )
        logger.debug(
            "ZitSampler: steps=%d seed=%d latents_shape=%s",
            steps, actual_seed, latents.shape,
        )
        return {"latents": latents, "seed": actual_seed}


@register
class ZitDecode(BaseNode):
    """Decode latent representations into a PIL Image.

    INPUT_SLOTS:  ``["pipeline", "latents"]``
    OUTPUT_SLOTS: ``["image"]``
    """

    NODE_TYPE = "ZitDecode"
    INPUT_SLOTS = ["pipeline", "latents"]
    OUTPUT_SLOTS = ["image"]

    def execute(self, pipeline: Any, latents: Any) -> dict[str, Any]:
        """Decode latents and return a PIL Image."""
        if _mock:
            logger.debug("ZitDecode mock: latents_shape=%s", getattr(latents, "shape", "unknown"))
            return {"image": "zit_image_mock"}

        from PIL import Image

        decoded = pipeline.vae.decode(
            latents / 0.1842,
            return_dict=False,
        )[0]
        pil_image = Image.from_tensor(decoded)
        logger.debug(
            "ZitDecode: output_size=%dx%d",
            pil_image.width, pil_image.height,
        )
        return {"image": pil_image}
