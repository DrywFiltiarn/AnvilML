"""End-to-end integration tests: full load + sample + decode chain produces real PIL images.

This file exercises the complete generation pipeline by chaining the underlying
arch modules directly — LoadModel (Phase 20) → Sampler (Phase 21) → decode()
(Phase 23) — against the respective fixture checkpoints. The produced image's
dimensions are verified against the latent spatial dimensions.

torch is guarded, not imported unconditionally: this file imports torch under
a try/except guard at module level so it stays importable in mock-mode CI
collection (the worker-linux-mock / worker-windows-mock jobs install base.txt
only, no torch). The real_mode marker ensures torch is actually available
when these tests run (ANVILML_DESIGN.md §18.3).
"""

from __future__ import annotations

import uuid

import pytest

# Guarded torch import — prevents import errors in mock-mode CI collection.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]

from worker.nodes.arch.diffusion.zit import (
    load as zit_load,
    pipeline_cache,
)
from worker.nodes.arch.vae.zit_vae import (
    decode,
    load as zit_vae_load,
)
from worker.nodes.base import NodeContext
from worker.nodes.sampler import Sampler
from worker.pipeline_cache import PipelineCache

_FIXTURE_DIR = __import__("pathlib").Path(__file__).parent / "fixtures"


def _make_ctx(mock: bool = True) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    return NodeContext(
        job_id=uuid.uuid4().bytes,
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=__import__("threading").Event(),
        emit=lambda e: None,
        pipeline_cache=PipelineCache(),
        mock=mock,
    )


@pytest.mark.real_mode
def test_e2e_full_chain_produces_pil_image() -> None:
    """Full load + sample + decode chain produces a real PIL Image with correct dimensions.

    Loads the ZiT diffusion model and ZiT VAE from fixture checkpoints,
    runs a single-sample denoising pass through the Sampler, decodes the
    resulting latent to a PIL Image, and verifies the image has mode "RGB"
    and dimensions matching the latent spatial size (8×8).

    This is the primary Runnable Proof for the end-to-end generation chain.

    Expected outcome: a PIL Image of size (8, 8), mode "RGB".
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    vae_fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"

    caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    # Load the ZiT diffusion model from fixture.
    model = zit_load(str(fixture_path), caps, device="cpu")

    # Load the ZiT VAE from fixture.
    vae_model = zit_vae_load(str(vae_fixture_path), caps, device="cpu")

    # Create an empty latent tensor of shape (1, 4, 8, 8) matching the
    # fixture's latent dimensions, cast to the model's dtype.
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    # Run the Sampler to get a denoised latent.
    node = Sampler()
    ctx = _make_ctx(mock=False)
    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    denoised_latent = result["latent"]

    # Decode the denoised latent to PIL Images.
    images = decode(vae_model, denoised_latent)

    # Assert the result is a non-empty list of PIL Image objects.
    assert isinstance(images, list)
    assert len(images) > 0

    # Assert image dimensions match the latent spatial size (8×8).
    assert images[0].size == (8, 8), (
        f"expected image size (8, 8), got {images[0].size}"
    )

    # Assert the image mode is RGB.
    assert images[0].mode == "RGB"

    # Clean up pipeline cache entry to avoid leaking state.
    cache_key = f"job_{ctx.job_id_str}:pipeline"
    if isinstance(ctx.pipeline_cache, PipelineCache) and cache_key in ctx.pipeline_cache._cache:
        del ctx.pipeline_cache._cache[cache_key]


@pytest.mark.real_mode
def test_e2e_batch_produces_multiple_images() -> None:
    """Batched latent (2 items) produces 2 PIL Images.

    Same setup as ``test_e2e_full_chain_produces_pil_image`` but with a
    batched latent of shape (2, 4, 8, 8). Asserts the result contains
    exactly 2 images, all with mode "RGB".

    Expected outcome: list of 2 PIL Images, all mode "RGB".
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    vae_fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"

    caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = zit_load(str(fixture_path), caps, device="cpu")
    vae_model = zit_vae_load(str(vae_fixture_path), caps, device="cpu")

    model_dtype = next(model.parameters()).dtype
    # Batch size 2 — two independent samples.
    latent_in = torch.zeros(2, 4, 8, 8, dtype=model_dtype)

    node = Sampler()
    ctx = _make_ctx(mock=False)
    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    denoised_latent = result["latent"]
    images = decode(vae_model, denoised_latent)

    assert len(images) == 2
    assert all(img.mode == "RGB" for img in images)

    # Clean up pipeline cache.
    cache_key = f"job_{ctx.job_id_str}:pipeline"
    if isinstance(ctx.pipeline_cache, PipelineCache) and cache_key in ctx.pipeline_cache._cache:
        del ctx.pipeline_cache._cache[cache_key]


@pytest.mark.real_mode
def test_e2e_image_is_real_pil_not_mock() -> None:
    """Output is genuinely a PIL Image, not a mock sentinel dict.

    Imports ``PIL.Image`` directly and asserts that the result of the
    full chain is an instance of ``PIL.Image.Image`` — confirming this
    is a real image object (with methods like ``.size``, ``.mode``,
    ``.getdata``) rather than a mock sentinel dict like ``{"mock": True}``.

    Expected outcome: ``isinstance(images[0], PIL.Image.Image)`` is True.
    """
    from PIL import Image as PILImage

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    vae_fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"

    caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = zit_load(str(fixture_path), caps, device="cpu")
    vae_model = zit_vae_load(str(vae_fixture_path), caps, device="cpu")

    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    node = Sampler()
    ctx = _make_ctx(mock=False)
    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    images = decode(vae_model, result["latent"])

    # Assert the output is a genuine PIL Image, not a mock sentinel.
    assert isinstance(images[0], PILImage.Image)

    # Clean up pipeline cache.
    cache_key = f"job_{ctx.job_id_str}:pipeline"
    if isinstance(ctx.pipeline_cache, PipelineCache) and cache_key in ctx.pipeline_cache._cache:
        del ctx.pipeline_cache._cache[cache_key]
