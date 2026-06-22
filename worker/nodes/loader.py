"""LoadModel node — loads a diffusion model from a safetensors file.

This module defines the ``LoadModel`` node, which accepts a ``model_id``
STRING input and outputs a ``MODEL`` slot containing either a real loaded
pipeline model object (in non-mock mode) or a lightweight ``MockModel``
sentinel (in mock mode).

The ``torch``, ``diffusers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Instead, any real-mode loading code must import these packages lazily
inside the non-mock code path, which is unreachable when
``ANVILML_WORKER_MOCK=1``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["LoadModel", "LoadVae", "MockModel", "MockVae", "MockClip"]


class MockModel:
    """Sentinel model object for mock mode.

    Carries only the ``arch`` attribute so that downstream nodes
    (Sampler, etc.) can inspect the model architecture without
    needing a real diffusers pipeline object.

    Args:
        arch: The model architecture identifier (e.g. ``"zit"``).
    """

    def __init__(self, arch: str) -> None:
        """Initialise a mock model sentinel.

        Args:
            arch: The model architecture identifier.
        """
        self.arch = arch


class MockVae:
    """Sentinel VAE object for mock mode.

    A lightweight placeholder that stands in for a real VAE pipeline
    component during testing. Real VAE objects produced by the
    safetensors loading path will have their own structure defined
    when ``pipeline_cache.py`` is implemented (P18-D1).
    """
    pass


class MockClip:
    """Sentinel CLIP object for mock mode.

    A lightweight placeholder carrying the ``clip_type`` attribute
    so that downstream nodes can inspect the tokeniser type
    without needing a real text-encoder pipeline object.

    Args:
        clip_type: The tokeniser type identifier (e.g. ``"qwen3"``,
            ``"clip_l"``, ``"t5"``). Defaults to ``"qwen3"``.
    """

    def __init__(self, clip_type: str = "qwen3") -> None:
        """Initialise a mock CLIP sentinel.

        Args:
            clip_type: The tokeniser type identifier.
        """
        self.clip_type = clip_type


class RealModel:
    """Lightweight wrapper around a diffusers transformer component.

    The real diffusers transformer object carries config data
    (e.g. ``in_channels``) that downstream consumers like
    ``EmptyLatent`` need to read, and it does not expose an
    ``.arch`` attribute. This wrapper preserves both the
    ``.arch`` interface that downstream code already expects
    from ``MockModel`` and the ``.in_channels`` value from the
    transformer's config.

    Args:
        transformer: A diffusers model instance (e.g.
            ``ZImageTransformer2DModel``) loaded from safetensors.
        arch: The architecture identifier string (e.g. ``"zit"``).
    """

    def __init__(self, transformer: Any, arch: str) -> None:
        """Initialise the real model wrapper.

        Args:
            transformer: The diffusers transformer component.
            arch: The architecture identifier for dispatch.
        """
        self._transformer = transformer
        self._arch = arch

    @property
    def arch(self) -> str:
        """The architecture identifier string."""
        return self._arch

    @property
    def in_channels(self) -> int:
        """Number of latent channels from the transformer's config."""
        return self._transformer.config.in_channels


@register
class LoadModel(BaseNode):
    """Load a diffusion model from a safetensors file.

    Accepts a ``model_id`` string input and returns a ``MODEL``
    slot containing either a real loaded pipeline component
    (in non-mock mode) or a ``MockModel`` sentinel (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: One required ``STRING`` slot named ``"model_id"``.
        OUTPUT_SLOTS: One ``MODEL`` slot named ``"model"``.
    """

    NODE_TYPE = "LoadModel"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load Model"
    DESCRIPTION = "Load a diffusion model (UNet or DiT) from a safetensors file"
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the LoadModel node.

        Reads the ``model_id`` input, checks mock mode, and either
        returns a ``MockModel`` sentinel or loads a real model via
        safetensors + pipeline_cache.

        Args:
            **inputs: Must contain ``"model_id"`` — the identifier
                of the model to load.

        Returns:
            Dict with key ``"model"`` containing either a ``MockModel``
            (mock mode) or a loaded pipeline model object (real mode).

        Raises:
            NotImplementedError: If called in non-mock mode. The real
                safetensors loading path is stubbed until P18-D1.
        """
        # Read the model_id input. In mock mode this is a
        # placeholder string; in real mode it references a
        # model directory or file path registered in the model store.
        model_id = inputs.get("model_id", "")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # instead of loading a real pipeline. This keeps tests
            # fast and avoids requiring GPU hardware or torch.
            # The arch="zit" matches the Phase 018 baseline model.
            return {"model": MockModel(arch="zit")}

        # Real mode: load actual safetensors weights.
        # Lazy imports — these packages are not available in mock mode
        # (no torch installed), so importing them here keeps the worker
        # importable when ANVILML_WORKER_MOCK=1.
        from safetensors.torch import safe_open
        from diffusers import ZImageTransformer2DModel
        import torch

        # Open the safetensors file to read metadata before loading.
        # framework="pt" is used because safetensors supports multiple
        # backends (pt, np, tf, jax); "pt" selects the PyTorch reader
        # which is what diffusers expects for model loading.
        with safe_open(model_id, framework="pt") as st:
            # Read architecture from safetensors file metadata.
            # The metadata dict is populated by the safetensors writer;
            # if absent, fall back to the directory naming convention
            # because not all model exports embed arch metadata.
            metadata = st.metadata
            arch = (metadata.get("arch") if metadata else None) or model_id

        # If the arch string still looks like a path (contains "/" or
        # "\\"), extract the directory name as the architecture hint.
        # This handles the common case where model_id is a directory
        # path like "/models/zit-fp8/unet" — we take the last component.
        # The "models/" directory naming convention uses the directory
        # name as the architecture identifier when metadata is absent.
        if "/" in arch or "\\" in arch:
            arch = arch.split("/")[-1].split("\\")[-1]

        # Define the loader closure that constructs the transformer.
        # This is passed to pipeline_cache.get_or_load() so the actual
        # model loading only happens on cache miss. The closure captures
        # model_id, arch, and torch_dtype to avoid redundant resolution.
        def loader_fn() -> RealModel:
            # ZImageTransformer2DModel is the correct diffusers class
            # for Z-Image Turbo FP8 models. It provides the transformer
            # backbone that the diffusion pipeline's denoising loop
            # calls at every step. from_pretrained loads from the
            # "unet" subfolder because the safetensors weights for
            # the transformer are stored there in the standard layout.
            transformer = ZImageTransformer2DModel.from_pretrained(
                model_id,
                subfolder="unet",
                torch_dtype=torch.float16,  # FP8 models load as FP16; the pipeline keeps the transformer at FP8 via InferenceCaps.
            )
            return RealModel(transformer, arch=arch)

        # Get the model from cache or load it via loader_fn.
        # The cache key uses "fp8" dtype because Z-Image Turbo FP8
        # models are stored and served in FP8 precision. The dtype
        # string is part of the cache key so FP8 and FP16 variants
        # of the same model are cached independently.
        # Note: ctx.pipeline_cache is typed as dict[str, Any] in
        # NodeContext but a PipelineCache instance at runtime
        # (retrofitted by P903-A2), so .get_or_load() is available.
        result = ctx.pipeline_cache.get_or_load(
            model_id, "fp8", loader_fn
        )
        return {"model": result}


@register
class LoadVae(BaseNode):
    """Load a VAE from a standalone safetensors file.

    Accepts a ``model_id`` string input and returns a ``VAE``
    slot containing either a real loaded VAE pipeline component
    (in non-mock mode) or a ``MockVae`` sentinel (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: One required ``STRING`` slot named ``"model_id"``.
        OUTPUT_SLOTS: One ``VAE`` slot named ``"vae"``.
    """

    NODE_TYPE = "LoadVae"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load VAE"
    DESCRIPTION = "Load a VAE from a standalone safetensors file"
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("vae", "VAE")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the LoadVae node.

        Reads the ``model_id`` input, checks mock mode, and either
        returns a ``MockVae`` sentinel or loads a real VAE via
        safetensors + pipeline_cache.

        Args:
            **inputs: Must contain ``"model_id"`` — the identifier
                of the VAE to load.

        Returns:
            Dict with key ``"vae"`` containing either a ``MockVae``
            (mock mode) or a loaded VAE pipeline component (real mode).

        Raises:
            Exception: Propagates errors from diffusers model loading
                (e.g. ``OSError`` if the model path is invalid,
                ``ValueError`` if the config is malformed).
        """
        # Read the model_id input. In mock mode this is a
        # placeholder string; in real mode it references a
        # VAE directory path registered in the model store.
        model_id = inputs.get("model_id", "")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # instead of loading a real VAE pipeline. This keeps
            # tests fast and avoids requiring GPU hardware or torch.
            return {"vae": MockVae()}

        # Real mode: load actual VAE weights via diffusers.
        # Lazy imports — these packages are not available in mock mode
        # (no torch installed), so importing them here keeps the worker
        # importable when ANVILML_WORKER_MOCK=1.
        from diffusers import AutoencoderKL
        import torch

        # Define the loader closure that constructs the AutoencoderKL.
        # This is passed to pipeline_cache.get_or_load() so the actual
        # model loading only happens on cache miss. The closure captures
        # model_id and torch_dtype to avoid redundant resolution.
        # from_pretrained with subfolder="vae" is used because the VAE
        # weights and config.json reside in a "vae/" subdirectory within
        # the model directory, matching the standard diffusers layout.
        def loader_fn() -> AutoencoderKL:
            return AutoencoderKL.from_pretrained(
                model_id,
                subfolder="vae",
                torch_dtype=torch.bfloat16,
            )

        # Get the VAE from cache or load it via loader_fn.
        # The cache key uses "bf16" dtype string — bfloat16 is the
        # native half-precision format for modern GPUs (A100/H100)
        # and avoids the overflow issues of fp16 during training.
        # Note: ctx.pipeline_cache is typed as dict[str, Any] in
        # NodeContext but a PipelineCache instance at runtime
        # (retrofitted by P903-A2), so .get_or_load() is available.
        result = ctx.pipeline_cache.get_or_load(
            model_id, "bf16", loader_fn
        )
        return {"vae": result}


@register
class LoadClip(BaseNode):
    """Load a text encoder (CLIP/T5/Qwen3) from a safetensors file.

    Accepts a ``model_id`` string input and an optional ``clip_type``
    hint, then returns a ``CLIP`` slot containing either a real loaded
    text-encoder pipeline component (in non-mock mode) or a
    ``MockClip`` sentinel carrying the resolved tokeniser type
    (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: One required ``STRING`` slot named ``"model_id"``,
            and one optional ``STRING`` slot named ``"clip_type"``.
        OUTPUT_SLOTS: One ``CLIP`` slot named ``"clip"``.
    """

    NODE_TYPE = "LoadClip"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load CLIP"
    DESCRIPTION = "Load a text encoder (CLIP/T5/Qwen3) from a safetensors file"
    INPUT_SLOTS = [SlotSpec("model_id", "STRING"),
                   SlotSpec("clip_type", "STRING", optional=True)]
    OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the LoadClip node.

        Reads the ``model_id`` and optional ``clip_type`` inputs,
        checks mock mode, and either returns a ``MockClip`` sentinel
        or loads a real text encoder via safetensors + pipeline_cache.

        Args:
            **inputs: Must contain ``"model_id"`` — the identifier
                of the text encoder to load.  May contain an optional
                ``"clip_type"`` to specify the tokeniser type
                (e.g. ``"qwen3"``, ``"clip_l"``, ``"t5"``).

        Returns:
            Dict with key ``"clip"`` containing either a ``MockClip``
            (mock mode) or a loaded text-encoder object (real mode).

        Raises:
            NotImplementedError: If called in non-mock mode. The real
                safetensors loading path is stubbed until P18-D1.
        """
        # Read the model_id input. In mock mode this is a
        # placeholder string; in real mode it references a
        # text-encoder file path registered in the model store.
        model_id = inputs.get("model_id", "")

        # Read the optional clip_type hint.  Defaults to "qwen3"
        # when not provided — this is the tokeniser type used by
        # the Phase 018 baseline Z-Image Turbo FP8 model.
        clip_type = inputs.get("clip_type", "qwen3")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # carrying the resolved clip_type instead of loading a
            # real text-encoder pipeline. This keeps tests fast
            # and avoids requiring GPU hardware or torch.
            return {"clip": MockClip(clip_type=clip_type)}

        # Real mode: load actual safetensors weights for the text
        # encoder. This path is stubbed — the real implementation
        # will use safetensors.safe_open() to read the weight
        # tensors and load via pipeline_cache.get_or_load(). The
        # pipeline_cache module is implemented in task P18-D1.
        # TODO(P18-A3): Implement real safetensors loading path.
        raise NotImplementedError(
            "Real LoadClip path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )
