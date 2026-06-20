"""Graph execution engine for the AnvilML Python worker.

This module provides the ``run_graph()`` function that executes a node
graph described as JSON. It performs a topological sort of the nodes
using Kahn's algorithm, then instantiates each node from the
``NODE_REGISTRY``, resolves input values from upstream outputs, calls
``execute()``, and stores results keyed by node ID.

The graph format is a dict with a ``"nodes"`` key containing a list of
node objects. Each node object has ``"id"``, ``"type"``, and ``"inputs"``
keys. Input values that are lists (e.g. ``["1", "latent"]``) reference
another node's output by ``(node_id, output_name)``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import logging
from collections import deque
from typing import Any

from worker.nodes import NODE_REGISTRY
from worker.nodes.base import NodeContext

logger = logging.getLogger(__name__)


def _topo_sort(nodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Topologically sort *nodes* using Kahn's algorithm.

    Builds a dependency graph from the node input references, then
    performs a breadth-first topological sort. If a cycle exists
    (not all nodes are processed), raises ``ValueError``.

    Args:
        nodes: List of node dicts, each with ``"id"``, ``"type"``,
            and ``"inputs"`` keys.

    Returns:
        The node list sorted in valid execution order.

    Raises:
        ValueError: If the graph contains a cycle.
    """
    # Build adjacency list and in-degree count.
    # Each node's inputs may reference other nodes via list values
    # like ["node_id", "output_name"]. We extract the node_id from
    # each such reference to build the dependency graph.
    node_ids = [n["id"] for n in nodes]
    in_degree: dict[str, int] = {nid: 0 for nid in node_ids}
    dependents: dict[str, list[str]] = {nid: [] for nid in node_ids}

    for node in nodes:
        nid = node["id"]
        inputs = node.get("inputs", {})
        for input_name, input_val in inputs.items():
            # Input values that are lists reference another node's output.
            # The format is [node_id, output_name]. Extract the node_id
            # to build the dependency edge.
            if isinstance(input_val, list) and len(input_val) >= 1:
                dep_id = str(input_val[0])
                if dep_id in in_degree and dep_id != nid:
                    # Avoid counting duplicate edges from multiple inputs
                    # referencing the same node (e.g. two slots both
                    # consuming node A's output).
                    dependents.setdefault(dep_id, []).append(nid)
                    in_degree[nid] = in_degree.get(nid, 0) + 1

    # Kahn's algorithm: start with all nodes that have no dependencies.
    queue: deque[str] = deque(nid for nid in node_ids if in_degree[nid] == 0)
    sorted_nodes: list[dict[str, Any]] = []

    while queue:
        nid = queue.popleft()
        # Find the node dict matching this ID.
        sorted_nodes.append(next(n for n in nodes if n["id"] == nid))
        for dep in dependents.get(nid, []):
            in_degree[dep] -= 1
            if in_degree[dep] == 0:
                queue.append(dep)

    # If we couldn't process all nodes, a cycle exists.
    # This is a hard error — the graph JSON is structurally invalid.
    if len(sorted_nodes) != len(node_ids):
        raise ValueError("graph contains a cycle")

    return sorted_nodes


def _resolve_input_value(
    value: Any, outputs: dict[str, dict[str, Any]]
) -> Any:
    """Resolve a single input value, handling node-reference lists.

    If the value is a list of the form ``[node_id, output_name]``,
    looks up the corresponding node's output. Otherwise returns the
    value unchanged (it may be a literal value like a number or string).

    Args:
        value: The input value to resolve. May be a literal or a
            node-reference list.
        outputs: Dict of node-id → output dict, produced by prior
            node executions in topological order.

    Returns:
        The resolved value. If the input was a node reference, returns
        the referenced output. Otherwise returns the input unchanged.
    """
    if isinstance(value, list) and len(value) >= 2:
        # This is a node reference: [node_id, output_name].
        # Look up the referenced node's output in the accumulated
        # outputs dict.
        ref_node_id = str(value[0])
        ref_output_name = str(value[1])
        if ref_node_id in outputs:
            return outputs[ref_node_id].get(ref_output_name)
        # If the referenced node isn't in outputs, return None.
        # This handles cases where the graph has nodes whose outputs
        # aren't consumed (or the reference is to a non-existent node).
        return None
    return value


def run_graph(
    graph: dict[str, Any],
    settings: dict[str, Any],
    ctx: NodeContext,
) -> None:
    """Execute a node graph in topological order.

    Topologically sorts the nodes from *graph*, then for each node:
    1. Instantiates the node class from ``NODE_REGISTRY``.
    2. Resolves input values by looking up prior node outputs.
    3. Calls ``node.execute(**inputs)``.
    4. Stores outputs keyed by node ID.

    Args:
        graph: A dict with a ``"nodes"`` key containing a list of
            node objects. Each node has ``"id"``, ``"type"``, and
            ``"inputs"`` keys.
        settings: Job-level settings dict (e.g. device preferences).
            Passed to nodes for potential future use.
        ctx: The ``NodeContext`` providing runtime access to job,
            device, cancellation, emit, and pipeline cache.

    Raises:
        ValueError: If the graph contains a cycle.
        KeyError: If a node's ``"type"`` is not found in
            ``NODE_REGISTRY``.
    """
    # Extract nodes from the graph. The graph is expected to have a
    # "nodes" key per the scheduler's JSON schema.
    nodes = graph.get("nodes", [])

    # Log the graph structure for diagnostic purposes.
    logger.debug(
        "run_graph: %d nodes in graph for job %s",
        len(nodes),
        ctx.job_id,
    )

    # Topological sort to determine execution order.
    # This ensures nodes execute only after all their dependencies
    # have completed, so input resolution always finds prior outputs.
    sorted_nodes = _topo_sort(nodes)
    logger.debug(
        "run_graph: topo order for job %s: %s",
        ctx.job_id,
        [n["id"] for n in sorted_nodes],
    )

    # Accumulated outputs keyed by node ID. Each value is a dict
    # mapping output slot names to computed values.
    outputs: dict[str, dict[str, Any]] = {}

    for node_def in sorted_nodes:
        node_id = node_def["id"]
        node_type = node_def["type"]
        node_inputs = node_def.get("inputs", {})

        # Look up the node class in the registry. A KeyError here
        # means the scheduler referenced an unregistered node type —
        # this is a programming error in the graph definition.
        node_cls = NODE_REGISTRY[node_type]
        logger.debug(
            "run_graph: executing node %s (%s) for job %s",
            node_id,
            node_type,
            ctx.job_id,
        )

        # Instantiate the node with the runtime context.
        node = node_cls(ctx)

        # Resolve input values. Each input may be a literal value
        # or a reference to another node's output ([node_id, output_name]).
        # We resolve each one by looking up the referenced node's outputs.
        resolved_inputs: dict[str, Any] = {}
        for input_name, input_val in node_inputs.items():
            resolved_inputs[input_name] = _resolve_input_value(
                input_val, outputs
            )

        # Call the node's execute method with resolved inputs.
        # Any exception from execute() propagates up to the caller
        # (worker_main), which catches it and sends a Failed event.
        result = node.execute(**resolved_inputs)
        outputs[node_id] = result

    logger.debug(
        "run_graph: completed %d nodes for job %s",
        len(sorted_nodes),
        ctx.job_id,
    )
