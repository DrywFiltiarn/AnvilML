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
