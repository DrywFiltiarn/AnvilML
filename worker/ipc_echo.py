#!/usr/bin/env python3
"""Minimal echo worker for IPC stress testing.

Connects a DEALER socket to the Rust ROUTER, sends a startup Ready message,
then enters a loop echoing Ping messages as Pong responses. Exits on
Shutdown message or when the connection is lost.

This script is deliberately minimal — no hardware probing, no node importing,
no error handling beyond what is needed for the echo loop.
"""

import sys

from worker.ipc import connect, recv_message, send_event


def main() -> None:
    """Run the echo worker loop.

    Args:
        port: TCP port on 127.0.0.1 where the Rust ROUTER is bound.

    Exits with 0 on Shutdown, 1 on unexpected error.
    """
    # Parse the port from CLI argument — simpler than env vars for a single-
    # worker test (no need for #[serial] or capture-and-restore pattern).
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <port>", file=sys.stderr)
        sys.exit(1)

    port = int(sys.argv[1])

    # Connect with a hardcoded worker identity. This is a single-worker test,
    # so there is no pool or dynamic identity management — both Rust and
    # Python use the same literal string.
    connect(port, "stress-test-worker")

    # Send a startup Ready message. This serves two purposes:
    # 1. Signals readiness to the test (the test waits for this message).
    # 2. Establishes the worker's identity frame on the ROUTER socket,
    #    which is required before the ROUTER can route messages to us.
    #
    # The Rust WorkerEvent::Ready variant requires all fields (no Option
    # wrappers), so we must send a complete event with minimal values.
    send_event(
        {
            "_type": "Ready",
            "worker_id": "stress-test-worker",
            "device_index": 0,
            "device_name": "stress-test-device",
            "device_type": "cpu",
            "vram_total_mib": 0,
            "vram_free_mib": 0,
            "torch_version": "0.0.0",
            "fp16": False,
            "bf16": False,
            "fp8": False,
            "flash_attention": False,
            "node_types": [],
        },
    )

    # Enter the echo loop. Process messages until Shutdown or connection loss.
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
            # This is the core of the stress test: each Ping must be
            # matched with a Pong of the same seq value.
            send_event({"_type": "Pong", "seq": msg["seq"]})
        elif msg_type == "Shutdown":
            # Graceful exit requested by the test.
            break


if __name__ == "__main__":
    main()
