"""Tests for worker.pipeline_cache — LRU component cache."""

from worker.pipeline_cache import PipelineCache


def test_get_or_load_cached_returns_without_calling_loader() -> None:
    """Repeated calls with the same key call loader_fn exactly once.

    Creates a cache, defines a loader_fn that tracks invocation count,
    calls get_or_load() twice with the same key, and asserts that:
    (1) both calls return the same value,
    (2) loader_fn was called exactly once.

    Preconditions: Empty cache.
    Expected output: loader_fn called once; both calls return "loaded_value".
    """
    call_count = 0

    def loader_fn() -> str:
        nonlocal call_count
        call_count += 1
        return "loaded_value"

    cache = PipelineCache()
    result1 = cache.get_or_load("model_a", loader_fn)
    result2 = cache.get_or_load("model_a", loader_fn)

    assert result1 == "loaded_value"
    assert result2 == "loaded_value"
    assert call_count == 1


def test_get_or_load_different_keys_each_call_loader() -> None:
    """Different keys each produce their own independent loader_fn call.

    Creates a cache, calls get_or_load() with two distinct keys, and
    asserts that loader_fn was called exactly twice (once per key).

    Preconditions: Empty cache.
    Expected output: loader_fn called twice; each key returns its own value.
    """
    call_count = 0

    def loader_fn() -> str:
        nonlocal call_count
        call_count += 1
        return f"value_{call_count}"

    cache = PipelineCache()
    result_a = cache.get_or_load("model_a", loader_fn)
    result_b = cache.get_or_load("model_b", loader_fn)

    assert result_a == "value_1"
    assert result_b == "value_2"
    assert call_count == 2


def test_lru_eviction_removes_least_recently_used() -> None:
    """When cache exceeds max_entries, the oldest entry is evicted.

    Creates a cache with max_entries=2, inserts three distinct keys
    in order (A, B, C), and asserts that after inserting C, key A
    has been evicted (loader_fn was never called for A on a retry,
    meaning it was evicted, not replaced).

    Preconditions: Cache with max_entries=2.
    Expected output: Key C is present; key A was evicted.
    """
    call_order: list[str] = []

    def make_loader(key: str) -> callable:
        """Return a loader_fn for *key* that records the call order.

        Args:
            key: The model key this loader is responsible for.

        Returns:
            A zero-argument callable that appends *key* to call_order
            and returns a string identifying the key.
        """
        def loader() -> str:
            call_order.append(key)
            return f"result_for_{key}"
        return loader

    cache = PipelineCache(max_entries=2)
    cache.get_or_load("A", make_loader("A"))
    cache.get_or_load("B", make_loader("B"))
    cache.get_or_load("C", make_loader("C"))

    # A should have been evicted when C was inserted.
    assert "A" not in call_order or call_order.index("A") == 0
    # But B and C should both be present (B inserted second, C third).
    assert "B" in call_order
    assert "C" in call_order
    # Verify the cache only contains B and C by checking get_or_load
    # for A — it should NOT call the loader again since A was evicted.
    # We verify by checking the cache internals.
    assert len(cache._cache) == 2
    assert "B" in cache._cache
    assert "C" in cache._cache
    assert "A" not in cache._cache


def test_access_refreshes_recency() -> None:
    """Accessing a cached entry moves it to most-recently-used position, protecting it from eviction.

    Creates a cache with max_entries=2, inserts A then B, then accesses A
    (which should move it to the most-recently-used end), then inserts C.
    Asserts that B — not A — is evicted, proving that access refreshed A's
    recency.

    Preconditions: Cache with max_entries=2.
    Expected output: B is evicted; A and C remain in cache.
    """
    cache = PipelineCache(max_entries=2)

    def make_loader(key: str) -> callable:
        """Return a loader_fn for *key*.

        Args:
            key: The model key this loader is responsible for.

        Returns:
            A zero-argument callable returning a string identifying the key.
        """
        def loader() -> str:
            return f"result_for_{key}"
        return loader

    cache.get_or_load("A", make_loader("A"))
    cache.get_or_load("B", make_loader("B"))

    # Access A — this should move it to the most-recently-used end.
    _ = cache.get_or_load("A", make_loader("A"))

    # Now insert C — B (the least-recently-used) should be evicted, not A.
    cache.get_or_load("C", make_loader("C"))

    assert len(cache._cache) == 2
    assert "A" in cache._cache
    assert "C" in cache._cache
    assert "B" not in cache._cache


def test_custom_max_entries() -> None:
    """Cache respects a non-default max_entries value.

    Creates a cache with max_entries=3, inserts 3 entries, asserts the
    cache is full, then inserts a 4th entry and asserts the cache still
    has exactly 3 entries (oldest evicted).

    Preconditions: Cache with max_entries=3.
    Expected output: Cache holds exactly 3 entries after 4 insertions.
    """
    cache = PipelineCache(max_entries=3)

    def loader() -> str:
        return "value"

    # Insert 3 entries — cache should be full.
    for i in range(3):
        cache.get_or_load(f"key_{i}", loader)

    assert len(cache._cache) == 3

    # Insert a 4th entry — oldest should be evicted.
    cache.get_or_load("key_3", loader)

    assert len(cache._cache) == 3
    assert "key_0" not in cache._cache
    assert "key_3" in cache._cache


def test_evicted_entry_is_truly_removed() -> None:
    """After eviction, get_or_load for the evicted key calls loader_fn again.

    Creates a cache with max_entries=2, fills it with A and B, then
    inserts C (evicting A). Calls get_or_load("A", ...) again and
    asserts that loader_fn is called a second time for key A (proving
    the entry was truly removed, not just overwritten with the same value).

    Preconditions: A was evicted from the cache.
    Expected output: loader_fn called twice for key A.
    """
    call_count = 0

    def loader_fn() -> str:
        nonlocal call_count
        call_count += 1
        return f"value_{call_count}"

    cache = PipelineCache(max_entries=2)

    # Fill the cache with A and B.
    cache.get_or_load("A", loader_fn)
    cache.get_or_load("B", loader_fn)

    # Insert C — A should be evicted.
    cache.get_or_load("C", loader_fn)

    # Verify A was evicted by checking the cache internals.
    assert "A" not in cache._cache
    assert len(cache._cache) == 2

    # Call get_or_load for A again — loader_fn should be called again.
    result = cache.get_or_load("A", loader_fn)

    # loader_fn should have been called 3 times total: once for A (initial),
    # once for B, once for C, and now again for A (re-loaded after eviction).
    assert call_count == 4
    assert result == "value_4"
