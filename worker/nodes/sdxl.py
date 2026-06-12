"""SDXL diffusion pipeline nodes.

Four ``@register``-decorated node classes that implement the SDXL pipeline
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


# ── Sentinel types for mock mode ──────────────────────────────────────────────


@dataclass
class _MockPipeline:
    """Stub pipeline object returned by ``SdxlLoadPipeline`` in mock mode."""

    name: str = "sdxl_pipeline_mock"


@dataclass
class _MockTensor:
    """Stub tensor object for mock-mode conditioning / latents."""

    shape: tuple[int, ...]


# ── Node implementations ──────────────────────────────────────────────────────


@register
class SdxlLoadPipeline(BaseNode):
    """Load an SDXL diffusion pipeline from a model hub ID.

    INPUT_SLOTS:  ``["model_id"]``
    OUTPUT_SLOTS: ``["pipeline"]``
    """

    NODE_TYPE = "SdxlLoadPipeline"
    INPUT_SLOTS = ["model_id"]
    OUTPUT_SLOTS = ["pipeline"]

    def execute(self, model_id: str) -> dict[str, Any]:
        """Load or return cached SDXL pipeline."""
        if _mock:
            logger.debug(
                "SdxlLoadPipeline mock: model=%s", model_id,
            )
            return {"pipeline": _MockPipeline()}

        try:
            from diffusers import StableDiffusionXLPipeline
        except ImportError:
            logger.warning(
                "diffusers.StableDiffusionXLPipeline unavailable for model=%s — "
                "using mock sentinel", model_id,
            )
            return {"pipeline": _MockPipeline()}

        pipeline = self.ctx.pipeline_cache.get_or_load(
            model_id=model_id,
            dtype="fp16",
            loader=lambda: StableDiffusionXLPipeline.from_pretrained(
                model_id, torch_dtype=torch.float16,
            ),
        )
        logger.debug(
            "SdxlLoadPipeline loaded: model=%s (cache_size=%d)",
            model_id, self.ctx.pipeline_cache.size,
        )
        return {"pipeline": pipeline}


@register
class SdxlTextEncode(BaseNode):
    """Encode text prompts into conditioning embeddings for an SDXL pipeline.

    INPUT_SLOTS:  ``["pipeline", "prompt", "negative_prompt"]``
    OUTPUT_SLOTS: ``["conditioning"]``
    """

    NODE_TYPE = "SdxlTextEncode"
    INPUT_SLOTS = ["pipeline", "prompt", "negative_prompt"]
    OUTPUT_SLOTS = ["conditioning"]

    def execute(
        self, pipeline: Any, prompt: str, negative_prompt: str,
    ) -> dict[str, Any]:
        """Return conditioning tensor pair for the given prompts."""
        if _mock:
            logger.debug(
                "SdxlTextEncode mock: prompt=%s negative=%s",
                prompt, negative_prompt,
            )
            return {
                "conditioning": (
                    _MockTensor((1, 77, 2048)),
                    _MockTensor((1, 2048)),
                ),
            }

        result = pipeline(
            prompt=prompt,
            negative_prompt=negative_prompt,
            return_dict=False,
        )
        logger.debug(
            "SdxlTextEncode: prompt_len=%d output_shapes=%s",
            len(prompt),
            tuple(t.shape for t in result),
        )
        return {"conditioning": result}


@register
class SdxlSampler(BaseNode):
    """Sample latents from an SDXL pipeline using the given conditioning.

    INPUT_SLOTS:  ``["pipeline", "conditioning", "steps", "guidance_scale", "seed"]``
    OUTPUT_SLOTS: ``["latents", "seed"]``
    """

    NODE_TYPE = "SdxlSampler"
    INPUT_SLOTS = ["pipeline", "conditioning", "steps", "guidance_scale", "seed"]
    OUTPUT_SLOTS = ["latents", "seed"]

    def execute(
        self,
        pipeline: Any,
        conditioning: tuple,
        steps: int,
        guidance_scale: float,
        seed: int,
    ) -> dict[str, Any]:
        """Run the SDXL sampler and return latents + actual seed."""
        if _mock:
            actual_seed = random.randint(0, 2**63 - 1) if seed == -1 else seed
            logger.debug(
                "SdxlSampler mock: steps=%d seed=%d guidance=%s",
                steps, actual_seed, guidance_scale,
            )
            return {
                "latents": _MockTensor((1, 4, 128, 128)),
                "seed": actual_seed,
            }

        actual_seed = random.randint(0, 2**63 - 1) if seed == -1 else seed
        device_str = self.ctx.device_str
        generator = torch.Generator(device=device_str).manual_seed(actual_seed)

        latents = pipeline(
            prompt_embeds=conditioning[0],
            negative_prompt_embeds=conditioning[1],
            generator=generator,
            num_inference_steps=steps,
            guidance_scale=guidance_scale,
        )
        logger.debug(
            "SdxlSampler: steps=%d seed=%d latents_shape=%s",
            steps, actual_seed, latents.shape,
        )
        return {"latents": latents, "seed": actual_seed}


@register
class SdxlDecode(BaseNode):
    """Decode latent representations into a PIL Image for SDXL.

    INPUT_SLOTS:  ``["pipeline", "latents"]``
    OUTPUT_SLOTS: ``["image"]``
    """

    NODE_TYPE = "SdxlDecode"
    INPUT_SLOTS = ["pipeline", "latents"]
    OUTPUT_SLOTS = ["image"]

    def execute(self, pipeline: Any, latents: Any) -> dict[str, Any]:
        """Decode latents and return a PIL Image."""
        if _mock:
            logger.debug(
                "SdxlDecode mock: latents_shape=%s",
                getattr(latents, "shape", "unknown"),
            )
            return {"image": "sdxl_image_mock"}

        from PIL import Image

        decoded = pipeline.vae.decode(
            latents / 0.1842,
            return_dict=False,
        )[0]
        pil_image = Image.from_tensor(decoded)
        logger.debug(
            "SdxlDecode: output_size=%dx%d",
            pil_image.width, pil_image.height,
        )
        return {"image": pil_image}
