"""In-worker LRU cache for model components.

Provides an in-worker LRU (Least-Recently-Used) cache used by loader nodes
and arch modules to avoid reloading model components from disk. Cache entries
are keyed by ``(model_id, dtype)`` pairs. When the cache exceeds its configured
capacity, the least-recently-used entry is evicted.

The cache handles ``torch.cuda.OutOfMemoryError`` by evicting all entries and
retrying the loader function once. In mock mode (no torch), the OOM path is
gracefully skipped.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import logging
import types
from collections import OrderedDict
from typing import Any

logger = logging.getLogger(__name__)

# Lazy torch availability check at module load time.
# In mock mode torch is not installed; catching OutOfMemoryError unconditionally
# would raise a NameError at runtime. We guard the except clause by checking
# this flag so the module remains importable without torch.
try:
    import torch  # noqa: F401  # used only for the exception type below

    _TORCH_AVAILABLE = True
except ImportError:
    _TORCH_AVAILABLE = False


class PipelineCache:
    """An in-worker LRU cache for model components.

    Cache entries are keyed by ``f"{model_id}:{dtype}"``. When the cache
    exceeds ``max_entries``, the least-recently-used entry is evicted via
    ``OrderedDict.popitem(last=False)``.

    Attributes:
        max_entries: Maximum number of entries the cache can hold before
            eviction begins.
    """

    def __init__(self, max_entries: int = 2) -> None:
        """Initialise the cache with a given capacity.

        Args:
            max_entries: Maximum number of entries before LRU eviction.
                Defaults to 2.
        """
        self.max_entries = max_entries
        self._cache: OrderedDict[str, Any] = OrderedDict()

    def get_or_load(
        self,
        model_id: str,
        dtype: str,
        loader_fn: types.Callable[[], Any],
    ) -> Any:
        """Return a cached value or compute and cache it via *loader_fn*.

        If the ``(model_id, dtype)`` key already exists in the cache, the
        entry is moved to the most-recently-used position and returned
        without invoking *loader_fn*.

        If the key is absent, *loader_fn* is called to compute the value.
        Before insertion, if the cache is full the LRU entry is evicted
        and logged at INFO level.

        On ``torch.cuda.OutOfMemoryError`` the entire cache is cleared
        (logged at WARN), and the loader is retried once. If the retry
        also raises, the exception is propagated.

        Args:
            model_id: Stable model identifier (e.g. SHA256 hex digest).
            dtype: Target data type string (e.g. ``"fp16"``, ``"bf16"``).
            loader_fn: Zero-argument callable that computes the value.
                Called only on cache miss.

        Returns:
            The cached or freshly computed value.

        Raises:
            torch.cuda.OutOfMemoryError: If the retry after OOM also fails.
        """
        key = f"{model_id}:{dtype}"

        # Cache hit: move to end (most recently used) and return.
        if key in self._cache:
            self._cache.move_to_end(key)
            logger.debug("pipeline_cache: cache hit key=%s", key)
            return self._cache[key]

        # Cache miss: compute the value via loader_fn.
        # Wrap in OOM handling — on OOM evict all entries and retry once.
        try:
            value = loader_fn()
        except Exception as exc:
            # Only catch torch.cuda.OutOfMemoryError; other exceptions
            # propagate normally. The _TORCH_AVAILABLE guard ensures this
            # branch is harmless when torch is not installed (mock mode).
            if _TORCH_AVAILABLE and isinstance(
                exc, torch.cuda.OutOfMemoryError
            ):
                # OOM: evict everything and retry once.
                self._cache.clear()
                logger.warning(
                    "pipeline_cache: OOM, evicted all entries"
                )
                # Retry the loader function a single time.
                return loader_fn()
            raise

        # Before inserting: evict LRU entry if cache is at capacity.
        if len(self._cache) >= self.max_entries:
            evicted_key, _evicted_value = self._cache.popitem(last=False)
            logger.info(
                "pipeline_cache: evicted key=%s",
                evicted_key,
            )

        # Insert the new entry (OrderedDict automatically places it at
        # the end, making it the most recently used).
        self._cache[key] = value
        logger.debug("pipeline_cache: cached key=%s", key)
        return value
