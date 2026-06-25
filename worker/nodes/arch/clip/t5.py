"""T5-XXL text encoder architecture dispatch module.

This module provides architecture-specific dispatch for T5-XXL text encoders,
including model detection via ``can_handle()`` and a load entry point via
``load()``.

In mock mode (``ANVILML_WORKER_MOCK=1``), the ``load()`` function returns a
lightweight ``RealClip(MockTokenizer(), MockTextEncoder())`` sentinel
immediately without importing torch, transformers, or safetensors. The real
loading path constructs a ``T5EncoderModel`` from verbatim config values
sourced from ``google/t5-v1_1-xxl``'s ``config.json``.

The ``torch``, ``transformers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Any real-mode imports must be inside the ``if not _mock:`` guard.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any

__all__ = ["can_handle", "load"]


def can_handle(clip_type: str) -> bool:
    """Check whether the given clip type is a T5 text encoder.

    Performs a simple string comparison against the canonical T5
    clip type identifier.

    Args:
        clip_type: The clip type string to check (e.g. ``"t5"``,
            ``"qwen3"``, ``"clip_l"``).

    Returns:
        ``True`` if ``clip_type == "t5"``, ``False`` otherwise.
    """
    # Match against the T5 clip type string. This is the canonical
    # identifier for T5-based text encoders that use T5TokenizerFast
    # and T5EncoderModel from the transformers library.
    return clip_type == "t5"


def load(
    model_id: str,
    torch_dtype: Any,
    device: str = "cpu",
) -> RealClip:  # noqa: F821
    """Load a T5-XXL text encoder from a safetensors file.

    In mock mode, returns a lightweight ``RealClip`` wrapping sentinel
    objects without importing torch, transformers, or safetensors.
    In real mode, constructs a ``T5EncoderModel`` from verbatim
    config values and loads weights from the provided model path.

    Args:
        model_id: Path to the model directory or safetensors file
            containing the T5-XXL text encoder weights.
        torch_dtype: PyTorch dtype for the model (e.g. ``torch.bfloat16``).
            Used in real mode to cast loaded weights.
        device: Target device string for tensor placement
            (e.g. ``"cuda:0"``, ``"cpu"``). Defaults to ``"cpu"``
            for backward compatibility with existing callers.

    Returns:
        A ``RealClip`` instance with ``.tokenizer`` and ``.text_encoder``
        attributes. In mock mode these are ``MockTokenizer`` and
        ``MockTextEncoder`` sentinels.

    Raises:
        OSError: If the model path does not exist or is inaccessible
            (real mode only).
        RuntimeError: If the safetensors file cannot be parsed
            (real mode only).
    """
    # Check mock mode by inspecting the environment variable.
    # This must be a runtime check (not a module-level import)
    # so that CI tests running with ANVILML_WORKER_MOCK=1
    # never touch torch/transformers/safetensors at import time.
    _mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

    if _mock:
        # In mock mode, return a lightweight sentinel object
        # immediately without importing torch, transformers, or
        # safetensors. This keeps tests fast and avoids requiring
        # GPU hardware or these heavy dependencies.
        from worker.nodes.loader import MockTokenizer, MockTextEncoder, RealClip

        return RealClip(MockTokenizer(), MockTextEncoder(), device=device)

    # Real mode: construct a T5-XXL text encoder from config values
    # and load weights from a safetensors file.
    # Lazy imports — these packages are not available in mock mode
    # (no torch installed), so importing them here keeps the worker
    # importable when ANVILML_WORKER_MOCK=1.
    from pathlib import Path

    import torch
    from safetensors.torch import load_file as safetensors_load_file
    from transformers import T5Config, T5EncoderModel, T5TokenizerFast
    from worker.nodes.loader import RealClip

    # Resolve the tokenizer directory relative to this module.
    # The tokenizer assets live in worker/assets/t5_tokenizer/
    # (four levels up from this file's parent, then into assets).
    # Note: the plan originally specified parent.parent.parent, but the
    # actual asset layout places tokenizers at worker/assets/
    # (one level higher than parent.parent.parent would resolve).
    tokenizer_dir = Path(__file__).parent.parent.parent.parent / "assets" / "t5_tokenizer"

    # Load the T5TokenizerFast from the bundled asset directory.
    # T5-XXL uses the standard T5 tokenizer — this is the tokenizer
    # used by google/t5-v1_1-xxl on HuggingFace.
    tokenizer = T5TokenizerFast.from_pretrained(tokenizer_dir)

    # Verbatim config values from google/t5-v1_1-xxl's config.json on
    # HuggingFace. These must not be replaced with T5Config defaults
    # because the config defaults may differ from the actual model's
    # training configuration.
    config_values = {
        "vocab_size": 32128,
        "d_model": 4096,
        "d_kv": 64,
        "d_ff": 10240,
        "num_layers": 24,
        "num_heads": 64,
        "relative_attention_num_buckets": 32,
        "feed_forward_proj": "gated-gelu",
        "tie_word_embeddings": False,
    }

    # Construct the model from config and load weights from the
    # safetensors file. T5EncoderModel is the encoder-only variant
    # of T5 — we use this instead of T5ForConditionalGeneration
    # because CLIP-like text encoding only needs the encoder path.
    #
    # Memory-safe construction: build on torch.device("meta") with the
    # default dtype temporarily set to bfloat16, so to_empty() materializes
    # real storage directly at bf16 — never at fp32 first. T5-XXL is
    # ~4.8B parameters (~18GB fp32, ~9GB bf16) — constructing this directly
    # or casting fp32 -> bf16 after the fact both transiently require far
    # more memory than the final bf16 model needs and have been confirmed
    # to OOM a memory-constrained machine. assign=True bypasses dtype
    # coercion, so the checkpoint's tensors are cast to bf16 first.
    original_default_dtype = torch.get_default_dtype()
    torch.set_default_dtype(torch.bfloat16)
    try:
        with torch.device("meta"):
            model = T5EncoderModel(T5Config(**config_values))
    finally:
        torch.set_default_dtype(original_default_dtype)
    model = model.to_empty(device="cpu")

    checkpoint = safetensors_load_file(model_id)
    checkpoint_bf16 = {k: v.to(torch.bfloat16) for k, v in checkpoint.items()}
    model.load_state_dict(checkpoint_bf16, assign=True)

    # Move the model to the target device.
    # .to() returns a new reference for some module types, so we
    # must assign the return value — failing to do so leaves the
    # original CPU model in place. This is the established PyTorch
    # pattern for device placement.
    model = model.to(device)

    return RealClip(tokenizer, model, device=device)
