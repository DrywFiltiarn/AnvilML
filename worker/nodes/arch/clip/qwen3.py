"""Qwen3 CLIP text-encoder architecture shape inference.

This module implements step 1 of the four-step loading contract
(ANVILML_DESIGN.md §11.3) for Qwen3 CLIP text-encoders: it opens a
safetensors checkpoint header-only, reads every tensor key's shape,
and infers the architecture hyperparameters (hidden dimension, layer
count, intermediate size, vocab size, architecture string, and native
dtype) from the Qwen3 tensor key naming convention.

The Qwen3 text encoder follows the standard transformer key naming
convention used by Qwen/Qwen3 models:

- ``model.embed_tokens.weight`` — vocab_size × hidden_dim embedding table
- ``model.layers.N.self_attn.{q,k,v,o}_proj.weight`` — per-layer
  attention projections (hidden_dim × hidden_dim)
- ``model.layers.N.mlp.{gate,up,down}_proj.weight`` — per-layer MLP
  projections (intermediate_size × hidden_dim for gate/up,
  hidden_dim × intermediate_size for down)
- ``model.layers.N.{input,post_attention}_layernorm.weight`` — per-layer
  layer norm scale vectors (hidden_dim,)
- ``model.norm.weight`` — final normalization (hidden_dim,)

Design: ANVILML_DESIGN.md §11.3 — the four-step loading contract,
step 1 (shape inference) for the Qwen3 CLIP text-encoder architecture.
"""

from __future__ import annotations

import logging
import re
from typing import Any

from safetensors import safe_open

logger = logging.getLogger(__name__)

# Canonical architecture identifier — the string that the dispatcher
# passes to can_handle() when routing CLIP model requests.
# Mirrors the "arch": "qwen3" value returned by _infer_hyperparams()
# when it reads metadata or falls back to key-pattern inference.
ARCH: str = "qwen3"


def _infer_hyperparams(path: str) -> dict[str, Any]:
    """Infer architecture hyperparameters from a Qwen3 safetensors checkpoint header.

    Opens the file header-only (no tensor data is loaded), reads every key's
    shape, and returns a dictionary of inferred hyperparameters. This function
    implements step 1 of the four-step loading contract
    (ANVILML_DESIGN.md §11.3).

    **P904 regression prevention:** this function reads ALL keys via
    ``f.keys()`` without any truncation or slicing — the P904 bug used
    ``list(f.keys())[:30]`` which silently dropped keys beyond index 30,
    causing incorrect layer counts for models with 12+ layers.

    Args:
        path: Filesystem path to a Qwen3-format safetensors checkpoint file.

    Returns:
        A dict with the following keys:
        - ``hidden_dim`` (int): The transformer hidden dimension.
        - ``num_hidden_layers`` (int): Number of transformer layers.
        - ``intermediate_size`` (int): MLP intermediate projection size.
        - ``vocab_size`` (int): Vocabulary embedding table size.
        - ``arch`` (str): Architecture string (e.g. ``"qwen3"``).
        - ``native_dtype`` (str): Canonical native dtype string (e.g. ``"fp32"``,
            ``"bf16"``, ``"fp8"``) inferred from the first weight tensor
            in the checkpoint header.

    Raises:
        ValueError: If the file is not a valid safetensors file, is truncated,
            or does not contain the expected Qwen3 key patterns.
    """
    # Open the file header-only — no tensor data is loaded into memory.
    # This is the critical safety guarantee: even multi-GB checkpoints
    # only load the ~100KB metadata header.
    # framework="np" (not "pt"): _infer_hyperparams_inner() only ever reads
    # .keys(), .get_slice(key).get_shape(), and .get_slice(key).get_dtype() —
    # it never calls .get_tensor() and never touches actual tensor data, so
    # there is no reason to request the torch framework here. Requesting
    # framework="pt" made safetensors require torch to even open the header,
    # which broke this function in mock-mode (no torch installed) despite it
    # being documented as one of the torch-free contract functions
    # (ANVILML_DESIGN.md §11.2) — this was a genuine defect, not a mock-mode
    # workaround (P900-series retrofit).
    # Wrap in try/except to convert platform-specific errors (FileNotFoundError,
    # SafetensorError for corrupted headers) into ValueError with a descriptive
    # message, providing a uniform error interface for callers.
    try:
        with safe_open(path, framework="np") as f:
            return _infer_hyperparams_inner(f, path)
    except (FileNotFoundError, OSError) as exc:
        raise ValueError(f"cannot open safetensors file: {exc}") from exc
    except Exception as exc:
        # Catch-all for SafetensorError and any other deserialization
        # failures (truncated headers, corrupted files, etc.).
        raise ValueError(f"cannot parse safetensors header: {exc}") from exc


def _infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]:
    """Inner logic for _infer_hyperparams — runs inside the safe_open context.

    This helper is factored out so the try/except wrapping in the public
    function cleanly catches exceptions from safe_open itself without
    needing to re-raise ValueError from inside the with block.

    Args:
        f: A safetensors safe_open handle (already opened, header-only).
        path: Original filesystem path (for error messages).

    Returns:
        A dict of inferred hyperparameters including ``native_dtype``.

    Raises:
        ValueError: If the checkpoint does not contain the expected Qwen3
            key patterns or required keys.
    """
    # Read ALL keys without truncation — P904 regression prevention.
    # The P904 bug used list(f.keys())[:30] which silently dropped keys
    # beyond index 30, causing incorrect layer counts for models with
    # 12+ layers. We read every key, unconditionally.
    keys = f.keys()

    # ------------------------------------------------------------------
    # 0. Detect native dtype from the first weight tensor.
    # ------------------------------------------------------------------
    # The safetensors header stores each tensor's dtype as a string
    # (e.g. "F32", "F16", "BF16", "F8_E4M3", "I32"). We iterate over
    # all keys and collect the dtype from the first weight tensor
    # (identified by the ".weight" suffix) to determine the checkpoint's
    # native precision. This is needed by _select_dtype() to decide
    # whether FP8 is viable (caps.fp8 AND native_dtype == fp8).
    # If no weight tensor is found, default to fp32 — the conservative
    # safe choice that prevents fp8 selection on unknown checkpoints.
    native_dtype: str = "fp32"
    for key in keys:
        if key.endswith(".weight"):
            # get_dtype() returns the safetensors dtype string
            # (e.g. "F32", "BF16", "F8_E4M3") for the first weight tensor.
            safetensors_dtype: str = f.get_slice(key).get_dtype()
            # Map the safetensors dtype string to a canonical lowercase
            # form that downstream code uses for comparison.
            native_dtype = _safetensors_dtype_to_canonical(safetensors_dtype)
            break

    # ------------------------------------------------------------------
    # 1. Infer hidden_dim from attention projection keys.
    # ------------------------------------------------------------------
    # hidden_dim is the first dimension of self_attn.{q,k,v,o}_proj.weight
    # tensors. We check each key using endswith() to find the first
    # matching attention projection and extract shape[0].
    hidden_dim: int | None = None
    for key in keys:
        if key.endswith((".self_attn.q_proj.weight",
                          ".self_attn.k_proj.weight",
                          ".self_attn.v_proj.weight",
                          ".self_attn.o_proj.weight")):
            hidden_dim = f.get_slice(key).get_shape()[0]
            break

    if hidden_dim is None:
        raise ValueError(
            f"cannot infer hidden_dim from safetensors keys in {path}: "
            "no recognized attention projection keys "
            "(self_attn.q_proj.weight, self_attn.k_proj.weight, "
            "self_attn.v_proj.weight, or self_attn.o_proj.weight) found"
        )

    # ------------------------------------------------------------------
    # 2. Count num_hidden_layers from layer indices.
    # ------------------------------------------------------------------
    # Extract numeric suffixes from model.layers.N.* keys to get
    # 0-indexed layer indices, then count = max_index + 1.
    layer_indices: set[int] = set()
    for key in keys:
        m = re.search(r"model\.layers\.(\d+)", key)
        if m:
            layer_indices.add(int(m.group(1)))

    if not layer_indices:
        raise ValueError(
            f"cannot infer num_hidden_layers from {path}: "
            "no model.layers.N.* keys found"
        )

    num_hidden_layers: int = max(layer_indices) + 1

    # ------------------------------------------------------------------
    # 3. Infer intermediate_size from MLP projection keys.
    # ------------------------------------------------------------------
    # intermediate_size is the first dimension of mlp.gate_proj.weight
    # and mlp.up_proj.weight tensors (intermediate_size × hidden_dim).
    # We check both patterns for robustness.
    intermediate_size: int | None = None
    for key in keys:
        if key.endswith((".mlp.gate_proj.weight", ".mlp.up_proj.weight")):
            intermediate_size = f.get_slice(key).get_shape()[0]
            break

    if intermediate_size is None:
        raise ValueError(
            f"cannot infer intermediate_size from {path}: "
            "no recognized MLP projection keys "
            "(mlp.gate_proj.weight or mlp.up_proj.weight) found"
        )

    # ------------------------------------------------------------------
    # 4. Infer vocab_size from embedding table.
    # ------------------------------------------------------------------
    # vocab_size is the first dimension of model.embed_tokens.weight
    # (vocab_size × hidden_dim).
    vocab_size: int | None = None
    for key in keys:
        if key.endswith(".embed_tokens.weight"):
            vocab_size = f.get_slice(key).get_shape()[0]
            break

    if vocab_size is None:
        raise ValueError(
            f"cannot infer vocab_size from {path}: "
            "no model.embed_tokens.weight key found"
        )

    # ------------------------------------------------------------------
    # 5. Detect architecture string.
    # ------------------------------------------------------------------
    # Primary path: check f.metadata() for an "arch" key. This is the
    # canonical source — the checkpoint author explicitly declared the
    # architecture name.
    meta = f.metadata()
    arch: str | None = meta.get("arch") if meta else None

    # Metadata-fallback path: when the "arch" key is absent from the
    # safetensors header, infer the architecture from key naming patterns.
    # Qwen3 family keys contain self_attn.*_proj or mlp.*_proj patterns
    # within model.layers.N.* paths.
    if arch is None:
        has_qwen3_patterns = False
        for key in keys:
            if any(
                pat in key
                for pat in (
                    "self_attn.q_proj",
                    "self_attn.k_proj",
                    "self_attn.v_proj",
                    "self_attn.o_proj",
                    ".mlp.gate_proj",
                    ".mlp.up_proj",
                    ".mlp.down_proj",
                )
            ):
                has_qwen3_patterns = True
                break

        if has_qwen3_patterns:
            arch = "qwen3"
        else:
            raise ValueError(
                f"unknown architecture in {path}: no arch metadata key "
                "and no recognizable Qwen3 key patterns found"
            )

    # Return all inferred hyperparameters as a single dict, including
    # the native_dtype so downstream dtype selection can use it.
    return {
        "hidden_dim": hidden_dim,
        "num_hidden_layers": num_hidden_layers,
        "intermediate_size": intermediate_size,
        "vocab_size": vocab_size,
        "arch": arch,
        "native_dtype": native_dtype,
    }


def _safetensors_dtype_to_canonical(safetensors_dtype: str) -> str:
    """Map a safetensors dtype string to a canonical lowercase form.

    Safetensors stores dtypes as uppercase abbreviations in the header
    (e.g. "F32", "BF16", "F8_E4M3"). This function normalizes them to
    lowercase canonical strings for comparison in dtype selection.

    Args:
        safetensors_dtype: A dtype string from safetensors tensor info,
            e.g. "F32", "F16", "BF16", "F8_E4M3", "F8_E5M2", "I32".

    Returns:
        A canonical lowercase string: "fp32", "fp16", "bf16", "fp8", etc.
        Unknown dtype strings fall through to "fp32" as a safe default.
    """
    # Map known safetensors dtype strings to their canonical forms.
    # The mapping covers all dtypes that Qwen3 checkpoints may use:
    # F32/F16/BF16 for standard precision, F8_E4M3/F8_E5M2 for FP8,
    # and I32 for integer metadata (falls through to fp32).
    mapping: dict[str, str] = {
        "F32": "fp32",
        "F16": "fp16",
        "BF16": "bf16",
        "F8_E4M3": "fp8",
        "F8_E5M2": "fp8",
    }
    # Unknown dtype strings fall through to fp32 as a safe default.
    return mapping.get(safetensors_dtype, "fp32")


def can_handle(key: str) -> bool:
    """Confirm this module handles the given dispatch key.

    The dispatcher passes the ``clip_type`` string as *key*. This
    function returns ``True`` only when the key matches this module's
    canonical architecture identifier.

    Args:
        key: The clip_type string to check, e.g. ``"qwen3"``.

    Returns:
        ``True`` if *key* equals ``"qwen3"``, ``False`` otherwise.
    """
    return key == ARCH
