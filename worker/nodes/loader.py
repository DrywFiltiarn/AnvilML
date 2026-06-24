"""LoadModel, LoadVae, and LoadClip nodes — load diffusion components from safetensors.

This module defines three loader nodes:

* ``LoadModel`` — loads a diffusion transformer (UNet / DiT) from a
  safetensors file via ``from_single_file()``.
* ``LoadVae`` — loads a VAE from a standalone safetensors file via
  ``from_single_file()``.
* ``LoadClip`` — loads a text encoder (CLIP / T5 / Qwen3) from a
  safetensors file via the architecture-dispatcher in
  ``worker.nodes.arch.clip``.

Each node accepts a ``model_id`` STRING input and outputs a typed slot
(MODEL, VAE, or CLIP) containing either a real loaded pipeline
component (in non-mock mode) or a lightweight sentinel (in mock mode).

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

from worker.nodes.arch import clip as arch_clip
from worker.nodes.arch import diffusion as arch_diffusion
from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = [
    "LoadModel",
    "LoadVae",
    "RealClip",
    "MockModel",
    "MockVae",
    "MockClip",
    "MockTokenizer",
    "MockTextEncoder",
]


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


class MockTokenizer:
    """Sentinel tokenizer object for mock mode.

    A lightweight placeholder that stands in for a real transformers
    tokenizer instance during testing. Real tokenizer objects produced
    by the safetensors loading path will have their own structure
    defined when ``qwen3.py`` is implemented (P18-D9).
    """
    pass


class MockTextEncoder:
    """Sentinel text-encoder object for mock mode.

    A lightweight placeholder that stands in for a real transformers
    text-encoder model during testing. Real text-encoder objects
    produced by the safetensors loading path will have their own
    structure defined when ``qwen3.py`` is implemented (P18-D9).
    """
    pass


class RealClip:
    """Lightweight wrapper around a transformers text-encoder pipeline.

    The real transformers text-encoder (tokenizer + text encoder model)
    carries config data (e.g. ``pad_token_id``, ``max_position_embeddings``)
    that downstream consumers like ``ClipTextEncode`` need to read, but
    the raw transformers objects do not expose a unified interface.
    This wrapper stores private refs and exposes public ``.tokenizer``
    and ``.text_encoder`` attributes, mirroring the ``RealModel`` pattern
    used for diffusion transformer components.

    Args:
        tokenizer: A transformers tokenizer instance (e.g.
            ``Qwen2Tokenizer``, ``CLIPTokenizer``, ``T5Tokenizer``).
        text_encoder: A transformers text-encoder model instance (e.g.
            ``Qwen3ForCausalLM``, ``CLIPTextModelWithProjection``,
            ``T5ForConditionalGeneration``).
    """

    def __init__(
        self,
        tokenizer: Any,
        text_encoder: Any,
        device: str = "cpu",
    ) -> None:
        """Initialise the real clip wrapper.

        Args:
            tokenizer: The transformers tokenizer component.
            text_encoder: The transformers text-encoder model.
            device: Target device string for tensor placement
                (e.g. ``"cuda:0"``, ``"cpu"``). Defaults to ``"cpu"``
                for backward compatibility with mock mode which does
                not pass this argument.
        """
        self._tokenizer = tokenizer
        self._text_encoder = text_encoder
        self._device = device

    @property
    def tokenizer(self) -> Any:
        """The transformers tokenizer instance."""
        return self._tokenizer

    @property
    def text_encoder(self) -> Any:
        """The transformers text-encoder model instance."""
        return self._text_encoder

    def encode(
        self, text: str, negative_text: str = ""
    ) -> tuple[list[Any], list[Any]]:
        """Encode text and negative text into embedding lists.

        Applies the chat template (for Qwen3-style tokenisers), tokenises
        the resulting string, runs it through the text encoder, extracts
        ``hidden_states[-2]``, and filters by attention mask. Returns
        two lists of tensors: one for the positive prompt and one for
        the negative prompt.

        In mock mode (``ANVILML_WORKER_MOCK=1``) returns empty lists
        without importing ``torch``.

        Args:
            text: The positive prompt string to encode.
            negative_text: The negative prompt string. Defaults to
                ``""`` — an empty string produces a zero-length embedding
                sequence that the downstream pipeline interprets as a
                "no-op" negative condition.

        Returns:
            A tuple of ``(positive_embeds, negative_embeds)`` where each
            element is a ``list[torch.FloatTensor]`` — one tensor per
            hidden state layer that survived attention-mask filtering.

        Raises:
            Exception: Propagates errors from the tokenizer or text
                encoder (e.g. ``OSError`` for missing files,
                ``RuntimeError`` for shape mismatches).
        """
        # Check mock mode — return empty lists without importing
        # torch when running in CI/test environments.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            return ([], [])

        # Real mode: lazy-import torch and transformers here so
        # the module remains importable in mock mode.
        import torch

        # Encode the positive prompt.
        # Qwen3 tokenisers use apply_chat_template to prepend the
        # correct system prompt and generation tokens — this is
        # mandatory because the text encoder was trained with these
        # tokens and will produce garbage embeddings without them.
        # enable_thinking=True matches the training configuration of
        # Qwen3-4B which was trained with explicit thinking tags.
        templated_text = self._tokenizer.apply_chat_template(
            messages=[{"role": "user", "content": text}],
            tokenize=False,
            add_generation_prompt=True,
            enable_thinking=True,
        )

        # Tokenise the templated text with fixed-length padding.
        # max_length=512 is the standard context window for text
        # encoders in diffusion pipelines — longer prompts are
        # truncated, shorter ones are padded to a uniform length.
        text_inputs = self._tokenizer(
            templated_text,
            padding="max_length",
            max_length=512,
            truncation=True,
            return_tensors="pt",
        )

        # Move inputs to the target device.
        # This ensures the text encoder (which lives on GPU in
        # production) receives tensors on the correct device.
        text_input_ids = text_inputs.input_ids.to(self._device)
        prompt_masks = text_inputs.attention_mask.to(self._device).bool()

        # Run through the text encoder with hidden state extraction.
        # output_hidden_states=True returns all layer outputs; we
        # take hidden_states[-2] (penultimate layer) which is the
        # standard practice for CLIP-like text encoding — the last
        # layer tends to overfit to the LM head and loses semantic
        # information needed for cross-modal alignment.
        hidden = self._text_encoder(
            input_ids=text_input_ids,
            attention_mask=prompt_masks,
            output_hidden_states=True,
        ).hidden_states[-2]

        # Filter out padding tokens using the attention mask.
        # Only tokens with attention_mask==1 contain real text;
        # masked tokens are padding and should not contribute to
        # the conditioning signal.
        positive_embeds = [
            hidden[i][prompt_masks[i]] for i in range(len(hidden))
        ]

        # Dual-conditioning: ZImagePipeline uses classifier-free
        # guidance (always enabled), so both positive and negative
        # embeddings are required. The negative embeds are produced
        # by encoding the negative_text string through the same
        # text encoder pipeline.
        if negative_text:
            neg_templated = self._tokenizer.apply_chat_template(
                messages=[{"role": "user", "content": negative_text}],
                tokenize=False,
                add_generation_prompt=True,
                enable_thinking=True,
            )

            neg_inputs = self._tokenizer(
                neg_templated,
                padding="max_length",
                max_length=512,
                truncation=True,
                return_tensors="pt",
            )

            neg_input_ids = neg_inputs.input_ids.to(self._device)
            neg_masks = neg_inputs.attention_mask.to(self._device).bool()

            neg_hidden = self._text_encoder(
                input_ids=neg_input_ids,
                attention_mask=neg_masks,
                output_hidden_states=True,
            ).hidden_states[-2]

            negative_embeds = [
                neg_hidden[i][neg_masks[i]] for i in range(len(neg_hidden))
            ]
        else:
            # Empty negative prompt: encode an empty string so the
            # negative embed has the same structure as the positive
            # embed. This is required for classifier-free guidance
            # which concatenates positive and negative along the
            # batch dimension.
            neg_inputs = self._tokenizer(
                "",
                padding="max_length",
                max_length=512,
                truncation=True,
                return_tensors="pt",
            )
            neg_input_ids = neg_inputs.input_ids.to(self._device)
            neg_masks = neg_inputs.attention_mask.to(self._device).bool()

            neg_hidden = self._text_encoder(
                input_ids=neg_input_ids,
                attention_mask=neg_masks,
                output_hidden_states=True,
            ).hidden_states[-2]

            negative_embeds = [
                neg_hidden[i][neg_masks[i]] for i in range(len(neg_hidden))
            ]

        return (positive_embeds, negative_embeds)


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
        single-file safetensors loading + pipeline_cache.

        Args:
            **inputs: Must contain ``"model_id"`` — the identifier
                of the model to load (path to a ``.safetensors`` file
                or a directory containing one).

        Returns:
            Dict with key ``"model"`` containing either a ``MockModel``
            (mock mode) or a ``RealModel`` wrapping a loaded transformer
            (real mode).

        Raises:
            OSError: If the model file or directory does not exist.
            ValueError: If the safetensors file is malformed or
                missing required keys.
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

        # Real mode: load actual safetensors weights via single-file
        # loading. The _load_model_from_safetensors helper handles
        # arch detection and dispatches to the correct arch module
        # for the actual diffusers loading.
        # Note: self.ctx.pipeline_cache is typed as dict[str, Any] in
        # NodeContext but a PipelineCache instance at runtime
        # (retrofitted by P903-A2), so .get_or_load() is available.
        # Capture self.ctx.device before passing into the lambda closure
        # so the transformer is placed on the worker's assigned device
        # (e.g. "cuda:0") instead of silently defaulting to CPU.
        result = self.ctx.pipeline_cache.get_or_load(
            model_id,
            "fp8",
            lambda: _load_model_from_safetensors(model_id, model_id, self.ctx.device),
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

        # Real mode: load actual VAE weights via the arch dispatch system.
        # The _load_vae_from_safetensors wrapper dispatches to the correct
        # arch module's load_vae() function (e.g. zit.py) which handles
        # the diffusers loading internally. The arch parameter defaults to
        # "zit" since that is the only architecture supported right now.
        # Note: self.ctx.pipeline_cache is typed as dict[str, Any] in
        # NodeContext but a PipelineCache instance at runtime
        # (retrofitted by P903-A2), so .get_or_load() is available.
        # Capture self.ctx.device before passing into the lambda closure
        # so the VAE is placed on the worker's assigned device (e.g.
        # "cuda:0") instead of silently defaulting to CPU.
        result = self.ctx.pipeline_cache.get_or_load(
            model_id,
            "bf16",
            lambda: _load_vae_from_safetensors(model_id, "zit", self.ctx.device),
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
            OSError: If the model file or directory does not exist.
            ValueError: If no architecture module claims the specified
                clip_type (e.g. unknown tokeniser type).
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

        # Real mode: dispatch to the correct architecture module via the
        # clip registry, wrapped by _load_clip_from_safetensors.
        # The wrapper handles the arch_clip.get_module() lookup and
        # module.load() call internally, passing self.ctx.device
        # explicitly since the wrapper is a module-level function.
        return _load_clip_from_safetensors(
            model_id, clip_type, self.ctx.device
        )


def _detect_arch_from_keys(checkpoint: dict[str, Any]) -> str | None:
    """Detect architecture from raw checkpoint key prefixes.

    Inspects the keys in a raw state dict for architecture-specific
    patterns (the ComfyUI approach). Returns the first matching
    architecture name, or ``None`` if no known pattern is found.

    Currently supported:

    * ``"zit"`` — keys starting with ``model.diffusion_model.``
      (Z-Image Turbo FP8 checkpoints).

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file,
            with keys in the original ComfyUI/export format
            (e.g. ``"model.diffusion_model.layers.0.attention.qkv.weight"``).

    Returns:
        The detected architecture string (e.g. ``"zit"``), or ``None``
        if no known key pattern is present.

    .. versionadded:: 0.1.0
    """
    # Scan keys for architecture-specific prefixes.
    # The order matters: more specific patterns first, general ones later.
    # Currently only ZiT has a dedicated prefix; future architectures
    # (Flux, etc.) add their own patterns here.
    #
    # ZiT checkpoints use "model.diffusion_model." as the top-level
    # prefix for all transformer weights. This is the canonical
    # ComfyUI detection signal for Z-Image Turbo models.
    has_diffusion_model_prefix = any(
        key.startswith("model.diffusion_model.") for key in checkpoint
    )

    if has_diffusion_model_prefix:
        return "zit"

    # No known architecture pattern found.
    return None


def _load_model_from_safetensors(
    model_id: str, arch: str, device: str = "cpu"
) -> RealModel:
    """Load a diffusion transformer from a single safetensors file.

    This is the active loading path for ``LoadModel`` in real mode.
    It inspects raw checkpoint key prefixes for architecture detection
    (primary), falls back to safetensors metadata (secondary), then
    to the path-derived *arch* parameter (tertiary), normalises the
    arch string from a path to a bare name, and dispatches to the
    correct arch module's ``load_transformer()`` function before
    moving the result to the target *device*.

    Args:
        model_id: Path to the safetensors file or directory
            containing the model weights.
        arch: Architecture identifier (e.g. ``"zit"``). Used as a
            fallback when both key-prefix and metadata detection fail.
        device: Target device string for tensor placement
            (e.g. ``"cuda:0"``, ``"cpu"``). Defaults to ``"cpu"``
            for backward compatibility with callers that do not
            pass a device argument.

    Returns:
        A ``RealModel`` wrapping the loaded transformer and arch.

    Raises:
        OSError: If the model file or directory does not exist.
        ValueError: If the safetensors file is malformed or the
            detected architecture has no matching arch module.
    """
    # Load the safetensors file for both metadata and key inspection.
    # We need the raw state dict keys for architecture detection
    # (key-prefix pattern matching, the ComfyUI approach) and the
    # metadata for backward-compatible checkpoint detection.
    # safetensors.load_file() returns the full state dict;
    # safe_open().metadata gives us the embedded metadata dict.
    from safetensors.torch import load_file as safetensors_load_file
    from safetensors.torch import safe_open

    raw_checkpoint = safetensors_load_file(model_id)

    # Primary detection: inspect raw checkpoint key prefixes.
    # This is the ComfyUI pattern — check for architecture-specific
    # key prefixes (e.g. "model.diffusion_model." for ZiT) before
    # any key stripping. This works for checkpoints that don't carry
    # export-tool metadata, which is the scaling case this task fixes.
    detected_arch = _detect_arch_from_keys(raw_checkpoint)

    # Open for metadata as fallback.
    with safe_open(model_id, framework="pt") as st:
        # Fallback 1: if key-prefix detection found nothing, try metadata.
        # Some checkpoints carry an "arch" key in their safetensors metadata
        # (written by the export tool). This is a reliable signal when present.
        if detected_arch is None:
            metadata = st.metadata
            detected_arch = (metadata.get("arch") if metadata else None)

        # Fallback 2: if both failed, use the arch parameter (path-derived).
        # This handles the common case where model_id is a directory path
        # like "/models/zit-fp8/unet" — we take the last component.
        if detected_arch is None:
            detected_arch = arch

    # If the arch string still looks like a path (contains "/" or
    # "\\"), extract the directory name as the architecture hint.
    # This handles the common case where model_id is a directory
    # path like "/models/zit-fp8/unet" — we take the last component.
    # The "models/" directory naming convention uses the directory
    # name as the architecture identifier when metadata is absent.
    if "/" in detected_arch or "\\" in detected_arch:
        detected_arch = detected_arch.split("/")[-1].split("\\")[-1]

    # Dispatch to the correct arch module's load_transformer() function.
    # get_module_by_name constructs a shim object with arch=detected_arch
    # and delegates to get_module() which iterates over loaded arch
    # modules' can_handle() functions. Only one arch module (zit) is
    # registered at this time.
    module = arch_diffusion.get_module_by_name(detected_arch)
    if module is None:
        # No arch module claims this architecture — raise a clear error
        # so the operator knows which architectures are supported.
        raise ValueError(f"unsupported architecture: {detected_arch!r}")

    # Load the transformer through the arch module's load_transformer()
    # function. This handles all diffusers loading internally, including
    # the correct torch_dtype and key remapping. The arch module's
    # load_transformer() performs zero network calls — all weights are
    # loaded from the local .safetensors file.
    transformer = module.load_transformer(model_id)

    # Move the transformer to the target device.
    # .to() may return a new object reference depending on the
    # diffusers version, so we always assign the return value
    # rather than assuming in-place mutation.
    transformer = transformer.to(device)

    return RealModel(transformer, arch=detected_arch)


def _load_vae_from_safetensors(model_id: str, arch: str, device: str) -> Any:
    """Load a VAE from a single safetensors file via the arch dispatch system.

    Dispatches to the correct arch module's ``load_vae()`` function
    (e.g. ``zit.py``), which handles the diffusers loading internally.
    The result is then moved to the target device.

    Only ZiT-compatible VAE checkpoints are supported — the arch
    module's ``load_vae()`` constructs an ``AutoencoderKL`` with
    ZiT-specific ``block_out_channels`` config.

    Args:
        model_id: Path to the safetensors file containing VAE weights.
        arch: Architecture identifier for dispatch (e.g. ``"zit"``).
        device: Target device string for tensor placement
            (e.g. ``"cuda:0"``, ``"cpu"``).

    Returns:
        A loaded VAE model instance on the target device.

    Raises:
        OSError: If the model file or directory does not exist.
        ValueError: If the architecture has no matching arch module.
    """
    # Dispatch to the correct arch module's load_vae() function.
    # get_module_by_name constructs a shim object with arch=arch
    # and delegates to get_module() which iterates over loaded arch
    # modules' can_handle() functions.
    module = arch_diffusion.get_module_by_name(arch)
    if module is None:
        # No arch module claims this architecture — raise a clear error.
        raise ValueError(f"unsupported architecture: {arch!r}")

    # Load the VAE through the arch module's load_vae() function.
    # This handles all diffusers loading internally, including the
    # correct checkpoint remapping. The arch module's load_vae()
    # performs zero network calls — all weights are loaded locally.
    vae = module.load_vae(model_id)

    # Move the VAE to the target device.
    # .to() may return a new object reference depending on the
    # diffusers version, so we always assign the return value
    # rather than assuming in-place mutation.
    return vae.to(device)


def _load_clip_from_safetensors(
    model_id: str, clip_type: str, device: str
) -> Any:
    """Load a text encoder (CLIP/T5/Qwen3) from a safetensors file.

    Dispatches to the correct architecture module via the clip
    registry, then delegates to the matched module's ``load()``
    function with ``torch.bfloat16`` precision and the target device.

    Args:
        model_id: Path to the safetensors file containing the
            text encoder weights.
        clip_type: The tokeniser type identifier (e.g. ``"qwen3"``,
            ``"clip_l"``, ``"t5"``).
        device: Target device string for tensor placement
            (e.g. ``"cuda:0"``, ``"cpu"``).

    Returns:
        A loaded text-encoder object (e.g. ``RealClip``).

    Raises:
        ValueError: If no arch module claims the specified clip_type.
    """
    # Dispatch to the correct architecture module via the clip
    # registry. The arch_clip.get_module() function iterates over
    # all loaded arch modules and returns the one whose can_handle()
    # matches the clip_type string. This mirrors the Sampler node's
    # arch.get_module(model) pattern for diffusion dispatch.
    module = arch_clip.get_module(clip_type)
    if module is None:
        # No arch module claims this clip_type — raise a clear error
        # so the operator knows which values are valid.
        raise ValueError(f"unsupported clip_type: {clip_type!r}")

    # Delegate to the matched module's load() function with the
    # bfloat16 dtype and the target device. This is the standard
    # precision for text encoders in diffusion pipelines. The
    # module's load() handles mock mode internally, returning a
    # RealClip sentinel when ANVILML_WORKER_MOCK=1.
    import torch

    return module.load(
        model_id, torch_dtype=torch.bfloat16, device=device
    )

