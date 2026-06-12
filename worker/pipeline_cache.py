"""LRU pipeline cache with VRAM-aware eviction.

The :class:`PipelineCache` class provides an LRU cache for loaded pipeline
objects keyed on ``(model_id, dtype)``.  When the estimated VRAM of a
newly-loaded pipeline exceeds the available free VRAM, the least-recently-
used entries are evicted (with ``torch.cuda.empty_cache()``) until enough
headroom is reclaimed.

Mock / CPU mode
---------------
When ``ANVILML_WORKER_MOCK=1`` the module sets ``torch = None`` and
``_free_vram_mib()`` returns a large sentinel (8192 MiB) so that eviction
never spuriously triggers during tests or on CPU-only hardware.
"""

from __future__ import annotations

import logging
import os
from collections import OrderedDict
from typing import Any, Callable

logger = logging.getLogger(__name__)

# ── Conditional torch import (same guard as worker_main.py) ────────────────────

_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

if _mock:
    torch: Any = None  # type: ignore[name-defined]
else:
    import torch  # noqa: E402  # type: ignore[assignment]

# ── VRAM estimation registry ───────────────────────────────────────────────────

# Heuristic VRAM estimates (MiB) per model_id.  Unknown models default to 2048.
_MODEL_VRAM_REGISTRY: dict[str, int] = {
    "stable-diffusion-v1-5": 2048,
    "stable-diffusion-v2-1": 2048,
    "stable-diffusion-xl-base-1-0": 4096,
    "sdxl-turbo": 2048,
    "sd-vae-ft-mse": 512,
}

# Dtype VRAM multipliers (float32 models use ~2× the storage).
_DTYPE_FACTOR: dict[str, float] = {
    "bf16": 1.0,
    "fp16": 1.0,
    "fp32": 2.0,
}


def _estimate_vram_mib(model_id: str, dtype: str) -> int:
    """Return a heuristic VRAM estimate in MiB for the given model + dtype.

    Parameters
    ----------
    model_id :
        Model identifier (e.g. ``"stable-diffusion-v1-5"``).
    dtype :
        Data type (e.g. ``"bf16"``, ``"fp16"``, ``"fp32"``).

    Returns
    -------
    int
        Estimated VRAM usage in MiB.
    """
    base = _MODEL_VRAM_REGISTRY.get(model_id, 2048)
    factor = _DTYPE_FACTOR.get(dtype, 1.0)
    return int(base * factor)


def _free_vram_mib() -> int:
    """Return the current free VRAM in MiB.

    In mock or CPU mode (torch is ``None`` or CUDA unavailable) returns a
    large sentinel (8192 MiB) so that eviction logic does not spuriously
    trigger.
    """
    if torch is None:
        return 8192
    try:
        if not torch.cuda.is_available():
            return 8192
        _, free = torch.cuda.mem_get_info()
        return free // (1024 * 1024)
    except Exception:
        return 8192


class PipelineCache:
    """LRU cache for diffusion pipeline objects with VRAM-aware eviction.

    Parameters
    ----------
    max_entries :
        Maximum number of pipeline entries the cache will hold.
    """

    def __init__(self, max_entries: int = 4) -> None:
        self._cache: OrderedDict[tuple[str, str], dict[str, Any]] = OrderedDict()
        self._max_entries = max_entries

    # ── Public API ─────────────────────────────────────────────────────

    def get_or_load(
        self,
        model_id: str,
        dtype: str,
        loader: Callable[[], Any],
    ) -> Any:
        """Load a pipeline from cache or invoke *loader*.

        If the ``(model_id, dtype)`` key is already in the cache the cached
        pipeline is returned and moved to the MRU position.  Otherwise the
        cache evicts LRU entries until enough VRAM headroom exists, then
        calls *loader* to produce a fresh pipeline.

        Parameters
        ----------
        model_id :
            Model identifier.
        dtype :
            Data type.
        loader :
            Callable that produces a new pipeline object when invoked.

        Returns
        -------
        Any
            The pipeline object.
        """
        key: tuple[str, str] = (model_id, dtype)
        est_vram = _estimate_vram_mib(model_id, dtype)

        # ── Cache hit ──────────────────────────────────────────────
        if key in self._cache:
            self._cache.move_to_end(key)
            cached = self._cache[key]
            logger.debug(
                "pipeline cache hit: model=%s dtype=%s (est %d MiB)",
                model_id, dtype, cached["est_vram_mib"],
            )
            return cached["pipeline"]

        # ── Cache miss — evict if needed ───────────────────────────
        while _free_vram_mib() < est_vram and len(self._cache) > 0:
            lru_key, lru_entry = next(iter(self._cache.items()))
            del self._cache[lru_key]
            logger.debug(
                "pipeline cache eviction: model=%s dtype=%s (est %d MiB) "
                "freed ~%d MiB (free=%d MiB)",
                lru_key[0], lru_key[1],
                lru_entry["est_vram_mib"],
                lru_entry["est_vram_mib"],
                _free_vram_mib(),
            )
            # Free CUDA memory after dropping the reference.
            if torch is not None:
                torch.cuda.empty_cache()

        # ── Load ───────────────────────────────────────────────────
        pipeline = loader()
        self._cache[key] = {
            "pipeline": pipeline,
            "est_vram_mib": est_vram,
        }
        self._cache.move_to_end(key)

        logger.debug(
            "pipeline cache miss: model=%s dtype=%s loaded (est %d MiB, "
            "cache_size=%d)",
            model_id, dtype, est_vram, len(self._cache),
        )
        return pipeline

    @property
    def size(self) -> int:
        """Number of entries currently in the cache."""
        return len(self._cache)

    def clear(self) -> None:
        """Remove all entries from the cache."""
        self._cache.clear()
