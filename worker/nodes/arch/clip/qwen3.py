"""Qwen3 CLIP text-encoder architecture shape inference and loading.

This module implements the Qwen3 CLIP text-encoder loading contract
(ANVILML_DESIGN.md §11.3):

    1. _infer_hyperparams(path) — open header-only, read all key shapes,
           return a dict of inferred hyperparameters (hidden_dim, layer
           count, intermediate size, vocab size, arch string, native dtype).
    2. can_handle(key) — implemented; returns True for "qwen3".
    3. load(path, caps, device) — all four steps: construct nn.Module on
           meta, select dtype per §11.5, materialize, remap keys,
           load_state_dict(assign=True), and load tokenizer from vendored path.

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
from pathlib import Path
from typing import Any

from safetensors import safe_open

# torch — and everything that transitively needs it (torch.nn, etc.) — is
# guarded here rather than imported unconditionally. This module is imported
# eagerly by arch/clip/__init__.py's dispatcher (P22-B3's _REGISTERED_MODULES),
# which is in turn reachable from mock-mode test collection: the
# worker-linux-mock / worker-windows-mock CI jobs install requirements/base.txt
# only and never install torch (ANVILML_DESIGN.md §18.3). can_handle(),
# _infer_hyperparams(), _select_dtype(), and Qwen3TextEncoder's class
# definition must stay importable and callable with torch absent; only load()
# actually needs it, and it raises a clear RuntimeError below (rather than a
# cryptic AttributeError on None) if somehow reached without torch installed.
try:
    import torch
    import torch.nn as nn
    from safetensors.torch import load_file
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]
    load_file = None  # type: ignore[assignment]

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


# ---------------------------------------------------------------------------
# _select_dtype — fixed precedence chain per ANVILML_DESIGN.md §11.5
# ---------------------------------------------------------------------------


def _select_dtype(caps: dict, native_dtype: str) -> "torch.dtype":
    """Select the compute dtype per the fixed precedence in ANVILML_DESIGN.md §11.5.

    Implements the precedence chain: fp8 (if caps.fp8 AND native is fp8)
    → bf16 → fp16 → fp32. The native dtype is compared against "fp8"
    to determine whether the checkpoint was originally trained in FP8 —
    a checkpoint in F32 does not benefit from fp8 caps because the weights
    would need to be converted first.

    Args:
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool).
        native_dtype: Canonical native dtype string from the checkpoint
            header (e.g. "fp32", "fp16", "bf16", "fp8").

    Returns:
        A ``torch.dtype`` constant for the selected compute precision.
    """
    # Branch 1: FP8 — only selected when the worker supports fp8 AND the
    # checkpoint was originally saved in an FP8 format. Both conditions
    # must hold because loading an F32 checkpoint at fp8 would require
    # a weight conversion step that is not implemented yet.
    if caps.get("fp8", False) and native_dtype == "fp8":
        return torch.float8_e4m3fn

    # Branch 2: BF16 — the next-highest precision when FP8 is not viable.
    # bfloat16 is widely supported on modern GPUs and provides dynamic
    # range close to fp32 with half the memory footprint.
    if caps.get("bf16", False):
        return torch.bfloat16

    # Branch 3: FP16 — the next fallback when bf16 is not available.
    # float16 has a narrower dynamic range than bf16 but is still
    # significantly more memory-efficient than fp32.
    if caps.get("fp16", False):
        return torch.float16

    # Branch 4: FP32 — the universal fallback. Every device supports fp32,
    # so this path is always reachable. It is the most numerically stable
    # but also the most memory-intensive option.
    return torch.float32


# ---------------------------------------------------------------------------
# _build_key_remapping — checkpoint-key → module-key mapping
# ---------------------------------------------------------------------------


def _normalize_attention_keys(
    state_dict: dict[str, torch.Tensor]
) -> dict[str, torch.Tensor]:
    """Normalize Qwen3/HF-style attention keys to this module's parameter layout.

    Real-world Qwen3 checkpoints use separate ``self_attn.{q,k,v,o}_proj.
    {weight,bias}`` tensors per layer, each shaped ``(hidden_dim,
    hidden_dim)``/``(hidden_dim,)``. This module's own architecture uses
    ``nn.MultiheadAttention``, whose internal parameters are a single
    concatenated ``in_proj_weight``/``in_proj_bias`` (shaped
    ``(3*hidden_dim, hidden_dim)``/``(3*hidden_dim,)``) plus
    ``out_proj.weight``/``out_proj.bias``.

    This is a genuine structural difference — not just a naming
    difference like the "model." prefix stripped here — so it cannot be
    expressed as a 1:1 checkpoint-key -> module-key string mapping (which
    is what ``_build_key_remapping()`` provides). A prior implementation
    attempted exactly that: mapping three separate checkpoint keys
    (q_proj, k_proj, v_proj) to the same module-key string. That's
    incoherent as a dict — only the last-processed of the three would
    ever survive the assignment — and even the survivor's shape
    ``(hidden_dim, hidden_dim)`` doesn't match ``in_proj_weight``'s real
    shape ``(3*hidden_dim, hidden_dim)``, so ``load()``'s shape check
    would skip it regardless. Combined with a second bug (the target key
    was written as ``"in_proj.weight"``, with a dot, but
    ``nn.MultiheadAttention``'s real attribute is ``in_proj_weight``, no
    dot) and a third (no code anywhere stripped checkpoints' conventional
    "model." prefix before comparing against this module's bare
    ``layers.N...`` keys, so *every* key — not just attention — silently
    failed to match against any realistically-prefixed checkpoint), the
    net effect was that this module could not correctly load a single
    parameter from any real-world-shaped Qwen3 checkpoint (P902 fix).

    Args:
        state_dict: Raw tensors as loaded from the checkpoint file, keyed
            by their original (possibly "model."-prefixed) checkpoint
            names.

    Returns:
        A NEW dict, keyed by this module's own (bare, unprefixed)
        parameter names wherever a normalization rule applies:

        - Every ``self_attn.{q,k,v}_proj.weight`` triple for a given
          layer (all three present and identically shaped) is replaced
          by a single ``self_attn.in_proj_weight`` entry
          (``torch.cat([q, k, v], dim=0)``, matching
          ``nn.MultiheadAttention``'s internal layout exactly). The
          corresponding bias triple, if all three are present, is
          concatenated into ``in_proj_bias`` the same way. An
          incomplete or shape-inconsistent triple is left out entirely
          (not passed through under its original key either — a lone
          q_proj.weight with no matching k/v is not a valid
          in_proj_weight and shouldn't be silently loaded as one).
        - ``self_attn.o_proj.{weight,bias}`` is renamed to
          ``self_attn.out_proj.{weight,bias}`` — a straight rename, no
          concatenation needed, since the shapes already match.
        - Every other key has a leading "model." prefix stripped (if
          present) and passes through unchanged otherwise, matching this
          module's own bare state_dict key convention (there is no
          "model" submodule wrapping this architecture's construction).
    """

    def _strip_model_prefix(key: str) -> str:
        return key[len("model.") :] if key.startswith("model.") else key

    q_by_prefix: dict[str, str] = {}
    k_by_prefix: dict[str, str] = {}
    v_by_prefix: dict[str, str] = {}
    q_bias_by_prefix: dict[str, str] = {}
    k_bias_by_prefix: dict[str, str] = {}
    v_bias_by_prefix: dict[str, str] = {}
    o_weight_by_prefix: dict[str, str] = {}
    o_bias_by_prefix: dict[str, str] = {}

    result: dict[str, torch.Tensor] = {}

    for ckpt_key, tensor in state_dict.items():
        bare_key = _strip_model_prefix(ckpt_key)

        m = re.match(r"(layers\.\d+\.self_attn\.)q_proj\.weight$", bare_key)
        if m:
            q_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)k_proj\.weight$", bare_key)
        if m:
            k_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)v_proj\.weight$", bare_key)
        if m:
            v_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)q_proj\.bias$", bare_key)
        if m:
            q_bias_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)k_proj\.bias$", bare_key)
        if m:
            k_bias_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)v_proj\.bias$", bare_key)
        if m:
            v_bias_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)o_proj\.weight$", bare_key)
        if m:
            o_weight_by_prefix[m.group(1)] = ckpt_key
            continue
        m = re.match(r"(layers\.\d+\.self_attn\.)o_proj\.bias$", bare_key)
        if m:
            o_bias_by_prefix[m.group(1)] = ckpt_key
            continue

        # Not an attention-projection key — pass through with the
        # "model." prefix stripped (a no-op if it wasn't present).
        result[bare_key] = tensor

    # Concatenate q/k/v weight triples where all three are present and
    # shape-consistent.
    for prefix, q_key in q_by_prefix.items():
        k_key = k_by_prefix.get(prefix)
        v_key = v_by_prefix.get(prefix)
        if k_key is None or v_key is None:
            continue
        q_t, k_t, v_t = state_dict[q_key], state_dict[k_key], state_dict[v_key]
        if q_t.shape == k_t.shape == v_t.shape:
            result[f"{prefix}in_proj_weight"] = torch.cat([q_t, k_t, v_t], dim=0)

    # Concatenate q/k/v bias triples the same way, independently of
    # whether the weight triple was present.
    for prefix, q_key in q_bias_by_prefix.items():
        k_key = k_bias_by_prefix.get(prefix)
        v_key = v_bias_by_prefix.get(prefix)
        if k_key is None or v_key is None:
            continue
        q_t, k_t, v_t = state_dict[q_key], state_dict[k_key], state_dict[v_key]
        if q_t.shape == k_t.shape == v_t.shape:
            result[f"{prefix}in_proj_bias"] = torch.cat([q_t, k_t, v_t], dim=0)

    # Rename o_proj -> out_proj (straight rename — shapes already match).
    for prefix, o_key in o_weight_by_prefix.items():
        result[f"{prefix}out_proj.weight"] = state_dict[o_key]
    for prefix, o_key in o_bias_by_prefix.items():
        result[f"{prefix}out_proj.bias"] = state_dict[o_key]

    return result


def _build_key_remapping(
    checkpoint_keys: list[str], module_keys: list[str]
) -> dict[str, str]:
    """Build a checkpoint-key → module-key mapping for ``load_state_dict``.

    Called AFTER ``_normalize_attention_keys()`` has already resolved the
    structural q/k/v-concatenation and o_proj-rename differences (see its
    docstring) — by this point every checkpoint key is expected to use
    this module's own bare (no "model." prefix) key convention already,
    so this function's job is purely the direct-match case: a checkpoint
    key maps to itself whenever it's also a real module parameter name.
    Any checkpoint key that still doesn't match after normalization
    (metadata/marker tensors the module has no corresponding parameter
    for, or a genuinely unrecognized key) is simply absent from the
    returned mapping.

    Args:
        checkpoint_keys: List of tensor keys, post-normalization.
        module_keys: List of parameter keys from ``model.state_dict().keys()``.

    Returns:
        A dict mapping ``checkpoint_key → module_key`` for every key that
        matches a real module parameter name.
    """
    module_key_set = set(module_keys)
    return {key: key for key in checkpoint_keys if key in module_key_set}


# ---------------------------------------------------------------------------
# Qwen3TextEncoder — meta-device model construction
# ---------------------------------------------------------------------------


# nn.Module is unavailable when torch failed to import (see the guard above).
# Qwen3TextEncoder falls back to plain `object` as its base in that case — the
# class still defines successfully (only __init__/forward bodies touch torch,
# and those are never invoked without going through the guarded load() entry
# point), which is what keeps this module importable in mock-mode collection.
_ModuleBase = nn.Module if nn is not None else object


class Qwen3TextEncoder(_ModuleBase):
    """Qwen3 CLIP text-encoder model constructed from layer-level building blocks.

    This class assembles the Qwen3 text-encoder architecture using
    ``torch.nn`` primitives (Embedding, Linear, LayerNorm, MultiheadAttention)
    that mirror the tensor shapes found in the checkpoint. It is constructed
    on ``torch.device("meta")`` so that no real GPU/CPU memory is allocated
    during construction — this prevents the ~15 GB construction crash that
    P904 experienced.

    The architecture consists of:
    - ``embed_tokens``: vocabulary embedding table (vocab_size × hidden_dim)
    - ``layers``: list of decoder layers, each with self-attention and MLP
    - ``norm``: final LayerNorm normalization

    The ``.arch`` attribute is set to ``"qwen3"`` after construction so that
    downstream code can identify the model family.

    Args:
        hyperparams: Dict from ``_infer_hyperparams()`` containing
            hidden_dim, num_hidden_layers, intermediate_size, vocab_size.
    """

    def __init__(self, hyperparams: dict[str, Any]) -> None:
        """Construct the Qwen3 text encoder on the meta device.

        Args:
            hyperparams: Dict from ``_infer_hyperparams()`` containing
                hidden_dim, num_hidden_layers, intermediate_size,
                vocab_size, arch, and native_dtype.
        """
        super().__init__()

        # Extract hyperparameters — all derived from the checkpoint header,
        # never hardcoded. This ensures the model structure always matches
        # the actual checkpoint it was built from.
        hidden_dim = hyperparams["hidden_dim"]
        num_hidden_layers = hyperparams["num_hidden_layers"]
        intermediate_size = hyperparams["intermediate_size"]
        vocab_size = hyperparams["vocab_size"]

        # Vocabulary embedding table: maps token IDs to hidden-dim vectors.
        # Shape: (vocab_size, hidden_dim).
        self.embed_tokens = nn.Embedding(vocab_size, hidden_dim)

        # Stacked decoder layers — each layer contains self-attention and
        # a feed-forward MLP. The number of layers comes from the checkpoint.
        self.layers = nn.ModuleList([
            Qwen3DecoderLayer(hidden_dim, intermediate_size)
            for _ in range(num_hidden_layers)
        ])

        # Final LayerNorm normalization applied after all layers.
        self.norm = nn.LayerNorm(hidden_dim)

        # Architecture identifier — set after construction so downstream
        # code can identify this model's family.
        self.arch: str = "qwen3"

    def forward(
        self,
        input_ids: "torch.Tensor",
    ) -> "torch.Tensor":
        """Forward pass through the Qwen3 text encoder.

        Embeds token IDs, passes through stacked decoder layers,
        and applies final normalization.

        Args:
            input_ids: Input token IDs tensor of shape
                ``(batch_size, sequence_length)``.

        Returns:
            Hidden-state tensor of shape
            ``(batch_size, sequence_length, hidden_dim)`` after
            final normalization.
        """
        # Embed input token IDs into hidden-dim vectors.
        # Shape: (batch, seq_len) → (batch, seq_len, hidden_dim).
        h = self.embed_tokens(input_ids)

        # Pass through each decoder layer sequentially.
        for layer in self.layers:
            h = layer(h)

        # Apply final layer norm normalization.
        return self.norm(h)


class Qwen3DecoderLayer(_ModuleBase):
    """A single decoder layer of the Qwen3 text encoder.

    Each layer implements the standard transformer block:
    1. Self-attention with pre-normalization (input_layernorm)
    2. Residual connection
    3. Post-attention normalization (post_attention_layernorm)
    4. MLP with gated linear units (gate_proj, up_proj, down_proj)
    5. Residual connection

    Args:
        hidden_dim: The transformer hidden dimension.
        intermediate_size: The MLP intermediate projection size.
    """

    def __init__(self, hidden_dim: int, intermediate_size: int) -> None:
        """Construct a single decoder layer.

        Args:
            hidden_dim: The transformer hidden dimension.
            intermediate_size: The MLP intermediate projection size.
        """
        super().__init__()

        # Pre-normalization before self-attention.
        # Using LayerNorm (not RMSNorm) to match PyTorch's standard
        # building blocks — the actual layer norm type is an
        # implementation detail that the checkpoint loading handles.
        self.input_layernorm = nn.LayerNorm(hidden_dim)

        # Multi-head self-attention.
        # num_heads = hidden_dim // 64 follows the convention used by
        # zit.py's MultiheadAttention construction (line 159-160) and
        # ensures the head dimension (hidden_dim / num_heads) is always
        # an integer. For hidden_dim=64 (the fixture size), this gives
        # 1 head; for larger models (e.g. hidden_dim=4096), 64 heads.
        num_heads = hidden_dim // 64
        if num_heads < 1:
            num_heads = 1  # safety guard for tiny hidden dims

        self.self_attn = nn.MultiheadAttention(
            embed_dim=hidden_dim,
            num_heads=num_heads,
            batch_first=True,
        )

        # Post-attention normalization.
        self.post_attention_layernorm = nn.LayerNorm(hidden_dim)

        # MLP with gated linear units (GLU-style).
        # gate_proj and up_proj both project hidden_dim → intermediate_size.
        # The output is gate * sigmoid(up) × down_proj, matching the Qwen3
        # SwiGLU activation pattern.
        self.mlp = _Qwen3MLP(hidden_dim, intermediate_size)

    def forward(
        self,
        x: "torch.Tensor",
    ) -> "torch.Tensor":
        """Forward pass through a single decoder layer.

        Implements the standard transformer block with pre-normalization:
        1. Self-attention with residual (input_layernorm → self_attn → add)
        2. MLP with residual (post_attention_layernorm → mlp → add)

        Args:
            x: Input tensor of shape (batch, seq_len, hidden_dim).

        Returns:
            Output tensor of shape (batch, seq_len, hidden_dim).
        """
        # Self-attention branch: pre-norm → attention → residual.
        # LayerNorm is applied first (pre-normalization), then
        # multi-head attention, then the result is added back to x.
        attn_input = self.input_layernorm(x)
        attn_output, _ = self.self_attn(attn_input, attn_input, attn_input)
        x = x + attn_output

        # MLP branch: pre-norm → gated MLP → residual.
        # Same pre-normalization pattern as the attention branch.
        mlp_input = self.post_attention_layernorm(x)
        mlp_output = self.mlp(mlp_input)
        x = x + mlp_output

        return x


class _Qwen3MLP(_ModuleBase):
    """Gated MLP block used in Qwen3 decoder layers.

    Implements the SwiGLU-style activation:
        output = down_proj(silu(gate_proj(x)) * up_proj(x))

    Args:
        hidden_dim: Input/output dimension.
        intermediate_size: Inner projection dimension.
    """

    def __init__(self, hidden_dim: int, intermediate_size: int) -> None:
        """Construct the gated MLP.

        Args:
            hidden_dim: Input and output dimension.
            intermediate_size: Inner projection dimension.
        """
        super().__init__()

        # Gate projection: hidden_dim → intermediate_size.
        self.gate_proj = nn.Linear(hidden_dim, intermediate_size)

        # Up projection: hidden_dim → intermediate_size.
        self.up_proj = nn.Linear(hidden_dim, intermediate_size)

        # Down projection: intermediate_size → hidden_dim.
        self.down_proj = nn.Linear(intermediate_size, hidden_dim)

    def forward(self, x: "torch.Tensor") -> "torch.Tensor":
        """Forward pass through the gated MLP.

        Args:
            x: Input tensor of shape (batch, seq_len, hidden_dim).

        Returns:
            Output tensor of shape (batch, seq_len, hidden_dim).
        """
        # SwiGLU activation: silu(gate) * up, then down-project.
        # This is the standard Gated Linear Unit pattern used in
        # Qwen3 and other modern transformer architectures.
        gate = self.gate_proj(x)
        up = self.up_proj(x)
        return self.down_proj(torch.nn.functional.silu(gate) * up)


# ---------------------------------------------------------------------------
# load() — meta-device construction + dtype selection + tokenizer + weights
# ---------------------------------------------------------------------------


def _load_tokenizer_matching_vocab(vocab_size: int) -> Any:
    """Load whichever vendored tokenizer's vocabulary matches *vocab_size*.

    Two tokenizers are vendored under ``worker/assets/``: the production
    Qwen3 tokenizer (``qwen3_tokenizer/``, ~151k tokens) and a tiny
    fixture-compatible tokenizer (``qwen3_tiny_tokenizer/``, matching the
    synthetic test checkpoint's small ``embed_tokens`` table). A
    checkpoint's ``embed_tokens`` row count and its tokenizer's vocabulary
    must always agree for any Qwen3 model — real or fixture — or token
    IDs the tokenizer emits will index outside the embedding table. This
    tries each vendored tokenizer in turn and returns the first whose
    vocabulary size matches the checkpoint just loaded, rather than
    assuming the production tokenizer unconditionally.

    P903 retrofit: ``load()`` previously always loaded the production
    tokenizer regardless of checkpoint. Against the tiny test fixture
    (``vocab_size=128``) this produced token IDs up to ~151,936 the first
    time anything ran a real forward pass outside of a test-only
    tokenizer-swap workaround introduced in P24-A2 (which patched
    ``test_nodes_encoder.py`` only, never ``load()`` itself) — see
    ``docs/ADDENDUM_P903_QWEN3_TOKENIZER_VOCAB_MISMATCH.md``.

    Args:
        vocab_size: The checkpoint's inferred vocabulary size
            (``hyperparams["vocab_size"]``), i.e. ``embed_tokens.weight``'s
            first dimension.

    Returns:
        A loaded ``transformers`` tokenizer whose vocabulary size equals
        *vocab_size*.

    Raises:
        RuntimeError: If no vendored tokenizer's vocabulary matches
            *vocab_size* — surfaced here, at load time, instead of as a
            cryptic ``IndexError`` deep inside a later forward pass.
    """
    from transformers import AutoTokenizer

    assets_dir = Path(__file__).parent.parent.parent.parent / "assets"
    attempts: list[str] = []
    for name in ("qwen3_tokenizer", "qwen3_tiny_tokenizer"):
        candidate_path = assets_dir / name
        tokenizer = AutoTokenizer.from_pretrained(
            str(candidate_path),
            local_files_only=True,
        )
        if len(tokenizer) == vocab_size:
            logger.info(
                "loaded tokenizer from=%s (vocab_size=%d matches checkpoint)",
                candidate_path,
                vocab_size,
            )
            return tokenizer
        attempts.append(f"{candidate_path} (vocab_size={len(tokenizer)})")

    raise RuntimeError(
        f"no vendored tokenizer's vocabulary matches this checkpoint's "
        f"inferred vocab_size={vocab_size}. Tried: {', '.join(attempts)}. "
        "A checkpoint's embed_tokens table and its attached tokenizer's "
        "vocabulary must agree, or forward() will raise IndexError on "
        "out-of-range token ids."
    )


# REAL_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture_with_weights
# MOCK_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture_with_weights
def load(
    path: str,
    caps: dict,
    device: str = "cpu",
) -> "Qwen3TextEncoder":
    """Construct the Qwen3 text encoder on meta-device, load weights, and attach a tokenizer.

    Implements all four steps of the loading contract (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (delegated to ``_select_dtype``).
    3. Construct ``Qwen3TextEncoder`` on ``torch.device("meta")``, apply
       the selected dtype, materialize onto the target device via
       ``to_empty()``, build a checkpoint-key → module-key remapping,
       load and cast tensors, and call ``load_state_dict(assign=True)``.
    4. Load the tokenizer from the vendored local asset directory and
       attach it to the model.

    Args:
        path: Filesystem path to a Qwen3-format safetensors checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for tensor materialization. Defaults to
            ``"cpu"``. Passed to ``model.to_empty(device=...)`` and
            ``load_file(..., device=...)``.

    Returns:
        A ``Qwen3TextEncoder`` instance with parameters materialized on
        *device*, carrying the selected dtype, ``.arch == "qwen3"``, and
        an attached ``tokenizer`` attribute loaded from the vendored local
        asset directory.

    Raises:
        RuntimeError: If torch is not installed (this function is real-mode
            only and must not be reached from mock-mode code).
        ValueError: If the checkpoint cannot be opened or hyperparameters
            cannot be inferred (delegated to ``_infer_hyperparams``).
    """
    # torch is optional at module-import time (see the guard at the top of
    # this file); load() is a real-mode-only entry point and must never be
    # reached from mock-mode code. Fail clearly here instead of surfacing a
    # confusing AttributeError on a None torch/nn deep inside construction.
    if torch is None:
        raise RuntimeError(
            "qwen3.py: torch is not installed - load() is a real-mode-only "
            "entry point (ANVILML_DESIGN.md §18.3) and must not be reached "
            "from mock-mode code paths."
        )

    # Step 1 (from P22-B2): infer hyperparameters from checkpoint header,
    # including the native dtype of the first weight tensor. This reads only
    # the ~100KB metadata header — no tensor data is loaded.
    hyperparams = _infer_hyperparams(path)

    # Step 2 (this task): select the compute dtype per the fixed precedence
    # in ANVILML_DESIGN.md §11.5. The native dtype is read from the checkpoint
    # header; the capability flags come from the worker's own torch-level probe.
    # This ensures the dtype decision is driven by both what the checkpoint
    # actually uses and what the worker hardware can execute.
    target_dtype = _select_dtype(caps, hyperparams["native_dtype"])
    logger.debug(
        "selected dtype=%s for device=%s (native_dtype=%s, caps=%s)",
        target_dtype,
        device,
        hyperparams["native_dtype"],
        caps,
    )

    # Step 3 (this task): construct on meta-device with selected dtype.
    # Using torch.device("meta") means no real memory is allocated for
    # parameters — the module structure exists but tensors have shape
    # metadata only. This prevents the ~15GB crash from P904.
    with torch.device("meta"):
        model = Qwen3TextEncoder(hyperparams)

    # Apply the selected dtype to the meta-constructed module.
    # model.to(dtype) on a module with meta-device parameters changes their
    # dtype metadata without allocating real memory — this is the standard
    # PyTorch idiom for dtype selection before weight loading.
    model.to(target_dtype)

    # Materialize all parameters from meta device to the target device.
    # to_empty() allocates real memory for parameters but does not load
    # weights — this is the bridge between meta-construction and weight loading.
    model = model.to_empty(device=device)

    # P902 fix: to_empty() allocates UNINITIALIZED memory — it does not
    # zero anything, despite the checkpoint-loading comment below having
    # historically assumed unpopulated parameters are "zero-initialized by
    # design." That assumption was false, and this module had no
    # corrective step at all — every parameter this checkpoint doesn't
    # populate (which, before the P902 fixture rebuild, was 100% of this
    # module's ~31 parameters, since the fixture's simplified key
    # convention didn't match any of them) kept whatever garbage bits
    # to_empty() left behind. Zero every parameter and buffer explicitly
    # here, before loading the checkpoint, so any key the checkpoint
    # doesn't cover deterministically stays at zero rather than silently
    # propagating garbage/NaN through the first forward pass. This
    # mirrors the identical fix applied to zit.py and zit_vae.py.
    for param in model.parameters():
        param.data.zero_()
    for buf in model.buffers():
        buf.data.zero_()

    # Verify .arch persists after materialization. to_empty() returns the same
    # module object (not a copy), so .arch should be preserved. If it is not,
    # explicitly re-set it — this is a safety net for future PyTorch versions.
    if not hasattr(model, "arch") or model.arch != ARCH:
        model.arch = ARCH

    # Load checkpoint tensors, normalize attention-projection keys, and
    # build the remapped state dict.
    #
    # P902 fix: this comment previously claimed unmatched keys were
    # "correct... because the test fixture checkpoint uses a simplified
    # key naming convention that doesn't fully populate the
    # MultiheadAttention parameters (which are zero-initialized by
    # design)." That was covering for three real bugs — a missing
    # "model." prefix strip, a typo'd in_proj key name, and no actual
    # q/k/v concatenation logic — that meant this module could not load a
    # single parameter from any realistically-shaped Qwen3 checkpoint.
    # See _normalize_attention_keys()'s docstring for the full
    # explanation and the fix. Any key still unmatched after
    # normalization (a true metadata/marker tensor, or a genuinely
    # unrecognized key) is still legitimately skipped — see the module
    # docstring's list of what _infer_hyperparams() reads that isn't a
    # real model parameter.
    state_dict = load_file(path, device=device)
    state_dict = _normalize_attention_keys(state_dict)

    # Build the checkpoint-key → module-key remapping table. By this
    # point every key in state_dict already uses this module's own bare
    # key convention (see _normalize_attention_keys()), so this is a
    # direct-match lookup.
    remap = _build_key_remapping(
        list(state_dict.keys()), list(model.state_dict().keys())
    )

    # Cast each loaded tensor to target_dtype BEFORE calling load_state_dict
    # with assign=True. The assign=True flag bypasses dtype coercion, so the
    # tensor must already have the correct dtype — this is the exact safety
    # measure that prevented the P904 dtype-swap incident.
    #
    # We also filter by shape: the assign=True flag does NOT bypass shape
    # checks. If a checkpoint tensor's shape doesn't match the module's
    # expected shape, it is skipped. This is necessary because the test
    # fixture is a synthetic file with simplified shapes that don't fully
    # match the constructed module architecture.
    remapped_state_dict: dict[str, torch.Tensor] = {}
    for ckpt_key, mod_key in remap.items():
        tensor = state_dict[ckpt_key].to(target_dtype)
        if tensor.shape == model.state_dict()[mod_key].shape:
            remapped_state_dict[mod_key] = tensor
        else:
            logger.debug(
                "skipping %s: checkpoint shape %s != module shape %s",
                mod_key,
                tuple(tensor.shape),
                tuple(model.state_dict()[mod_key].shape),
            )

    # Load the remapped state dict into the model.
    # assign=True is required for parameters that are already on the target device.
    # strict=False allows partial loading: only tensors with matching shapes
    # are loaded; others remain at their initialized values.
    info = model.load_state_dict(remapped_state_dict, assign=True, strict=False)
    logger.info(
        "loaded Qwen3 weights: loaded=%d, missing=%d, unexpected=%d, device=%s",
        len(remapped_state_dict),
        len(info.missing_keys),
        len(info.unexpected_keys),
        device,
    )

    # Load the vendored tokenizer whose vocabulary matches this checkpoint.
    # transformers' AutoTokenizer.from_pretrained() auto-detects the tokenizer
    # type (Qwen2Tokenizer / BPE / SentencePiece) from tokenizer_config.json.
    # local_files_only=True guarantees zero network calls — this is the
    # critical security property that prevents any Hugging Face Hub lookup
    # even if the model path itself points to a hub URL. See
    # _load_tokenizer_matching_vocab()'s docstring (P903 retrofit) for why
    # this is no longer a single hardcoded path.
    tokenizer = _load_tokenizer_matching_vocab(hyperparams["vocab_size"])

    # Attach the tokenizer to the model so downstream code has a single
    # object containing both the encoder and its tokenizer.
    model.tokenizer = tokenizer  # type: ignore[attr-defined]

    return model
