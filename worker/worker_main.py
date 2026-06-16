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

from worker.ipc import connect, recv_message, send_event


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
        "node_types": [],
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
