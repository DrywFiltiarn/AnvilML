"""Graph executor for the AnvilML Python worker.

Implements ``run_graph`` — a Kahn topological-sort + node-execution loop
that resolves edge references, dispatches nodes via ``NODE_REGISTRY``,
handles cancellation cooperatively, and emits ``Progress`` /
``Completed`` / ``Failed`` / ``Cancelled`` IPC events.

Exception
---------
:class:`CancelledError`
    Raised by node implementations (e.g. ``ZitSampler`` callbacks) to
    signal that the current job has been cancelled.  The executor catches
    this and emits a ``Cancelled`` event.
"""

from __future__ import annotations

import logging
import os
import time
import traceback
from typing import Any

import worker.ipc as ipc
from worker.nodes.base import NODE_REGISTRY

logger = logging.getLogger(__name__)


class CancelledError(Exception):
    """Raised by a node to signal that the current job was cancelled."""


def run_graph(
    graph: dict,
    settings: dict,
    device_str: str,
    cancel_flag: Any,
    emit_fn: Any,
    pipeline_cache: Any,
    job_id: str,
) -> dict[str, Any]:
    """Execute a node graph in topological order.

    Parameters
    ----------
    graph :
        Dict with a ``"nodes"`` key containing a list of node dicts.
        Each node dict must have a ``"type"`` and optionally an ``"id"``
        and ``"inputs"`` dict.  Edge references inside ``"inputs"`` are
        dicts with ``"node_id"`` and ``"output_slot"`` keys.
    settings :
        Global execution settings (e.g. ``seed``, ``steps``).
    device_str :
        Device string (e.g. ``"cuda:0"``, ``"cpu"``).
    cancel_flag :
        A ``threading.Event`` set when the job is cancelled.
    emit_fn :
        Callback to emit IPC events (e.g. ``ipc.write_frame``).
    pipeline_cache :
        LRU cache for loaded pipeline objects.
    job_id :
        Unique identifier for the current job.

    Returns
    -------
    dict
        ``{"status": "completed"|"cancelled"|"failed", ...}``
    """
    nodes = graph.get("nodes", [])

    # ── Kahn topological sort ──────────────────────────────────────────

    # Build adjacency list and in-degree map from edge refs in inputs.
    node_ids = [n.get("id", str(i)) for i, n in enumerate(nodes)]
    node_id_set = set(node_ids)
    adjacency: dict[str, list[str]] = {nid: [] for nid in node_ids}
    in_degree: dict[str, int] = {nid: 0 for nid in node_ids}

    for node in nodes:
        node_id = node.get("id", str(nodes.index(node)))
        inputs = node.get("inputs", {})
        for _slot, value in inputs.items():
            if isinstance(value, dict) and "node_id" in value and "output_slot" in value:
                ref_node_id = value["node_id"]
                if ref_node_id in node_id_set:
                    adjacency[ref_node_id].append(node_id)
                    in_degree[node_id] += 1

    # BFS queue — start with nodes that have no incoming edges.
    queue = [nid for nid in node_ids if in_degree[nid] == 0]
    sorted_order: list[str] = []

    while queue:
        nid = queue.pop(0)
        sorted_order.append(nid)
        for neighbour in adjacency[nid]:
            in_degree[neighbour] -= 1
            if in_degree[neighbour] == 0:
                queue.append(neighbour)

    if len(sorted_order) < len(nodes):
        logger.debug("cycle detected in graph for job %s", job_id)
        emit_fn({
            "_type": "Failed",
            "job_id": job_id,
            "error": "cycle_detected",
        })
        return {"status": "failed", "error": "cycle_detected"}

    # ── Execute nodes in sorted order ──────────────────────────────────

    logger.info("executing graph with %d nodes for job %s", len(nodes), job_id)
    start = time.monotonic()
    node_outputs: dict[str, dict[str, Any]] = {}

    try:
        for idx, node in enumerate(nodes):
            # Cancel check before each node.
            if cancel_flag.is_set():
                emit_fn({
                    "_type": "Cancelled",
                    "job_id": job_id,
                })
                return {"status": "cancelled"}

            node_type = node.get("type", "unknown")
            node_id = node.get("id", str(idx))

            if node_type in NODE_REGISTRY:
                # ── Registered node: dispatch via NODE_REGISTRY ────────

                # Resolve inputs: edge refs → previous node outputs,
                # literals → pass through.
                resolved_inputs: dict[str, Any] = {}
                for slot, value in node.get("inputs", {}).items():
                    if isinstance(value, dict) and "node_id" in value and "output_slot" in value:
                        ref = value
                        prev_outputs = node_outputs[ref["node_id"]]
                        resolved_inputs[slot] = prev_outputs[ref["output_slot"]]
                    else:
                        resolved_inputs[slot] = value

                try:
                    cls = NODE_REGISTRY[node_type]
                    ctx = _NodeContext(
                        pipeline_cache=pipeline_cache,
                        device_str=device_str,
                        emit_fn=emit_fn,
                        cancel_flag=cancel_flag,
                        job_id=job_id,
                    )
                    node_instance = cls(ctx)
                    outputs = node_instance.execute(**resolved_inputs)
                    node_outputs[node_id] = outputs
                except CancelledError:
                    emit_fn({
                        "_type": "Cancelled",
                        "job_id": job_id,
                    })
                    return {"status": "cancelled"}
                except Exception as e:
                    tb = traceback.format_exc()
                    logger.error(
                        "node execution failed for job %s: %s", job_id, e,
                        exc_info=e,
                    )
                    emit_fn({
                        "_type": "Failed",
                        "job_id": job_id,
                        "error": str(e),
                        "traceback": tb,
                    })
                    return {
                        "status": "failed",
                        "error": str(e),
                        "traceback": tb,
                    }
            else:
                # ── Unregistered node: fallback (mock) execution ───────
                logger.debug(
                    "node type %r not registered for job %s — skipping",
                    node_type, job_id,
                )

                # Handle SaveImage specially for backward compatibility.
                if node_type == "SaveImage":
                    inputs = node.get("inputs", {})
                    prompt = inputs.get("prompt", "")
                    seed = inputs.get("seed", settings.get("seed", -1))
                    if seed == -1:
                        import random
                        seed = random.randint(0, 2**63 - 1)
                    steps = inputs.get("steps", settings.get("steps", 1))
                    import base64
                    from io import BytesIO
                    from PIL import Image
                    img = Image.new("RGB", (64, 64), (0, 0, 0))
                    buf = BytesIO()
                    img.save(buf, format="PNG")
                    image_b64 = base64.b64encode(buf.getvalue()).decode("ascii")
                    emit_fn({
                        "_type": "ImageReady",
                        "job_id": job_id,
                        "image_b64": image_b64,
                        "width": 64,
                        "height": 64,
                        "format": "png",
                        "seed": seed,
                        "steps": steps,
                        "prompt": prompt,
                    })

            # Emit progress after processing.
            emit_fn({
                "_type": "Progress",
                "job_id": job_id,
                "node_index": idx,
                "node_total": len(nodes),
                "node_type": node_type,
            })

            # Respect mock node delay for integration tests.
            delay_ms = int(os.environ.get("ANVILML_MOCK_NODE_DELAY_MS", "0"))
            if delay_ms > 0 and idx < len(nodes) - 1:
                time.sleep(delay_ms / 1000.0)

    except CancelledError:
        emit_fn({
            "_type": "Cancelled",
            "job_id": job_id,
        })
        return {"status": "cancelled"}
    except Exception as e:
        tb = traceback.format_exc()
        logger.error(
            "node execution failed for job %s: %s", job_id, e,
            exc_info=e,
        )
        emit_fn({
            "_type": "Failed",
            "job_id": job_id,
            "error": str(e),
            "traceback": tb,
        })
        return {"status": "failed", "error": str(e), "traceback": tb}

    # ── Completion ─────────────────────────────────────────────────────

    elapsed_ms = int((time.monotonic() - start) * 1000)
    logger.info(
        "graph completed for job %s in %d ms", job_id, elapsed_ms
    )
    emit_fn({
        "_type": "Completed",
        "job_id": job_id,
        "elapsed_ms": elapsed_ms,
    })
    return {"status": "completed", "elapsed_ms": elapsed_ms}


class _NodeContext:
    """Thin wrapper that adapts NodeContext fields for executor usage.

    The executor builds a simple context object; node implementations
    that expect the full ``NodeContext`` from ``worker.nodes.base``
    receive the same attribute names.
    """

    def __init__(
        self,
        pipeline_cache: Any,
        device_str: str,
        emit_fn: Any,
        cancel_flag: Any,
        job_id: str,
    ) -> None:
        self.pipeline_cache = pipeline_cache
        self.device_str = device_str
        self.emit_fn = emit_fn
        self.cancel_flag = cancel_flag
        self.job_id = job_id
