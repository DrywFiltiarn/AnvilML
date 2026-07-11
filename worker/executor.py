"""Graph execution utilities for the AnvilML Python worker.

Provides topological sorting of job graphs via Kahn's algorithm, enabling
correct dependency-ordered node execution. The module operates on raw graph
dicts and does not depend on NodeContext, NODE_REGISTRY, or any node class
at import time — those are imported inside function bodies to avoid
transitive torch dependencies during test collection.
"""

import logging
from collections import defaultdict, deque

logger = logging.getLogger(__name__)


def topo_sort(graph: dict) -> list[dict]:
    """Perform Kahn's-algorithm topological sort on a job graph.

    Accepts a graph dict with the shape ``{"nodes": [...], "edges": [...]}``,
    builds a directed adjacency list from the edges, computes in-degrees for
    all nodes, and returns the nodes in a valid topological order where every
    node appears after all nodes it depends on.

    The ``"from"`` and ``"to"`` edge strings have the format
    ``"node_id:slot_name"`` (e.g. ``"load_model_0:MODEL"``). Only the node
    ID (before the first colon) is extracted; slot names are handled by prior
    validation in ``dag.rs`` and are irrelevant to ordering.

    If the graph has no ``"edges"`` key or an empty edges list, every node
    has in-degree 0 and the result is the nodes in their original insertion
    order. If the graph has no ``"nodes"`` key, an empty list is returned.

    Args:
        graph: A dict with ``"nodes"`` (list of node dicts) and optionally
            ``"edges"`` (list of edge dicts with ``"from"`` and ``"to"``
            string keys).

    Returns:
        A list of node dicts in valid topological order. Returns an empty
        list if the graph has no nodes.

    Raises:
        ValueError: If the graph contains a cycle — the error message
            contains the IDs of the nodes involved in the cycle.
    """
    # Extract nodes list — graceful degradation if key is missing or
    # malformed. The Rust validator should have caught this before the
    # graph reaches the worker, but we handle it here anyway.
    nodes: list[dict] = graph.get("nodes", [])
    if not isinstance(nodes, list):
        return []

    # Build adjacency list and in-degree map from edges. Only the "from"
    # and "to" fields are used; slot names (after the colon) are ignored
    # since they are validated separately by dag.rs.
    adjacency: dict[str, list[str]] = defaultdict(list)
    in_degree: dict[str, int] = {node["id"]: 0 for node in nodes}

    edges = graph.get("edges", [])
    if isinstance(edges, list):
        for edge in edges:
            # Parse "from" and "to" strings — split on first colon only,
            # because a node ID could theoretically contain colons.
            # splitn(2, ':') mirrors the Rust dag.rs pattern (line 178).
            from_id = edge.get("from", "").split(":", 1)[0]
            to_id = edge.get("to", "").split(":", 1)[0]

            # Guard against malformed edge entries with empty IDs.
            # These would be caught by the Rust validator, but we skip
            # them here rather than failing — a missing edge entry
            # shouldn't break execution of a valid graph.
            if not from_id or not to_id:
                continue

            # Avoid duplicate edges: only add if this edge hasn't been
            # recorded yet. Duplicate edges would inflate in-degrees
            # and prevent correct topological ordering.
            if to_id not in adjacency[from_id]:
                adjacency[from_id].append(to_id)
                in_degree[to_id] = in_degree.get(to_id, 0) + 1

    # Initialize the processing queue with all nodes having in-degree 0.
    # These are the root nodes with no dependencies — they can be
    # executed immediately. Using deque for O(1) popleft.
    queue: deque[str] = deque(
        nid for nid, deg in in_degree.items() if deg == 0
    )

    result: list[dict] = []

    # Process the queue using Kahn's algorithm: pop a node, append to
    # result, decrement in-degrees of its neighbors, enqueue any that
    # reach in-degree 0.
    while queue:
        node_id = queue.popleft()

        # Find the node dict matching this ID. Since node IDs are
        # unique (validated by the Rust scheduler), there is exactly
        # one match. Linear scan is fine — node counts are small.
        node = next(n for n in nodes if n["id"] == node_id)
        result.append(node)

        # Decrement in-degrees for all nodes that depend on this one.
        for neighbor in adjacency[node_id]:
            in_degree[neighbor] -= 1
            # When a neighbor's in-degree reaches 0, all its
            # dependencies have been processed — it is now ready.
            if in_degree[neighbor] == 0:
                queue.append(neighbor)

    # After processing, if the result contains fewer nodes than the
    # input, a cycle exists: the remaining nodes have non-zero in-degree
    # because they depend on each other in a loop. This is the same
    # signal used by Rust dag.rs check 6.
    if len(result) < len(nodes):
        remaining = [nid for nid, deg in in_degree.items() if deg > 0]
        raise ValueError(
            f"Cycle detected in graph — nodes involved: {', '.join(remaining)}"
        )

    return result


def execute_graph(graph: dict, ctx_factory) -> dict:
    """Execute all nodes in *graph* in topological order, checking for cancellation.

    Uses ``topo_sort()`` to determine a valid execution order, then instantiates
    each node via ``NODE_REGISTRY`` and calls its ``execute()`` method.  Before
    every node execution the function checks ``ctx.cancel_flag.is_set()`` — if the
    flag is set the loop stops immediately and returns
    ``{"cancelled": True}``.  On normal completion (all nodes executed without
    cancellation) it returns ``{"cancelled": False}`` with a ``results`` dict
    keyed by node ID.

    ``NODE_REGISTRY`` and ``NodeContext`` are imported inside this function body
    (not at module level) to avoid transitive ``torch`` dependencies during test
    collection — the same pattern used in ``worker_main.py`` for ``worker.ipc``.

    Args:
        graph: A graph dict with ``"nodes"`` and optionally ``"edges"`` keys,
            the same shape accepted by ``topo_sort()``.
        ctx_factory: A callable that takes no arguments and returns a
            ``NodeContext`` instance.  The factory is responsible for supplying
            ``job_id``, ``cancel_flag`` (a ``threading.Event``), and all other
            context fields.

    Returns:
        A dict with two possible shapes:
        * ``{"cancelled": True}`` — the cancel flag was set before or during
          execution.  No ``results`` key is present.
        * ``{"cancelled": False, "results": {...}}`` — all nodes executed
          successfully.  The ``results`` dict maps node IDs to their
          ``execute()`` return values.

    Raises:
        ValueError: If the graph contains a cycle (propagated from ``topo_sort()``).
        KeyError: If a node references a type not present in ``NODE_REGISTRY``.
    """
    # Import NODE_REGISTRY inside the function body — importing at module
    # level would transitively pull in torch via worker.nodes.__init__.py.
    # The worker_main.py pattern (importing worker.ipc inside functions)
    # established this as the standard approach for avoiding transitive deps.
    from worker.nodes.base import NODE_REGISTRY  # noqa: PLC0415

    # Sort nodes into dependency order.  If the graph has no edges, every
    # node has in-degree 0 and the result is their original insertion order.
    sorted_nodes = topo_sort(graph)

    # Log the number of nodes at DEBUG level — operators need this for
    # diagnosing why a job is taking long (too many nodes) or completing
    # instantly (zero nodes, which would be a validation gap).
    logger.debug("execute_graph: %d nodes to execute", len(sorted_nodes))

    # Accumulate node outputs keyed by node ID.  This dict is returned in
    # the "cancelled": False result so callers can inspect intermediate
    # outputs (e.g. for debugging or for passing to downstream nodes).
    results: dict[str, dict] = {}

    # Create the runtime context via the caller-supplied factory.  The
    # factory (produced by the caller in worker_main.py) constructs a
    # NodeContext with a threading.Event() as the cancel_flag, allowing
    # the caller to set the flag from a different thread (e.g. on
    # CancelJob IPC message).
    ctx = ctx_factory()

    for node in sorted_nodes:
        # Cooperative cancellation checkpoint — check the cancel flag
        # BEFORE executing each node.  This is the only cancellation point;
        # we never interrupt a node mid-execute.  The flag is checked
        # again after each node completes, so a node that sets the flag
        # during its own execute() will prevent subsequent nodes from
        # running.
        if ctx.cancel_flag.is_set():
            logger.info("execute_graph: cancel flag set, stopping after %d nodes", len(results))
            return {"cancelled": True}

        # Instantiate the node class from the registry.  The node type
        # string comes from the graph dict (validated by the Rust scheduler
        # before reaching the worker), so a KeyError here would indicate a
        # missing node type registration — a bug, not a runtime condition.
        node_cls = NODE_REGISTRY[node["type"]]

        # Execute the node with its inputs.  The node's execute() method
        # is responsible for reading from ctx and producing output.
        # Inputs are passed as keyword arguments matching slot names.
        node_instance = node_cls()
        node_output = node_instance.execute(ctx, **node.get("inputs", {}))

        # Store the output keyed by node ID.  This allows the caller to
        # inspect results after execution completes (normal or cancelled).
        results[node["id"]] = node_output

    # All nodes executed without cancellation — return the full results
    # dict so the caller can inspect intermediate outputs.
    return {"cancelled": False, "results": results}
