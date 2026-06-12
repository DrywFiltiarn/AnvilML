"""Model defaults for diffusion backends.

Pure data module — no I/O, no decision points.
Per FORGE_AGENT_RULES.md §11.1, no logging required.
"""

from dataclasses import dataclass


@dataclass
class ModelDefaults:
    """Default generation parameters for a diffusion model."""

    steps: int
    guidance_scale: float
    width: int
    height: int
    dtype: str
    supports_negative_prompt: bool = False


# ZiT (Zero-Iteration) — distilled model, fewer steps, no CFG needed
ZIT_DEFAULTS = ModelDefaults(
    steps=8,
    guidance_scale=0.0,
    width=1024,
    height=1024,
    dtype="bf16",
)

# SDXL — standard model, requires CFG, supports negative prompts
SDXL_DEFAULTS = ModelDefaults(
    steps=20,
    guidance_scale=7.5,
    width=1024,
    height=1024,
    dtype="fp16",
    supports_negative_prompt=True,
)
