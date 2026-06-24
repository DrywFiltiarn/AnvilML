"""Qwen3 text encoder architecture dispatch module.

This module provides architecture-specific dispatch for Qwen3 text encoders,
including model detection via ``can_handle()`` and a load entry point via
``load()``.

In mock mode (``ANVILML_WORKER_MOCK=1``), the ``load()`` function returns a
lightweight ``RealClip(MockTokenizer(), MockTextEncoder())`` sentinel
immediately without importing torch, transformers, or safetensors. The real
loading path constructs a ``Qwen3ForCausalLM`` from verbatim config values
sourced from ``Qwen/Qwen3-4B``'s ``config.json``.

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
    """Check whether the given clip type is a Qwen3 text encoder.

    Performs a simple string comparison against the canonical Qwen3
    clip type identifier.

    Args:
        clip_type: The clip type string to check (e.g. ``"qwen3"``,
            ``"clip_l"``, ``"t5"``).

    Returns:
        ``True`` if ``clip_type == "qwen3"``, ``False`` otherwise.
    """
    # Match against the Qwen3 clip type string. This is the canonical
    # identifier for Qwen3-based text encoders that use Qwen2Tokenizer
    # and Qwen3ForCausalLM from the transformers library.
    return clip_type == "qwen3"


def load(
    model_id: str,
    torch_dtype: Any,
    device: str = "cpu",
) -> RealClip:  # noqa: F821
    """Load a Qwen3 text encoder from a safetensors file.

    In mock mode, returns a lightweight ``RealClip`` wrapping sentinel
    objects without importing torch, transformers, or safetensors.
    In real mode, constructs a ``Qwen3ForCausalLM`` from verbatim
    config values and loads weights from the provided model path.

    Args:
        model_id: Path to the model directory or safetensors file
            containing the Qwen3 text encoder weights.
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

    # Real mode: construct a Qwen3 text encoder from config values
    # and load weights from a safetensors file.
    # Lazy imports — these packages are not available in mock mode
    # (no torch installed), so importing them here keeps the worker
    # importable when ANVILML_WORKER_MOCK=1.
    from pathlib import Path

    from safetensors.torch import load_file as safetensors_load_file
    from transformers import Qwen2Tokenizer, Qwen3Config, Qwen3ForCausalLM
    from worker.nodes.loader import RealClip

    # Resolve the tokenizer directory relative to this module.
    # The tokenizer assets live in worker/assets/qwen25_tokenizer/
    # (four levels up from this file's parent, then into assets).
    # Note: the plan originally specified parent.parent.parent, but the
    # actual asset layout places tokenizers at worker/assets/
    # (one level higher than parent.parent.parent would resolve).
    tokenizer_dir = Path(__file__).parent.parent.parent.parent / "assets" / "qwen25_tokenizer"

    # Load the Qwen2 tokenizer from the bundled asset directory.
    # Qwen3 models use the Qwen2 tokenizer family — this is a shared
    # tokenizer used across both Qwen2 and Qwen3 model families.
    tokenizer = Qwen2Tokenizer.from_pretrained(tokenizer_dir)

    # Verbatim config values from Qwen/Qwen3-4B's config.json on
    # HuggingFace. These must not be replaced with Qwen3Config
    # defaults because the config defaults may differ from the
    # actual model's training configuration.
    config_values = {
        "vocab_size": 151936,
        "hidden_size": 2560,
        "intermediate_size": 9728,
        "num_hidden_layers": 36,
        "num_attention_heads": 32,
        "num_key_value_heads": 8,
        "head_dim": 128,
        "max_position_embeddings": 40960,
        "tie_word_embeddings": True,
    }

    # Construct the model from config and load weights from the
    # safetensors file. Qwen3ForCausalLM is used as the text
    # encoder — the causal LM head provides contextual embeddings
    # needed for CLIP-like text encoding.
    model = Qwen3ForCausalLM(Qwen3Config(**config_values))
    model.load_state_dict(safetensors_load_file(model_id))

    # Move the model to the target device.
    # .to() returns a new reference for some module types, so we
    # must assign the return value — failing to do so leaves the
    # original CPU model in place. This is the established PyTorch
    # pattern for device placement.
    model = model.to(device)

    return RealClip(tokenizer, model, device=device)
