"""Tests for :mod:`worker.pipeline_cache` and the OOM trap in
:mod:`worker.executor`.

All tests run under ``ANVILML_WORKER_MOCK=1``.  Where the test needs
a real ``torch`` object (e.g. the OOM trap test) it patches the
executor module's ``torch`` and ``_mock`` attributes.
"""

from __future__ import annotations

import sys
import threading
import types
from unittest.mock import MagicMock, patch

import pytest

from worker.executor import CancelledError, run_graph
from worker.nodes.base import BaseNode, NODE_REGISTRY, register
from worker.pipeline_cache import PipelineCache


# ── Fixtures ────────────────────────────────────────────────────────────────────


@pytest.fixture(autouse=True)
def _clear_registry() -> None:
    """Ensure NODE_REGISTRY is empty before each test."""
    NODE_REGISTRY.clear()


@pytest.fixture
def mock_torch() -> types.ModuleType:
    """Provide a fake torch module with cuda.OutOfMemoryError."""
    fake_cuda = MagicMock()
    oom_exc = type("OutOfMemoryError", (RuntimeError,), {})
    fake_cuda.OutOfMemoryError = oom_exc

    fake_torch = MagicMock()
    fake_torch.cuda = fake_cuda
    return fake_torch


# ── PipelineCache unit tests ────────────────────────────────────────────────────


class TestPipelineCacheHit:
    """Cache hit: same key returns cached pipeline without reloading."""

    def test_cache_hit_returns_cached(self):
        """First call invokes loader; second call returns the same object."""
        pipeline_a = object()
        loader = MagicMock(return_value=pipeline_a)

        cache = PipelineCache(max_entries=4)

        # First call — cache miss.
        result = cache.get_or_load("model-a", "bf16", loader)
        assert result is pipeline_a
        loader.assert_called_once()
        assert cache.size == 1

        # Second call — cache hit. Loader must NOT be called again.
        loader.reset_mock()
        result2 = cache.get_or_load("model-a", "bf16", loader)
        assert result2 is pipeline_a
        loader.assert_not_called()
        assert cache.size == 1


class TestPipelineCacheMiss:
    """Cache miss: new key invokes loader and stores result."""

    def test_cache_miss_invokes_loader(self):
        """Different keys each invoke the loader independently."""
        pipeline_a = object()
        pipeline_b = object()
        loader = MagicMock(side_effect=[pipeline_a, pipeline_b])

        cache = PipelineCache(max_entries=4)

        r1 = cache.get_or_load("model-a", "bf16", loader)
        assert r1 is pipeline_a
        assert cache.size == 1

        r2 = cache.get_or_load("model-b", "bf16", loader)
        assert r2 is pipeline_b
        assert cache.size == 2


class TestPipelineCacheEviction:
    """Eviction: LRU entry is evicted when free VRAM is insufficient."""

    def test_eviction_on_vram_pressure(self):
        """When free VRAM < estimate, LRU entry is evicted."""
        # The eviction loop calls _free_vram_mib() at the start of each
        # iteration to check if enough headroom was reclaimed after
        # empty_cache().  We use a call counter so the side_effect can
        # return low VRAM on the first call (triggering eviction) and
        # high VRAM on subsequent calls (simulating that empty_cache
        # freed enough memory to stop the loop).
        call_count = 0

        def _free_vram_side_effect() -> int:
            nonlocal call_count
            call_count += 1
            if call_count <= 1:
                return 100  # Low — triggers eviction.
            return 8192  # After empty_cache, VRAM is freed.

        with patch(
            "worker.pipeline_cache._free_vram_mib",
            side_effect=_free_vram_side_effect,
        ), patch(
            "worker.pipeline_cache._estimate_vram_mib",
            return_value=2000,
        ):
            cache = PipelineCache(max_entries=2)

            p1, p2, p3 = object(), object(), object()

            # Fill cache to capacity — free VRAM is high so no eviction.
            cache.get_or_load("m1", "bf16", lambda: p1)
            cache.get_or_load("m2", "bf16", lambda: p2)
            assert cache.size == 2

            # Reset counter so the next call returns 100 (triggers eviction).
            call_count = 0

            # Load m3 — should evict m1 (LRU) because free=100 < est=2000.
            # After empty_cache(), free VRAM jumps to 8192 so eviction stops.
            cache.get_or_load("m3", "bf16", lambda: p3)
            assert cache.size == 2

            # m1 should be gone, m2 and m3 should remain.
            assert ("m1", "bf16") not in cache._cache
            assert ("m2", "bf16") in cache._cache
            assert ("m3", "bf16") in cache._cache


# ── OOM trap tests ──────────────────────────────────────────────────────────────


class TestOomTrap:
    """OOM trap: torch.cuda.OutOfMemoryError is caught before generic Exception."""

    def test_oom_trap_emits_failed(self, mock_torch):
        """A node raising torch.cuda.OutOfMemoryError emits
        Failed{error: 'cuda_oom'} and the worker stays alive."""

        class OomNode(BaseNode):
            NODE_TYPE = "OomNode"

            def execute(self, **inputs: object) -> dict[str, object]:
                raise mock_torch.cuda.OutOfMemoryError("CUDA out of memory")

        register(OomNode)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        nodes = [{"type": "OomNode", "id": "oom", "inputs": {}}]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        # Patch the executor module's torch and _mock so the OOM trap
        # is active (torch is not None).
        with patch.dict(sys.modules, {"torch": mock_torch}):
            import worker.executor as exec_mod

            original_torch = exec_mod.torch
            original_mock = exec_mod._mock
            exec_mod.torch = mock_torch
            exec_mod._mock = False

        try:
            result = run_graph(
                graph=graph,
                settings={},
                device_str="cpu",
                cancel_flag=cancel_flag,
                emit_fn=emit_fn,
                pipeline_cache=PipelineCache(),
                job_id="oom-test",
            )

            assert result["status"] == "failed"
            assert result["error"] == "cuda_oom"
            assert "traceback" in result

            failed_events = [f for f in emitted if f["_type"] == "Failed"]
            assert len(failed_events) == 1
            assert failed_events[0]["error"] == "cuda_oom"
            assert failed_events[0]["job_id"] == "oom-test"

            # No Completed event.
            assert not any(f["_type"] == "Completed" for f in emitted)
        finally:
            # Restore original module state.
            exec_mod.torch = original_torch
            exec_mod._mock = original_mock

    def test_oom_trap_skipped_in_mock(self):
        """When torch is None (mock mode), OOM error falls through to
        general exception handler — error is NOT 'cuda_oom'."""

        # In mock mode, torch is None in executor.py. We simulate a
        # RuntimeError with "CUDA out of memory" in the message.
        # The executor should NOT treat it as cuda_oom.
        class FakeOomNode(BaseNode):
            NODE_TYPE = "FakeOom"

            def execute(self, **inputs: object) -> dict[str, object]:
                raise RuntimeError("CUDA out of memory")

        register(FakeOomNode)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        nodes = [{"type": "FakeOom", "id": "fake", "inputs": {}}]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        # Ensure executor sees torch as None (mock mode).
        import worker.executor as exec_mod

        original_torch = exec_mod.torch
        original_mock = exec_mod._mock
        exec_mod.torch = None  # type: ignore[assignment]
        exec_mod._mock = True

        try:
            result = run_graph(
                graph=graph,
                settings={},
                device_str="cpu",
                cancel_flag=cancel_flag,
                emit_fn=emit_fn,
                pipeline_cache=PipelineCache(),
                job_id="mock-oom-test",
            )

            assert result["status"] == "failed"
            # Should NOT be cuda_oom — falls through to general handler.
            assert result["error"] == "CUDA out of memory"
            assert "traceback" in result

            failed_events = [f for f in emitted if f["_type"] == "Failed"]
            assert len(failed_events) == 1
            assert failed_events[0]["error"] == "CUDA out of memory"
        finally:
            exec_mod.torch = original_torch
            exec_mod._mock = original_mock
