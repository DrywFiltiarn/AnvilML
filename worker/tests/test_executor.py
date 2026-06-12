"""Tests for :mod:`worker.executor`."""

from __future__ import annotations

import threading
import time

import pytest

from worker.executor import CancelledError, run_graph
from worker.nodes.base import NODE_REGISTRY, BaseNode, register


@pytest.fixture(autouse=True)
def _clear_registry() -> None:
    """Ensure NODE_REGISTRY is empty before each test."""
    NODE_REGISTRY.clear()


class TestValidGraph:
    """Tests for successful graph execution."""

    def test_progress_completed_and_edge_resolution(self):
        """run_graph executes nodes in topo order, emits Progress per node,
        resolves edge refs, and emits Completed with elapsed_ms >= 0."""

        class NodeA(BaseNode):
            NODE_TYPE = "NodeA"

            def execute(self, **inputs: object) -> dict[str, object]:
                return {"out_a": "value_a"}

        class NodeB(BaseNode):
            NODE_TYPE = "NodeB"

            def execute(self, **inputs: object) -> dict[str, object]:
                # Verify edge ref was resolved.
                assert inputs.get("from_a") == "value_a"
                return {"out_b": "value_b"}

        register(NodeA)
        register(NodeB)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        nodes = [
            {"type": "NodeA", "id": "a", "inputs": {}},
            {
                "type": "NodeB",
                "id": "b",
                "inputs": {"from_a": {"node_id": "a", "output_slot": "out_a"}},
            },
        ]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        result = run_graph(
            graph=graph,
            settings={},
            device_str="cpu",
            cancel_flag=cancel_flag,
            emit_fn=emit_fn,
            pipeline_cache=None,
            job_id="valid-test",
        )

        assert result["status"] == "completed"
        assert result["elapsed_ms"] >= 0

        # Two Progress events (one per node) in topo order.
        progress_events = [f for f in emitted if f["_type"] == "Progress"]
        assert len(progress_events) == 2
        assert progress_events[0]["node_type"] == "NodeA"
        assert progress_events[0]["node_index"] == 0
        assert progress_events[1]["node_type"] == "NodeB"
        assert progress_events[1]["node_index"] == 1

        # One Completed event.
        completed_events = [f for f in emitted if f["_type"] == "Completed"]
        assert len(completed_events) == 1
        assert completed_events[0]["job_id"] == "valid-test"
        assert completed_events[0]["elapsed_ms"] >= 0

        # No Failed or Cancelled events.
        assert not any(f["_type"] == "Failed" for f in emitted)
        assert not any(f["_type"] == "Cancelled" for f in emitted)


class TestCycleDetected:
    """Tests for cycle detection in the topological sort."""

    def test_cycle_emits_failed(self):
        """Graph with circular edge refs (A→B→A) emits Failed{error:
        'cycle_detected'} and no Completed."""

        class NodeA(BaseNode):
            NODE_TYPE = "CycleA"

            def execute(self, **inputs: object) -> dict[str, object]:
                return {}

        class NodeB(BaseNode):
            NODE_TYPE = "CycleB"

            def execute(self, **inputs: object) -> dict[str, object]:
                return {}

        register(NodeA)
        register(NodeB)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        nodes = [
            {
                "type": "CycleA",
                "id": "a",
                "inputs": {"from_b": {"node_id": "b", "output_slot": "out"}},
            },
            {
                "type": "CycleB",
                "id": "b",
                "inputs": {"from_a": {"node_id": "a", "output_slot": "out"}},
            },
        ]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        result = run_graph(
            graph=graph,
            settings={},
            device_str="cpu",
            cancel_flag=cancel_flag,
            emit_fn=emit_fn,
            pipeline_cache=None,
            job_id="cycle-test",
        )

        assert result["status"] == "failed"
        assert result["error"] == "cycle_detected"

        # Failed event emitted.
        failed_events = [f for f in emitted if f["_type"] == "Failed"]
        assert len(failed_events) == 1
        assert failed_events[0]["error"] == "cycle_detected"

        # No Completed or Progress events.
        assert not any(f["_type"] == "Completed" for f in emitted)
        assert not any(f["_type"] == "Progress" for f in emitted)


class TestNodeException:
    """Tests for exception handling during node execution."""

    def test_exception_emits_failed(self):
        """One mock node raises RuntimeError('boom'); verify Failed event
        is emitted with error and traceback, and no Completed."""

        class FailingNode(BaseNode):
            NODE_TYPE = "Failing"

            def execute(self, **inputs: object) -> dict[str, object]:
                raise RuntimeError("boom")

        class GoodNode(BaseNode):
            NODE_TYPE = "Good"

            def execute(self, **inputs: object) -> dict[str, object]:
                return {"ok": True}

        register(FailingNode)
        register(GoodNode)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        nodes = [
            {"type": "Good", "id": "good", "inputs": {}},
            {"type": "Failing", "id": "fail", "inputs": {}},
        ]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        result = run_graph(
            graph=graph,
            settings={},
            device_str="cpu",
            cancel_flag=cancel_flag,
            emit_fn=emit_fn,
            pipeline_cache=None,
            job_id="exception-test",
        )

        assert result["status"] == "failed"
        assert result["error"] == "boom"
        assert "traceback" in result
        assert "boom" in result["traceback"]

        # Progress for the first node, then Failed.
        progress_events = [f for f in emitted if f["_type"] == "Progress"]
        assert len(progress_events) == 1
        assert progress_events[0]["node_type"] == "Good"

        failed_events = [f for f in emitted if f["_type"] == "Failed"]
        assert len(failed_events) == 1
        assert failed_events[0]["error"] == "boom"

        # No Completed event.
        assert not any(f["_type"] == "Completed" for f in emitted)


class TestCancelDuringExecution:
    """Tests for cooperative cancellation during graph execution."""

    def test_cancel_emits_cancelled_and_skips_remaining(self):
        """Cancel flag set between node executions emits Cancelled, no
        Completed, and subsequent nodes are not started."""

        executed_nodes: list[str] = []

        class BlockingNode(BaseNode):
            """Node that blocks until the cancel flag is set."""

            NODE_TYPE = "Blocking"

            def execute(self, **inputs: object) -> dict[str, object]:
                executed_nodes.append("blocking")
                while not self.ctx.cancel_flag.is_set():
                    time.sleep(0.01)
                raise CancelledError("cancelled by flag")

        class FastNode(BaseNode):
            NODE_TYPE = "Fast"

            def execute(self, **inputs: object) -> dict[str, object]:
                executed_nodes.append("fast")
                return {"done": True}

        register(BlockingNode)
        register(FastNode)

        emitted: list[dict] = []

        def emit_fn(frame: dict) -> None:
            emitted.append(frame)

        # Fast is first, Blocking is second.
        nodes = [
            {"type": "Fast", "id": "fast", "inputs": {}},
            {"type": "Blocking", "id": "block", "inputs": {}},
        ]
        graph = {"nodes": nodes}
        cancel_flag = threading.Event()

        # Start execution in a separate thread so we can cancel mid-way.
        result_holder: dict = {}

        def run() -> None:
            result_holder["result"] = run_graph(
                graph=graph,
                settings={},
                device_str="cpu",
                cancel_flag=cancel_flag,
                emit_fn=emit_fn,
                pipeline_cache=None,
                job_id="cancel-test",
            )

        t = threading.Thread(target=run, daemon=True)
        t.start()

        # Wait for Fast node to complete.
        while len(executed_nodes) < 1:
            time.sleep(0.01)

        # Now cancel — Blocking node is blocked in its execute loop.
        cancel_flag.set()

        # Wait for run_graph to return (CancelledError is caught).
        t.join(timeout=5)
        assert t.is_alive() is False, "run_graph did not return after cancel"

        result = result_holder["result"]
        assert result["status"] == "cancelled"

        # Fast was executed; Blocking started but was cancelled mid-way.
        assert "fast" in executed_nodes

        # Progress for Fast, then Cancelled (no Progress for Blocking).
        progress_events = [f for f in emitted if f["_type"] == "Progress"]
        assert len(progress_events) == 1
        assert progress_events[0]["node_type"] == "Fast"

        cancelled_events = [f for f in emitted if f["_type"] == "Cancelled"]
        assert len(cancelled_events) == 1
        assert cancelled_events[0]["job_id"] == "cancel-test"

        # No Completed event.
        assert not any(f["_type"] == "Completed" for f in emitted)
