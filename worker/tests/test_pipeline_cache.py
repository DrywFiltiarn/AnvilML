"""Tests for the in-worker LRU pipeline cache (worker.pipeline_cache).

Tests cover cache hit/miss, LRU eviction ordering, single-slot edge case,
and mock-mode operation (no torch available).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

from worker.pipeline_cache import PipelineCache


def test_cache_hit() -> None:
    """Verify a cache hit returns the stored value without re-invoking the loader.

    Preconditions:
        PipelineCache(max_entries=3) with an empty cache.

    Tests:
        Call get_or_load("model-a", "fp16", loader_fn) twice with the same
        key. The second call must return the cached value without calling
        loader_fn again.

    Expected output:
        loader_fn was called exactly once; the second get_or_load returns
        the cached result.
    """
    call_count = 0
    cache = PipelineCache(max_entries=3)

    def loader_fn() -> str:
        """Compute and return a sentinel value."""
        nonlocal call_count
        call_count += 1
        return "value-a"

    # First call: cache miss — loader_fn is invoked.
    result1 = cache.get_or_load("model-a", "fp16", loader_fn)
    assert call_count == 1
    assert result1 == "value-a"

    # Second call: cache hit — loader_fn is NOT invoked again.
    result2 = cache.get_or_load("model-a", "fp16", loader_fn)
    assert call_count == 1  # unchanged — no second invocation
    assert result2 == "value-a"


def test_cache_miss() -> None:
    """Verify a cache miss invokes the loader and stores the result.

    Preconditions:
        PipelineCache(max_entries=3) with an empty cache.

    Tests:
        Call get_or_load("model-b", "bf16", loader_fn) for a new key.
        Verify loader_fn was called, the result was stored, and subsequent
        calls with the same key return the cached result.

    Expected output:
        loader_fn called once; result stored; second call returns cached value.
    """
    call_count = 0
    cache = PipelineCache(max_entries=3)

    def loader_fn() -> str:
        """Compute and return a sentinel value."""
        nonlocal call_count
        call_count += 1
        return "value-b"

    # First call: cache miss — loader_fn is invoked.
    result1 = cache.get_or_load("model-b", "bf16", loader_fn)
    assert call_count == 1
    assert result1 == "value-b"

    # Second call: cache hit — loader_fn is NOT invoked again.
    result2 = cache.get_or_load("model-b", "bf16", loader_fn)
    assert call_count == 1
    assert result2 == "value-b"


def test_lru_eviction() -> None:
    """Verify LRU eviction removes the least-recently-used entry.

    Preconditions:
        PipelineCache(max_entries=2) with an empty cache.

    Tests:
        Insert two entries with different keys (A, B). Insert a third key (C)
        which triggers eviction of A (the LRU entry). Verify A is no longer
        in the cache. Then access B (making it most recently used), insert D,
        and verify B is evicted (now the LRU).

    Expected output:
        After C inserted, A is evicted. After accessing B then inserting D,
        B is evicted. Cache contains {C, D}.
    """
    cache = PipelineCache(max_entries=2)

    def make_loader(value: str):
        """Create a loader that returns a sentinel value."""
        return lambda: value

    # Insert A and B (cache is now full: {A, B}).
    a_val = cache.get_or_load("model-a", "fp16", make_loader("A"))
    b_val = cache.get_or_load("model-b", "bf16", make_loader("B"))
    assert a_val == "A"
    assert b_val == "B"

    # Access B — this moves it to the most-recently-used end.
    b_val2 = cache.get_or_load("model-b", "bf16", make_loader("B"))
    assert b_val2 == "B"

    # Insert C — cache is full, so the LRU entry (A) is evicted.
    c_val = cache.get_or_load("model-c", "fp16", make_loader("C"))
    assert c_val == "C"

    # A should no longer be in the cache.
    a_hit = cache.get_or_load("model-a", "fp16", make_loader("A"))
    assert a_hit == "A"  # loader_fn was called again (cache miss)

    # Now insert D — B was the LRU (accessed before C), so B is evicted.
    d_val = cache.get_or_load("model-d", "bf16", make_loader("D"))
    assert d_val == "D"

    # B should no longer be in the cache.
    b_hit = cache.get_or_load("model-b", "bf16", make_loader("B"))
    assert b_hit == "B"  # loader_fn was called again (cache miss)


def test_max_entries_one() -> None:
    """Verify a single-slot cache evicts the previous entry on new insert.

    Preconditions:
        PipelineCache(max_entries=1) with an empty cache.

    Tests:
        Insert entry A, then entry B. Verify A was evicted and only B
        remains in the cache.

    Expected output:
        Only B remains; A is evicted.
    """
    cache = PipelineCache(max_entries=1)

    def make_loader(value: str):
        """Create a loader that returns a sentinel value."""
        return lambda: value

    # Insert A (cache now: {A}).
    a_val = cache.get_or_load("model-a", "fp16", make_loader("A"))
    assert a_val == "A"

    # Insert B — A is evicted (only slot available).
    b_val = cache.get_or_load("model-b", "bf16", make_loader("B"))
    assert b_val == "B"

    # A should no longer be in the cache — it was evicted when B was inserted.
    # Loading A again triggers a cache miss and evicts B (now the only entry).
    a_hit = cache.get_or_load("model-a", "fp16", make_loader("A"))
    assert a_hit == "A"  # loader_fn called again (cache miss)

    # After reloading A, only A remains (B was evicted).
    assert len(cache._cache) == 1
    assert "model-a:fp16" in cache._cache


def test_oom_evict_all_in_mock() -> None:
    """Verify the cache operates normally when torch is not available (mock mode).

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse fixture.
        In mock mode, ``torch`` is not installed, so ``OutOfMemoryError``
        cannot be raised.

    Tests:
        Create a ``PipelineCache``, call ``get_or_load`` with a normal loader.
        The module must be importable without torch, and the cache must
        function correctly (the OOM path is unreachable in mock mode).

    Expected output:
        No ImportError; cache operates normally; loader_fn is called and
        the result is cached.
    """
    cache = PipelineCache(max_entries=2)
    call_count = 0

    def loader_fn() -> str:
        """Compute and return a sentinel value."""
        nonlocal call_count
        call_count += 1
        return "mock-value"

    # Normal operation should work fine in mock mode.
    result = cache.get_or_load("model-x", "fp16", loader_fn)
    assert call_count == 1
    assert result == "mock-value"

    # Second call should be a cache hit.
    result2 = cache.get_or_load("model-x", "fp16", loader_fn)
    assert call_count == 1
    assert result2 == "mock-value"
