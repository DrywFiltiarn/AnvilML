#!/usr/bin/env python3
"""Mock-mode worker entry point for AnvilML.

This module is the Python worker process spawned by the Rust supervisor.
In mock mode (``ANVILML_WORKER_MOCK=1``), it connects to the Rust ROUTER
socket via ``worker.ipc``, emits a synthetic ``Ready`` event with mock
hardware capability values (no torch import), and enters a message
dispatch loop that responds to ``Ping`` with ``Pong`` and exits cleanly
on ``Shutdown``.

Non-mock mode is not yet implemented — the worker exits with code 1 if
``ANVILML_WORKER_MOCK`` is unset or ``"0"``. This safety gate prevents
accidental torch imports in test CI where the GPU stack is unavailable.

The worker is intended to be run as a subprocess:

    ANVILML_IPC_PORT=8488 \\
    ANVILML_WORKER_ID=worker-0 \\
    ANVILML_DEVICE_INDEX=0 \\
    ANVILML_DEVICE_TYPE=cpu \\
    ANVILML_WORKER_MOCK=1 \\
    python -m worker.worker_main

Or directly:

    ANVILML_IPC_PORT=8488 \\
    ANVILML_WORKER_MOCK=1 \\
    python worker/worker_main.py
"""

from __future__ import annotations

import os
import sys
import threading
import time

from worker.ipc import connect, recv_message, send_event
from worker.nodes import NODE_REGISTRY
from worker.nodes.base import NodeContext
from worker.executor import run_graph
from worker.pipeline_cache import PipelineCache


# Module-level cancel flag for job cancellation.
# This is set by the CancelJob message handler and checked by the
# Execute handler. A threading.Event is used so the CancelJob handler
# can signal cancellation to any thread checking the flag. The
# .set() / .clear() API is used instead of list indexing.
_cancel_flag = threading.Event()

# Module-level PipelineCache instance — created once at worker process
# startup so cache entries (loaded model components) persist across
# all jobs dispatched to this worker process.
_pipeline_cache: PipelineCache = PipelineCache()


def _import_nodes() -> None:
    """Import the ``worker.nodes`` package to trigger auto-import.

    Importing ``worker.nodes`` runs the module-level ``_ensure_imported()``
    function in ``worker.nodes.__init__``, which scans the package
    directory for sibling ``.py`` files and imports each one.  Any
    concrete node classes decorated with ``@register`` are then
    recorded in ``NODE_REGISTRY``.

    This function is a no-op after the first call because the
    auto-import mechanism in ``__init__.py`` is idempotent.
    """
    # Importing the package triggers the auto-import loop in
    # __init__.py, which scans for and imports sibling node modules.
    # The NODE_REGISTRY dict is populated by @register decorators
    # at import time.
    import worker.nodes  # noqa: F401


def _build_node_types_list() -> list[dict]:
    """Build the ``node_types`` list for the ``Ready`` IPC event.

    Iterates ``NODE_REGISTRY`` and converts each entry into a dict
    matching the ``NodeTypeDescriptor`` Rust struct shape:
    ``{type_name, display_name, category, description, inputs, outputs}``.

    Each ``SlotSpec`` is converted to a dict with ``name``,
    ``slot_type``, and ``optional`` keys.

    Returns:
        A list of node type descriptor dicts, one per registered
        node type. The list is empty when no concrete node modules
        have been imported yet (e.g. during initial setup).
    """
    node_types: list[dict] = []

    for type_name, node_cls in NODE_REGISTRY.items():
        # Convert SlotSpec objects to plain dicts for msgpack
        # serialisation. The Rust NodeTypeDescriptor expects
        # these as plain dicts, not dataclass instances.
        inputs = [
            {"name": s.name, "slot_type": s.slot_type, "optional": s.optional}
            for s in node_cls.INPUT_SLOTS
        ]
        outputs = [
            {"name": s.name, "slot_type": s.slot_type, "optional": s.optional}
            for s in node_cls.OUTPUT_SLOTS
        ]

        node_types.append({
            "type_name": type_name,
            "display_name": node_cls.DISPLAY_NAME,
            "category": node_cls.CATEGORY,
            "description": node_cls.DESCRIPTION,
            "inputs": inputs,
            "outputs": outputs,
        })

    return node_types


def main() -> None:
    """Run the mock-mode worker lifecycle.

    Reads identity and connection parameters from environment variables,
    connects to the Rust ROUTER socket, emits a Ready event, and enters
    the message dispatch loop.

    Exits with 0 on Shutdown, 1 if not in mock mode.
    """
    # Read identity and connection parameters from environment.
    # These are injected by the Rust supervisor's WorkerEnv at spawn time.
    # Defaults match ENVIRONMENT.md §3.4 for test convenience.
    port_str = os.environ.get("ANVILML_IPC_PORT", "8488")
    worker_id = os.environ.get("ANVILML_WORKER_ID", "worker-0")
    device_index_str = os.environ.get("ANVILML_DEVICE_INDEX", "0")
    device_type = os.environ.get("ANVILML_DEVICE_TYPE", "cpu")

    port = int(port_str)
    device_index = int(device_index_str)

    # Safety gate: mock mode is the only mode implemented so far.
    # Non-mock mode would need torch, which is unavailable in test CI.
    # Exiting with code 1 prevents silent failures where the worker
    # starts but cannot function.
    if os.environ.get("ANVILML_WORKER_MOCK") != "1":
        print(
            f"worker_main: ANVILML_WORKER_MOCK is not set to '1' "
            f"(got {os.environ.get('ANVILML_WORKER_MOCK')!r}). "
            f"Non-mock mode not yet implemented.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Connect to the Rust ROUTER socket. This creates the DEALER socket
    # and sets its identity so the ROUTER can route messages back to us.
    connect(port, worker_id)

    # Import all registered node types. This triggers the auto-import
    # mechanism in worker.nodes.__init__, which scans for and imports
    # sibling node modules. Concrete node classes decorated with @register
    # are then recorded in NODE_REGISTRY.
    _import_nodes()

    # Build the node types list from the populated registry. This
    # converts each registered node class into a dict matching the
    # NodeTypeDescriptor Rust struct shape for the Ready event.
    node_types = _build_node_types_list()

    # Build and send the Ready event. This is the synchronisation point
    # between Rust and Python — the Rust supervisor transitions the
    # worker to Idle only on receipt of a valid Ready event.
    #
    # In mock mode, all hardware capability values are synthetic. The
    # device_name is "Mock" (not "CPU") to clearly distinguish synthetic
    # values from real hardware in logs and telemetry.
    ready_event = {
        "_type": "Ready",
        "worker_id": worker_id,
        "device_index": device_index,
        "device_name": "Mock",
        "device_type": device_type,
        "vram_total_mib": 8192,
        "vram_free_mib": 8192,
        "torch_version": "mock",
        "fp16": True,
        "bf16": True,
        "fp8": True,
        "flash_attention": True,
        "node_types": node_types,
    }
    send_event(ready_event)

    # Enter the message dispatch loop. Process messages until Shutdown
    # or connection loss. Uses sys.exit(0) for Shutdown rather than
    # break to ensure the process exits with code 0, matching the
    # "Shutdown -> exit 0" contract with the Rust supervisor.
    while True:
        try:
            msg = recv_message()
        except Exception:
            # Connection lost or socket error — exit cleanly.
            break

        # Dispatch based on the _type discriminator.
        msg_type = msg.get("_type", "")

        if msg_type == "Ping":
            # Echo back a Pong with the same sequence number.
            # This heartbeat mechanism lets the supervisor verify
            # the worker process is alive and responsive.
            send_event({"_type": "Pong", "seq": msg["seq"]})
        elif msg_type == "Execute":
            # Execute a job graph. Extract job parameters from the
            # message, build a NodeContext, call run_graph(), and
            # report Completed or Failed back to the supervisor.
            job_id = msg["job_id"]
            graph = msg["graph"]
            settings = msg.get("settings", {})
            device_index = msg.get("device_index", 0)

            # Build the device string from the device index.
            # Format as "cuda:N" for GPU devices, "cpu" otherwise.
            # This matches the device string convention used by
            # the Rust supervisor's WorkerEnv.
            if device_type == "cpu":
                device = "cpu"
            else:
                device = f"{device_type}:{device_index}"

            # Record start time for elapsed_ms calculation.
            start = time.monotonic()

            # Build a NodeContext for this job execution.
            # The cancel_flag is a threading.Event — nodes check
            # .is_set() during long-running operations.
            # We reset the cancel flag for each new job execution.
            # The pipeline_cache is a shared LRU cache (PipelineCache
            # instance) created once at module level — cache entries
            # (loaded model components) persist across jobs dispatched
            # to this worker process, enabling cache hits on repeated
            # model loads within the same worker lifetime.
            _cancel_flag.clear()
            ctx = NodeContext(
                job_id=job_id,
                device=device,
                cancel_flag=_cancel_flag,
                emit=send_event,
                pipeline_cache=_pipeline_cache,  # shared LRU cache (see module-level _pipeline_cache)
            )

            try:
                # Execute the graph. run_graph performs topological sort,
                # instantiates nodes, resolves inputs, and calls execute().
                # Any exception from a node propagates here.
                run_graph(graph, settings, ctx)

                # Compute elapsed time and report success.
                elapsed_ms = int((time.monotonic() - start) * 1000)
                send_event({
                    "_type": "Completed",
                    "job_id": job_id,
                    "elapsed_ms": elapsed_ms,
                })
            except Exception as e:
                # Node execution failed — report the error and log it.
                # The error message is passed to the supervisor so it
                # can store it in the job's error field.
                elapsed_ms = int((time.monotonic() - start) * 1000)
                send_event({
                    "_type": "Failed",
                    "job_id": job_id,
                    "error": str(e),
                })
                print(
                    f"worker_main: job {job_id} failed: {e}",
                    file=sys.stderr,
                )
        elif msg_type == "CancelJob":
            # Cancel the current job execution.
            # Set the cancel flag so the executor stops at its next checkpoint.
            # The executor checks this flag between nodes/steps.
            # After setting the flag, send a Cancelled event back to the
            # supervisor so the scheduler can update the job status.
            _cancel_flag.set()
            send_event({
                "_type": "Cancelled",
                "job_id": msg.get("job_id"),
            })
        elif msg_type == "Shutdown":
            # Graceful exit requested by the supervisor.
            sys.exit(0)
        else:
            # Unknown message type — log a warning and continue.
            # This future-proofs the worker against new message types
            # added by future tasks without requiring a restart.
            print(
                f"worker_main: unknown message type {msg_type!r}, ignoring",
                file=sys.stderr,
            )


if __name__ == "__main__":
    main()
