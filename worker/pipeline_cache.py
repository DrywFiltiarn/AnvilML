"""Per-component LRU cache for the AnvilML Python worker.

This module provides a small, in-process LRU cache used by loader nodes
(LoadModel, LoadVae, LoadClip) to avoid redundant model or component
reload requests within a single worker process lifetime. The cache is
keyed by model/component identifier and stores raw components only — it
does not manage assembled pipelines. Pipeline assembly from cached
components is the diffusion arch module's responsibility.

The cache is intentionally single-threaded: the worker process runs a
single dispatch loop (see ANVILML_DESIGN.md §14.4) so no lock is needed.
If this assumption changes, a threading.Lock should be added around
all cache mutations.
"""

from __future__ import annotations

from collections import OrderedDict
from typing import Any, Callable


class PipelineCache:
    """An LRU cache for model and component objects within a worker process.

    Uses ``collections.OrderedDict`` for O(1) LRU recency tracking via
    ``move_to_end()`` and O(1) eviction via ``popitem(last=False)``.

    Args:
        max_entries: Maximum number of entries the cache holds. When the
            cache exceeds this size, the least-recently-used entry is
            evicted. Defaults to 4, appropriate for holding a small
            number of device models (e.g. UNet, VAE, CLIP text encoder,
            tokenizer).

    Contract:
        ``get_or_load(key, loader_fn)`` returns a cached value if present,
        or calls ``loader_fn()`` exactly once, caches the result, and
        returns it. Subsequent calls with the same key return the cached
        value without invoking ``loader_fn`` again. Failed loader calls
        (exceptions) do not populate the cache — the key remains absent
        and subsequent calls retry.
    """

    def __init__(self, max_entries: int = 4) -> None:
        """Construct a PipelineCache with the given capacity.

        Args:
            max_entries: Maximum number of entries. Defaults to 4.
        """
        self.max_entries = max_entries
        self._cache: OrderedDict[str, Any] = OrderedDict()

    def get_or_load(self, key: str, loader_fn: Callable[[], Any]) -> Any:
        """Return a cached value or load it via *loader_fn* and cache the result.

        If *key* is already in the cache, its recency is refreshed by moving
        it to the end (standard LRU access pattern), then the cached value
        is returned.

        If *key* is not present, *loader_fn* is called exactly once. The
        result is stored in the cache and marked as most-recently-used.
        If the cache exceeds ``max_entries`` after insertion, the oldest
        entry is evicted. The evicted value is discarded — Python's
        refcounting handles resource cleanup.

        If *loader_fn* raises an exception, the cache is not modified and
        the exception propagates to the caller. This means transient failures
        are retried on the next call, not cached as a sentinel.

        Args:
            key: The cache key (e.g. a model ID string).
            loader_fn: A zero-argument callable that produces the value to
                cache. It is invoked at most once per key.

        Returns:
            The cached or freshly loaded value.

        Raises:
            Any exception raised by *loader_fn* is propagated unchanged.
        """
        if key in self._cache:
            # Move to end to mark as most-recently-used — this refreshes
            # recency so the entry survives future evictions. Standard LRU
            # access pattern for OrderedDict.
            self._cache.move_to_end(key)
            return self._cache[key]

        # Key not in cache — call the loader exactly once. The result is
        # stored and the key is moved to the end (most-recently-used).
        # If loader_fn raises, the cache is untouched and the exception
        # propagates — transient failures are retried, not cached.
        value = loader_fn()
        self._cache[key] = value
        self._cache.move_to_end(key)

        # Evict the least-recently-used entry if we exceeded capacity.
        # popitem(last=False) removes from the front (oldest) end.
        # Python's refcounting handles cleanup of the evicted value.
        if len(self._cache) > self.max_entries:
            self._cache.popitem(last=False)

        return value
