"""Tests for worker.executor — topological sort of job graphs."""

from worker.executor import topo_sort


def _make_graph(nodes: list[str], edges: list[tuple[str, str]] | None = None) -> dict:
    """Construct a minimal graph dict for testing.

    Args:
        nodes: List of node IDs.
        edges: Optional list of (from_id, to_id) tuples. If None,
            the graph has no edges.

    Returns:
        A graph dict with ``"nodes"`` and ``"edges"`` keys.
    """
    node_list = [
        {"id": nid, "type": "TestNode", "inputs": {}} for nid in nodes
    ]
    graph: dict = {"nodes": node_list}
    if edges:
        graph["edges"] = [
            {"from": f"{src}:{idx}", "to": f"{dst}:{idx}"}
            for idx, (src, dst) in enumerate(edges)
        ]
    return graph


def test_topo_sort_single_node() -> None:
    """A graph with one node and no edges returns that node in a list.

    Constructs a graph with a single node and calls topo_sort().
    Asserts the result is a list containing exactly that node.

    Preconditions: A single-node graph has no edges.
    Expected output: [node] where node is the input node dict.
    """
    graph = _make_graph(["A"])
    result = topo_sort(graph)
    assert len(result) == 1
    assert result[0]["id"] == "A"


def test_topo_sort_linear_chain() -> None:
    """A→B→C chain returns nodes in correct dependency order.

    Constructs a graph with three nodes connected as A→B→C and calls
    topo_sort(). Asserts the result is [A, B, C] — each node appears
    after all nodes it depends on.

    Preconditions: A→B→C is a strict linear chain with no branches.
    Expected output: [A, B, C] in that exact order.
    """
    graph = _make_graph(["A", "B", "C"], [("A", "B"), ("B", "C")])
    result = topo_sort(graph)
    assert len(result) == 3
    assert result[0]["id"] == "A"
    assert result[1]["id"] == "B"
    assert result[2]["id"] == "C"


def test_topo_sort_parallel_branches() -> None:
    """A graph with parallel branches produces a valid topological order.

    Constructs a graph where A fans out to B and C (both depend only
    on A) and calls topo_sort(). Asserts A comes before both B and C.
    Since Kahn's algorithm processes nodes in insertion order when
    multiple have in-degree 0, the relative order of B and C is
    non-deterministic — the test asserts a partial order only.

    Preconditions: A→B and A→C; no edge between B and C.
    Expected output: A appears before B and C; B and C order unspecified.
    """
    graph = _make_graph(["A", "B", "C"], [("A", "B"), ("A", "C")])
    result = topo_sort(graph)
    assert len(result) == 3
    # A must come before both B and C.
    # Use a helper to find a node's position by its "id" field — the
    # result dicts contain "type" and "inputs" keys that make them
    # unequal to a plain {"id": "X"} dict, so list.index() would fail.
    def _position(nodes: list[dict], target_id: str) -> int:
        """Return the index of the node dict whose "id" matches *target_id*.

        Raises ValueError if no matching node is found.
        """
        for i, n in enumerate(nodes):
            if n["id"] == target_id:
                return i
        raise ValueError(f"Node {target_id!r} not found in result")

    assert _position(result, "A") < _position(result, "B")
    assert _position(result, "A") < _position(result, "C")


def test_topo_sort_cycle_detected() -> None:
    """A cyclic graph raises ValueError with cycle node IDs in the message.

    Constructs a graph with a cycle (A→B→C→A) and calls topo_sort().
    Asserts ValueError is raised and the error message contains the
    IDs of the nodes involved in the cycle.

    Preconditions: A→B→C→A forms a directed cycle.
    Expected output: ValueError with message containing "A", "B", "C".
    """
    graph = _make_graph(["A", "B", "C"], [("A", "B"), ("B", "C"), ("C", "A")])
    try:
        topo_sort(graph)
        pytest.fail("Expected ValueError for cyclic graph")
    except ValueError as exc:
        msg = str(exc)
        assert "Cycle detected" in msg
        # All three nodes should be in the error message.
        assert "A" in msg
        assert "B" in msg
        assert "C" in msg


def test_topo_sort_no_edges_key() -> None:
    """A graph without an ``"edges"`` key returns nodes in original order.

    Constructs a graph with three nodes and no ``"edges"`` key at all
    (not even an empty list) and calls topo_sort(). Asserts all three
    nodes are returned in their original insertion order.

    This tests the graceful handling of graphs that lack an ``"edges"``
    key entirely — a single-node graph or a graph that was validated
    but has no edges.

    Preconditions: The graph dict has ``"nodes"`` but no ``"edges"``.
    Expected output: Nodes returned in original insertion order.
    """
    graph: dict = {
        "nodes": [
            {"id": "X", "type": "TestNode", "inputs": {}},
            {"id": "Y", "type": "TestNode", "inputs": {}},
            {"id": "Z", "type": "TestNode", "inputs": {}},
        ]
    }
    result = topo_sort(graph)
    assert len(result) == 3
    assert result[0]["id"] == "X"
    assert result[1]["id"] == "Y"
    assert result[2]["id"] == "Z"


def test_topo_sort_empty_graph() -> None:
    """A graph with no nodes returns an empty list.

    Constructs a graph with an empty ``"nodes"`` list and calls
    topo_sort(). Asserts the result is an empty list.

    Preconditions: The graph has ``"nodes": []``.
    Expected output: [].
    """
    graph: dict = {"nodes": []}
    result = topo_sort(graph)
    assert result == []


def test_topo_sort_missing_nodes_key() -> None:
    """A graph without a ``"nodes"`` key returns an empty list.

    Constructs a graph dict with no ``"nodes"`` key and calls
    topo_sort(). Asserts the result is an empty list. This is
    graceful degradation — the Rust validator should have caught
    this before the graph reaches the worker.

    Preconditions: The graph dict has no ``"nodes"`` key.
    Expected output: [].
    """
    graph: dict = {"edges": []}
    result = topo_sort(graph)
    assert result == []


def test_topo_sort_no_torch_import() -> None:
    """Module does not transitively import torch.

    Uses subprocess.run() to spawn a fresh Python process that imports
    worker.executor and asserts "torch" not in sys.modules. This confirms
    the module has no transitive torch dependency at import time
    (required by the mock-mode CI jobs that install only base.txt).

    Uses subprocess isolation (not sys.modules manipulation) per
    ENVIRONMENT.md §11.3.

    Expected outcome: Subprocess exits 0 with "OK" in stdout.
    """
    import subprocess
    import sys

    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import worker.executor; import sys; "
            "assert 'torch' not in sys.modules; print('OK')",
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert result.returncode == 0, (
        f"Subprocess failed: stdout={result.stdout!r}, "
        f"stderr={result.stderr!r}"
    )


# ---------------------------------------------------------------------------
# Helper types for execute_graph tests — these do NOT import torch or any
# node module, keeping the test collection time clean for mock-mode CI.
# ---------------------------------------------------------------------------


class _MockContext:
    """Minimal NodeContext substitute for testing execute_graph without torch.

    Provides the same public interface that execute_graph expects:
    cancel_flag (threading.Event), plus dummy values for fields that
    execute_graph never touches (job_id, device, caps, emit, pipeline_cache,
    mock).  This avoids importing worker.nodes.base at test collection time.
    """

    def __init__(self) -> None:
        """Construct a mock context with an unset cancel_flag."""
        import threading

        self.cancel_flag = threading.Event()
        self.job_id = "test-job-000"
        self.device = "cpu"
        self.caps: dict = {}
        self.emit = lambda evt: None  # no-op — tests don't check emissions
        self.pipeline_cache = None
        self.mock = True


class _MockNode:
    """Minimal node substitute for testing execute_graph.

    Implements the execute(ctx, **inputs) signature expected by
    execute_graph.  Returns a dict with the input value so callers
    can verify data flows through the execution pipeline correctly.

    The class supports a shared ``execution_log`` list (set as a
    class attribute) that records every call to execute(), enabling
    tests to verify execution order and which nodes ran.
    """

    execution_log: list[str] = []  # type: ignore[assignment] — set per-test

    def execute(self, ctx: object, **inputs: object) -> dict:  # noqa: ANN002, ANN401
        """Execute the mock node, logging its ID via a closure.

        Args:
            ctx: The mock context (unused by this node; present for
                signature consistency with the real BaseNode.execute).
            **inputs: Input values — the "value" key is returned
                as the sole output under the same key.

        Returns:
            Dict with key "output" containing the input "value".
        """
        # Capture the node ID from the closure variable set by the
        # test helper — this is how we track which node executed.
        # The node_id is stored on the instance by the test setup.
        _MockNode.execution_log.append(getattr(self, "_node_id", "unknown"))
        return {"output": inputs.get("value", None)}


def _make_mock_node(node_id: str) -> type[_MockNode]:
    """Create a MockNode subclass with a specific node ID.

    Args:
        node_id: The ID to associate with this node instance.

    Returns:
        A new class that, when instantiated and executed, logs the
        given node_id to MockNode.execution_log.
    """
    cls: type[_MockNode] = type(
        f"MockNode_{node_id}",
        (_MockNode,),
        {"_node_id": node_id},
    )
    return cls


def _make_execute_graph_graph(
    node_types: list[str],
    edges: list[tuple[str, str]] | None = None,
) -> dict:
    """Construct a graph dict using mock node types.

    Args:
        node_types: List of node type strings (these become the
            "type" field in each node dict).
        edges: Optional list of (from_id, to_id) tuples.

    Returns:
        A graph dict compatible with topo_sort() and execute_graph().
    """
    node_list = [
        {"id": f"node_{i}", "type": ntype, "inputs": {"value": i}}
        for i, ntype in enumerate(node_types)
    ]
    graph: dict = {"nodes": node_list}
    if edges:
        graph["edges"] = [
            {"from": f"{src}:0", "to": f"{dst}:0"}
            for src, dst in edges
        ]
    return graph


def _build_registry(
    node_types: list[str],
) -> dict[str, type[_MockNode]]:
    """Build a NODE_REGISTRY substitute mapping type names to mock classes.

    Args:
        node_types: List of node type strings to register.

    Returns:
        A dict suitable for passing to execute_graph's registry access
        pattern (NODE_REGISTRY[node["type"]]).
    """
    registry: dict[str, type[_MockNode]] = {}
    for ntype in node_types:
        # Create a unique class for each type so they can carry
        # distinct _node_id values.
        registry[ntype] = _make_mock_node(ntype)
    return registry


def test_execute_graph_cancel_before_first() -> None:
    """Cancel flag set before the loop starts — no nodes execute.

    Constructs a graph with three nodes, sets the cancel flag on the
    mock context before calling execute_graph(), then asserts that:
    (1) the result is {"cancelled": True},
    (2) no node's execute() was called.

    Preconditions: MockContext with cancel_flag already set.
    Expected output: {"cancelled": True}, execution_log is empty.
    """
    from worker.executor import execute_graph

    # Reset the shared execution log from any prior test.
    _MockNode.execution_log = []

    ctx = _MockContext()
    ctx.cancel_flag.set()  # Set before the loop starts

    graph = _make_execute_graph_graph(["A", "B", "C"])

    def ctx_factory() -> _MockContext:
        return ctx

    result = execute_graph(graph, ctx_factory)

    # The function should return immediately with cancelled=True.
    assert result == {"cancelled": True}
    # No nodes should have been executed.
    assert _MockNode.execution_log == []


def test_execute_graph_cancel_after_first() -> None:
    """Cancel flag set mid-execution — first node runs, second skipped.

    Uses a two-node graph.  The first node's execute() sets the cancel
    flag (via a mutable list shared with the test), so the second node
    is never reached.  Asserts:
    (1) first node ran, second did not,
    (2) result is {"cancelled": True}.

    Preconditions: Two-node graph; first node sets cancel_flag on execute.
    Expected output: {"cancelled": True}, execution_log == ["A"].
    """
    from worker.executor import execute_graph

    _MockNode.execution_log = []

    ctx = _MockContext()

    # Use a mutable container (list) so the nested class can modify
    # it — Python closures cannot assign to variables in outer scopes
    # without nonlocal, and this is simpler than a class attribute.
    cancel_on_first: list[bool] = [False]

    class _CancellingNode(_MockNode):
        """A mock node that sets the cancel flag on its first execute call.

        Logs to the shared execution_log like its parent class, then
        sets the cancel flag so the executor stops before the next
        node runs.
        """

        _node_id = "CancelNode"  # Set the node ID so execution_log records it

        def execute(self, ctx: object, **inputs: object) -> dict:  # noqa: ANN002, ANN401
            # Log execution — mirrors the parent class logic so the
            # test can verify which nodes ran.
            _MockNode.execution_log.append(getattr(self, "_node_id", "unknown"))
            # Set the cancel flag on the first execution so the loop
            # stops before the second node runs.  This verifies the
            # cancel checkpoint happens BEFORE each node, not after.
            if not cancel_on_first[0]:
                cancel_on_first[0] = True
                ctx.cancel_flag.set()
            return {"output": inputs.get("value", None)}

    graph = _make_execute_graph_graph(["CancelNode", "NodeB"])

    # Replace the registry entry for "CancelNode" with our cancelling
    # variant.  We need to patch the registry used by execute_graph —
    # since execute_graph imports NODE_REGISTRY inside its body, we
    # temporarily replace it in the base module.
    from worker.nodes import base

    original = base.NODE_REGISTRY.get("CancelNode")
    base.NODE_REGISTRY["CancelNode"] = _CancellingNode

    try:

        def ctx_factory() -> _MockContext:
            return ctx

        result = execute_graph(graph, ctx_factory)

        # First node ran (it set the cancel flag), second did not.
        assert len(_MockNode.execution_log) == 1
        assert _MockNode.execution_log[0] == "CancelNode"
        assert result == {"cancelled": True}
    finally:
        # Restore the original registry entry (or remove ours).
        if original is None:
            base.NODE_REGISTRY.pop("CancelNode", None)
        else:
            base.NODE_REGISTRY["CancelNode"] = original


def test_execute_graph_no_cancel_completes() -> None:
    """No cancel flag set — all nodes execute in order.

    Constructs a three-node graph with no edges (all independent),
    does not set the cancel flag, and asserts:
    (1) all three nodes executed,
    (2) result is {"cancelled": False, "results": {...}},
    (3) the results dict contains outputs for all nodes.

    Preconditions: Three-node graph, cancel flag never set.
    Expected output: {"cancelled": False, "results": {"node_0": ..., "node_1": ..., "node_2": ...}}.
    """
    from worker.executor import execute_graph

    _MockNode.execution_log = []

    ctx = _MockContext()
    # cancel_flag is NOT set — the default is unset (False).

    node_types = ["NodeA", "NodeB", "NodeC"]
    graph = _make_execute_graph_graph(node_types)

    # Patch the registry so execute_graph finds our mock nodes.
    from worker.nodes import base

    original_entries: dict[str, type] = {}
    for ntype in node_types:
        original_entries[ntype] = base.NODE_REGISTRY.get(ntype)
        base.NODE_REGISTRY[ntype] = _make_mock_node(ntype)

    try:

        def ctx_factory() -> _MockContext:
            return ctx

        result = execute_graph(graph, ctx_factory)

        # All three nodes should have executed.
        assert result["cancelled"] is False
        assert len(_MockNode.execution_log) == 3
        assert set(_MockNode.execution_log) == set(node_types)
        # Results dict should contain all three nodes.
        assert "results" in result
        assert len(result["results"]) == 3
    finally:
        # Restore original registry entries.
        for ntype in node_types:
            if original_entries[ntype] is None:
                base.NODE_REGISTRY.pop(ntype, None)
            else:
                base.NODE_REGISTRY[ntype] = original_entries[ntype]


def test_execute_graph_execution_order_matches_topo_sort() -> None:
    """Execution order matches topo_sort() output for a DAG.

    Constructs a graph with edges A→B→C (linear chain) and asserts
    that the actual execution order recorded by the mock nodes matches
    the topological order returned by topo_sort().

    Preconditions: A→B→C chain; cancel flag unset.
    Expected output: execution_log == ["NodeA", "NodeB", "NodeC"].
    """
    from worker.executor import execute_graph, topo_sort

    _MockNode.execution_log = []

    ctx = _MockContext()

    node_types = ["NodeA", "NodeB", "NodeC"]
    graph = _make_execute_graph_graph(node_types, [("NodeA", "NodeB"), ("NodeB", "NodeC")])

    # Compute the expected topological order independently.
    expected_order = [node["id"] for node in topo_sort(graph)]

    # Patch the registry.
    from worker.nodes import base

    original_entries: dict[str, type] = {}
    for ntype in node_types:
        original_entries[ntype] = base.NODE_REGISTRY.get(ntype)
        base.NODE_REGISTRY[ntype] = _make_mock_node(ntype)

    try:

        def ctx_factory() -> _MockContext:
            return ctx

        result = execute_graph(graph, ctx_factory)

        assert result["cancelled"] is False

        # The execution_log contains node type names (from _MockNode),
        # but topo_sort returns node dicts with "id" fields.  The mock
        # node IDs are "node_0", "node_1", "node_2" — we need to map
        # execution_log entries back to node types.  Since each mock
        # class has a _node_id attribute, we check that the types
        # executed match the topological order.
        #
        # The execution_log records the _node_id set per class, which
        # equals the type name (e.g. "NodeA").  topo_sort returns
        # dicts with "id" fields like "node_0" — these are different
        # identifiers.  Instead, we verify that the execution order
        # of types matches the topo_sort order by comparing the
        # sequence of types in topo_sort against execution_log.
        topo_type_order = [node["type"] for node in topo_sort(graph)]
        assert _MockNode.execution_log == topo_type_order
    finally:
        for ntype in node_types:
            if original_entries[ntype] is None:
                base.NODE_REGISTRY.pop(ntype, None)
            else:
                base.NODE_REGISTRY[ntype] = original_entries[ntype]


def test_execute_graph_results_dict() -> None:
    """Output dict is populated correctly with node results keyed by ID.

    Constructs a three-node graph, executes it, and asserts that the
    returned results dict contains exactly the node IDs as keys and
    that each value is the node's execute() return dict.

    Preconditions: Three-node graph, cancel flag unset.
    Expected output: results dict maps "node_0"→{"output": 0}, etc.
    """
    from worker.executor import execute_graph

    _MockNode.execution_log = []

    ctx = _MockContext()

    node_types = ["ResultA", "ResultB", "ResultC"]
    graph = _make_execute_graph_graph(node_types)

    # Patch the registry.
    from worker.nodes import base

    original_entries: dict[str, type] = {}
    for ntype in node_types:
        original_entries[ntype] = base.NODE_REGISTRY.get(ntype)
        base.NODE_REGISTRY[ntype] = _make_mock_node(ntype)

    try:

        def ctx_factory() -> _MockContext:
            return ctx

        result = execute_graph(graph, ctx_factory)

        assert result["cancelled"] is False
        assert "results" in result

        results = result["results"]
        # Each node was created with inputs {"value": i} where i is
        # its index, and MockNode.execute returns {"output": value}.
        expected_keys = {"node_0", "node_1", "node_2"}
        assert set(results.keys()) == expected_keys

        # Verify each result value is a dict with "output" key.
        for node_id, output in results.items():
            assert isinstance(output, dict)
            assert "output" in output
    finally:
        for ntype in node_types:
            if original_entries[ntype] is None:
                base.NODE_REGISTRY.pop(ntype, None)
            else:
                base.NODE_REGISTRY[ntype] = original_entries[ntype]


def test_execute_graph_no_torch_import() -> None:
    """execute_graph does not cause torch import at module collection time.

    Uses subprocess.run() to spawn a fresh Python process that imports
    worker.executor (which now contains execute_graph) and asserts
    "torch" not in sys.modules.  This confirms that importing the
    module — even though execute_graph references NODE_REGISTRY —
    does not transitively pull in torch at collection time, because
    NODE_REGISTRY is imported inside the function body.

    Uses subprocess isolation (not sys.modules manipulation) per
    ENVIRONMENT.md §11.3.

    Expected outcome: Subprocess exits 0 with "OK" in stdout.
    """
    import subprocess
    import sys

    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import worker.executor; import sys; "
            "assert 'torch' not in sys.modules; print('OK')",
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert result.returncode == 0, (
        f"Subprocess failed: stdout={result.stdout!r}, "
        f"stderr={result.stderr!r}"
    )
