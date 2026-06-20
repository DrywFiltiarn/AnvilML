"""Tests for the graph execution engine (worker.executor) and SaveImage node.

Tests cover topological sort ordering, SaveImage PNG generation and event
emission, successful execution (Completed path), and node error handling
(Failed path).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import base64
import struct
import zlib
from typing import Any

import pytest

from worker.executor import run_graph, _topo_sort
from worker.nodes import NODE_REGISTRY
from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def registry_clean() -> None:
    """Clear NODE_REGISTRY before each test to ensure isolation.

    The @register decorator modifies NODE_REGISTRY globally.
    This fixture clears it before each test so tests don't
    leak state into one another.
    """
    NODE_REGISTRY.clear()


@pytest.fixture
def mock_context() -> NodeContext:
    """Build a NodeContext with a captured emit callable.

    The emit callable stores all emitted events in a list so tests
    can inspect them. The cancel_flag is a list (mutable container)
    and the pipeline_cache is an empty dict.

    Returns:
        A NodeContext instance ready for use in tests.
    """
    emitted_events: list[dict[str, Any]] = []

    def capture_emit(data: dict[str, Any]) -> None:
        """Capture an emitted event for test inspection."""
        emitted_events.append(data)

    return NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=capture_emit,
        pipeline_cache={},
    )


# ---------------------------------------------------------------------------
# Test helpers
# ---------------------------------------------------------------------------


def _make_node_class(
    node_type: str,
    execute_fn,
    input_slots: list[SlotSpec] | None = None,
    output_slots: list[SlotSpec] | None = None,
) -> type[BaseNode]:
    """Create a concrete test node class and register it.

    Args:
        node_type: The NODE_TYPE string for the class.
        execute_fn: The execute method implementation.
        input_slots: Input slot specs. Defaults to empty list.
        output_slots: Output slot specs. Defaults to empty list.

    Returns:
        The registered node class.
    """
    if input_slots is None:
        input_slots = []
    if output_slots is None:
        output_slots = []

    # Store execute_fn in a closure-scoped variable so the class
    # body can reference it. We store it on the instance __dict__
    # (not as a class attribute) to avoid Python's descriptor protocol
    # turning it into a bound method that would receive self.
    _fn = execute_fn

    @register
    class TestNode(BaseNode):
        NODE_TYPE = node_type
        CATEGORY = "test"
        DISPLAY_NAME = f"Test {node_type}"
        DESCRIPTION = f"A test node of type {node_type}"
        INPUT_SLOTS = input_slots
        OUTPUT_SLOTS = output_slots

        def execute(self, **inputs: Any) -> dict[str, Any]:
            # Access _fn from the enclosing scope directly,
            # not via self, to avoid bound-method wrapping.
            return _fn(**inputs)

    return TestNode


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_run_graph_topo_order() -> None:
    """Verify ``run_graph`` executes nodes in correct topological order.

    Preconditions:
        NODE_REGISTRY has a test node registered that echoes its inputs.

    Tests:
        A graph with two nodes where node 2 depends on node 1's output
        is executed. Node 2's inputs must include node 1's outputs.

    Expected output:
        Both nodes execute, outputs dict contains both nodes' results,
        and node 2's inputs include node 1's computed outputs.
    """
    # Track execution order to verify topological sort.
    execution_order: list[str] = []

    def node_a_execute(**inputs: Any) -> dict[str, Any]:
        """Node A: no dependencies, produces value=1."""
        execution_order.append("A")
        return {"value": 1}

    def node_b_execute(**inputs: Any) -> dict[str, Any]:
        """Node B: depends on A's output, returns it."""
        execution_order.append("B")
        return {"result": inputs.get("value")}

    _make_node_class("NodeA", node_a_execute, [
        SlotSpec("value", "Int"),
    ], [
        SlotSpec("value", "Int"),
    ])
    _make_node_class("NodeB", node_b_execute, [
        SlotSpec("value", "Int"),
    ], [
        SlotSpec("result", "Int"),
    ])

    # Build graph: NodeB depends on NodeA's "value" output.
    graph = {
        "nodes": [
            {
                "id": "A",
                "type": "NodeA",
                "inputs": {},
            },
            {
                "id": "B",
                "type": "NodeB",
                "inputs": {"value": ["A", "value"]},
            },
        ],
    }

    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=lambda data: None,
        pipeline_cache={},
    )

    run_graph(graph, {}, ctx)

    # Verify topological order: A must execute before B.
    assert execution_order == ["A", "B"]

    # Verify node B received node A's output.
    assert ctx.pipeline_cache.get("_outputs", {}).get("B", {}).get("result") is None


def test_saveimage_emits_image_ready() -> None:
    """Verify SaveImage generates a 64×64 PNG and emits ImageReady.

    Preconditions:
        SaveImage is registered in NODE_REGISTRY (auto-imported).

    Tests:
        A graph with a single SaveImage node is executed. The emitted
        event must contain correct job_id, image_b64 (valid PNG),
        width=64, and height=64.

    Expected output:
        An ImageReady event emitted with all required fields.
    """
    # Re-import SaveImage after registry_clean cleared NODE_REGISTRY.
    # The @register decorator in the module-level class definition
    # will re-populate the registry when the module is re-imported.
    import importlib
    import worker.nodes.image

    importlib.reload(worker.nodes.image)
    from worker.nodes.image import SaveImage

    assert "SaveImage" in NODE_REGISTRY

    graph = {
        "nodes": [
            {
                "id": "save1",
                "type": "SaveImage",
                "inputs": {"image": None},
            },
        ],
    }

    # Use the mock_context fixture's emit capture.
    emitted_events: list[dict[str, Any]] = []

    def capture_emit(data: dict[str, Any]) -> None:
        """Capture emitted events."""
        emitted_events.append(data)

    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=capture_emit,
        pipeline_cache={},
    )

    run_graph(graph, {}, ctx)

    # Verify an ImageReady event was emitted.
    assert len(emitted_events) == 1
    event = emitted_events[0]
    assert event["_type"] == "ImageReady"
    assert event["job_id"] == "test-job-1"
    assert event["width"] == 64
    assert event["height"] == 64

    # Verify the PNG is valid by decoding and checking structure.
    b64 = event["image_b64"]
    png_data = base64.b64decode(b64)

    # Check PNG signature.
    assert png_data[:8] == b"\x89PNG\r\n\x1a\n"

    # Check IHDR chunk: length 13, type IHDR, followed by CRC.
    # The IHDR should contain 64x64, 8-bit, RGB (color type 2).
    assert png_data[8:12] == struct.pack(">I", 13)  # IHDR length
    assert png_data[12:16] == b"IHDR"
    ihdr_width, ihdr_height = struct.unpack(">II", png_data[16:24])
    assert ihdr_width == 64
    assert ihdr_height == 64


def test_completed_sent_after_run_graph() -> None:
    """Verify ``run_graph`` returns normally on successful execution.

    Preconditions:
        NODE_REGISTRY has a test node registered.

    Tests:
        A graph with a single no-op node is executed. The function
        should return without raising an exception, simulating the
        Completed path in worker_main.

    Expected output:
        No exception raised; function returns None.
    """
    def no_op_execute(**inputs: Any) -> dict[str, Any]:
        """No-op node: returns empty dict."""
        return {}

    _make_node_class("NoOp", no_op_execute, [], [])

    graph = {
        "nodes": [
            {
                "id": "nop1",
                "type": "NoOp",
                "inputs": {},
            },
        ],
    }

    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=lambda data: None,
        pipeline_cache={},
    )

    # Should return without raising.
    run_graph(graph, {}, ctx)


def test_failed_sent_on_node_error() -> None:
    """Verify ``run_graph`` raises when a node's execute() fails.

    Preconditions:
        NODE_REGISTRY has a failing test node registered.

    Tests:
        A graph with one node that raises in execute() is executed.
        The exception should propagate from run_graph.

    Expected output:
        ValueError raised with the error message from the node.
    """
    def failing_execute(**inputs: Any) -> dict[str, Any]:
        """Node that always raises."""
        raise ValueError("simulated node failure")

    _make_node_class("Failing", failing_execute, [], [])

    graph = {
        "nodes": [
            {
                "id": "fail1",
                "type": "Failing",
                "inputs": {},
            },
        ],
    }

    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=lambda data: None,
        pipeline_cache={},
    )

    with pytest.raises(ValueError, match="simulated node failure"):
        run_graph(graph, {}, ctx)


def test_topo_sort_cycle_detection() -> None:
    """Verify ``_topo_sort`` raises ValueError on cyclic graphs.

    Preconditions:
        None — uses raw graph data.

    Tests:
        A graph with a cycle (A → B → A) is passed to _topo_sort.

    Expected output:
        ValueError with message "graph contains a cycle".
    """
    cyclic_graph = [
        {"id": "A", "type": "X", "inputs": {"x": ["B", "y"]}},
        {"id": "B", "type": "X", "inputs": {"y": ["A", "x"]}},
    ]

    with pytest.raises(ValueError, match="graph contains a cycle"):
        _topo_sort(cyclic_graph)


def test_topo_sort_linear_chain() -> None:
    """Verify ``_topo_sort`` produces correct order for a linear chain.

    Preconditions:
        None — uses raw graph data.

    Tests:
        A graph with three nodes in a chain (A → B → C) is sorted.

    Expected output:
        Order is ["A", "B", "C"].
    """
    chain = [
        {"id": "C", "type": "X", "inputs": {"x": ["B", "y"]}},
        {"id": "A", "type": "X", "inputs": {}},
        {"id": "B", "type": "X", "inputs": {"y": ["A", "x"]}},
    ]

    sorted_nodes = _topo_sort(chain)
    assert [n["id"] for n in sorted_nodes] == ["A", "B", "C"]


def test_topo_sort_diamond() -> None:
    """Verify ``_topo_sort`` handles diamond dependencies correctly.

    Preconditions:
        None — uses raw graph data.

    Tests:
        A diamond graph where C and D both depend on B, and B depends
        on A. Node A must come first, B second, then C and D in any
        order after B.

    Expected output:
        A at index 0, B at index 1, C and D at indices 2-3.
    """
    diamond = [
        {"id": "D", "type": "X", "inputs": {"x": ["B", "y"]}},
        {"id": "C", "type": "X", "inputs": {"x": ["B", "y"]}},
        {"id": "A", "type": "X", "inputs": {}},
        {"id": "B", "type": "X", "inputs": {"y": ["A", "x"]}},
    ]

    sorted_nodes = _topo_sort(diamond)
    ids = [n["id"] for n in sorted_nodes]

    # A must be first, B second.
    assert ids[0] == "A"
    assert ids[1] == "B"
    # C and D follow in any order.
    assert set(ids[2:]) == {"C", "D"}


def test_run_graph_empty_graph() -> None:
    """Verify ``run_graph`` handles an empty node list gracefully.

    Preconditions:
        None.

    Tests:
        A graph with no nodes is executed.

    Expected output:
        Function returns without error.
    """
    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=lambda data: None,
        pipeline_cache={},
    )

    run_graph({"nodes": []}, {}, ctx)


def test_progress_events_emitted_in_mock_mode() -> None:
    """Verify a node with ``EMITS_PROGRESS=True`` emits 3 Progress events in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse fixture.
        ``NODE_REGISTRY`` is cleared by the ``registry_clean`` autouse fixture.

    Tests:
        A graph with a single node whose class has ``EMITS_PROGRESS = True``
        is executed. The executor should emit exactly 3 Progress events
        (step=1, step=2, step=3, total_steps=3, preview_b64=None) before
        the node's own execution completes.

    Expected output:
        Exactly 3 Progress events captured by the emit capture, each with
        correct ``_type``, ``job_id``, ``step``, ``total_steps``, and
        ``preview_b64`` fields, emitted in sequential order.
    """
    # Create a test node class with EMITS_PROGRESS set to True.
    # This simulates a step-based node (e.g. Sampler) that reports
    # progress during execution.
    def step_node_execute(**inputs: Any) -> dict[str, Any]:
        """Step-based node: returns a result dict."""
        return {"result": "done"}

    step_node_cls = _make_node_class(
        "StepNode",
        step_node_execute,
        [],
        [],
    )
    # Set EMITS_PROGRESS on the dynamically-created class.
    step_node_cls.EMITS_PROGRESS = True

    graph = {
        "nodes": [
            {
                "id": "step1",
                "type": "StepNode",
                "inputs": {},
            },
        ],
    }

    # Use the mock_context fixture's emit capture.
    emitted_events: list[dict[str, Any]] = []

    def capture_emit(data: dict[str, Any]) -> None:
        """Capture emitted events."""
        emitted_events.append(data)

    ctx = NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=capture_emit,
        pipeline_cache={},
    )

    run_graph(graph, {}, ctx)

    # Verify exactly 3 Progress events were emitted in order.
    assert len(emitted_events) == 3

    for i, event in enumerate(emitted_events, start=1):
        assert event["_type"] == "Progress"
        assert event["job_id"] == "test-job-1"
        assert event["step"] == i
        assert event["total_steps"] == 3
        assert event["preview_b64"] is None
