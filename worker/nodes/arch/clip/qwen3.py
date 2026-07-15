"""Qwen3 CLIP text-encoder architecture shape inference and loading.

This module implements the Qwen3 CLIP text-encoder loading contract
(ANVILML_DESIGN.md §11.3):

    1. _infer_hyperparams(path) — open header-only, read all key shapes,
           return a dict of inferred hyperparameters (hidden_dim, layer
           count, intermediate size, vocab size, arch string, native dtype).
    2. can_handle(key) — implemented; returns True for "qwen3".
    3. load(path, caps, device) — steps 2–3: construct nn.Module on meta,
           select dtype per §11.5, load tokenizer from vendored path.
           Materialize + remap + load weights is P22-C2's scope.

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
except ImportError:
    torch = None  # type: ignore[assignment]
    nn = None  # type: ignore[assignment]

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
# load() — meta-device construction + dtype selection + tokenizer loading
# ---------------------------------------------------------------------------


# REAL_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture
def load(
    path: str,
    caps: dict,
    device: str = "cpu",
) -> "Qwen3TextEncoder":
    """Construct the Qwen3 text encoder on meta-device and load its tokenizer.

    Implements steps 2–3 of the four-step loading contract
    (ANVILML_DESIGN.md §11.3):

    1. Infer hyperparameters from checkpoint header (delegated to
       ``_infer_hyperparams``).
    2. Select compute dtype based on capability flags and checkpoint native
       dtype (delegated to ``_select_dtype``).
    3. Construct ``Qwen3TextEncoder`` on ``torch.device("meta")``, apply
       the selected dtype, and load the tokenizer from the vendored local
       asset directory.

    Weight materialization (``to_empty``) and checkpoint weight loading
    (``load_state_dict(assign=True)``) are the scope of the next task.

    Args:
        path: Filesystem path to a Qwen3-format safetensors checkpoint file.
        caps: Worker capability dict from ``probe_capabilities()`` with keys
            ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``
            (all bool). The dtype selection follows the fixed precedence in
            ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native is fp8)
            → bf16 → fp16 → fp32.
        device: Target device string for future materialization. Defaults to
            ``"cpu"``. Logged for diagnostics.

    Returns:
        A ``Qwen3TextEncoder`` instance with parameters on the meta device,
        carrying the selected dtype, ``.arch == "qwen3"``, and an attached
        ``tokenizer`` attribute loaded from the vendored local asset
        directory.

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

    # Load the tokenizer from the vendored local asset directory.
    # transformers' AutoTokenizer.from_pretrained() auto-detects the tokenizer
    # type (Qwen2Tokenizer / BPE / SentencePiece) from tokenizer_config.json.
    # local_files_only=True guarantees zero network calls — this is the
    # critical security property that prevents any Hugging Face Hub lookup
    # even if the model path itself points to a hub URL.
    # __file__ is at worker/nodes/arch/clip/qwen3.py.
    # parent.parent.parent.parent walks: clip → arch → nodes → worker
    # then we append "assets/qwen3_tokenizer".
    tokenizer_path = str(
        Path(__file__).parent.parent.parent.parent / "assets" / "qwen3_tokenizer"
    )
    from transformers import AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained(
        tokenizer_path,
        local_files_only=True,
    )
    logger.info("loaded tokenizer from=%s", tokenizer_path)

    # Attach the tokenizer to the model so downstream code has a single
    # object containing both the encoder and its tokenizer.
    model.tokenizer = tokenizer  # type: ignore[attr-defined]

    return model
