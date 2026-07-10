#!/usr/bin/env python3
"""Runnable Proof: WebSocket client observes JobCompleted for a PassThrough job.

This script exercises the live event stream (GET /v1/events) end-to-end against
real dispatch — connecting a WebSocket client, submitting a PassThrough job via
HTTP POST, and asserting that a ``job_completed`` JSON frame with the matching
``job_id`` arrives on the WebSocket within 10 seconds.

Usage (server must already be running with ``mock-hardware``):

    python3 scripts/run_proof_p16_e1.py

Exit 0 on success, non-zero on failure.
"""

from __future__ import annotations

import asyncio
import json
import sys
import urllib.error
import urllib.request

import websockets


async def main() -> None:
    """Connect to the event stream, submit a job, and wait for completion."""
    url = "ws://127.0.0.1:8488/v1/events"
    api_url = "http://127.0.0.1:8488/v1/jobs"

    # Connect to the WebSocket event stream.
    async with websockets.connect(url) as ws:
        # The connection's first message is always the initial SystemStats
        # frame (per ANVILML_DESIGN.md §13.6). Consume it so the stream is
        # positioned for real events.
        initial_frame = await ws.recv()
        initial = json.loads(initial_frame)
        assert initial["type"] == "system_stats", (
            f"expected initial system_stats frame, got {initial['type']}"
        )

        # Submit a single-node PassThrough job via HTTP POST.
        payload = json.dumps({
            "graph": {
                "nodes": [
                    {
                        "id": "n0",
                        "type": "PassThrough",
                        "inputs": {"value": 1},
                    }
                ]
            },
            "settings": {},
        }).encode()
        req = urllib.request.Request(
            api_url,
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        response = urllib.request.urlopen(req)
        job_data = json.loads(response.read())
        job_id = job_data["job_id"]

        # Read from the WebSocket until a job_completed frame with the
        # matching job_id arrives. A PassThrough job completes very quickly
        # under mock-hardware (no real compute), so the frame should appear
        # well within the 10-second timeout.
        async with asyncio.timeout(10):
            while True:
                frame_raw = await ws.recv()
                frame = json.loads(frame_raw)
                # Print every frame to stdout for observability — the proof
                # transcript in the implementation report will capture this.
                print(json.dumps(frame, indent=2))
                if frame.get("type") == "job_completed" and frame.get("job_id") == job_id:
                    print(f"\nProof passed: job_completed for {job_id} received.")
                    return

    sys.exit(1)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except asyncio.TimeoutError:
        print(
            "\nProof failed: no job_completed frame arrived within 10 seconds.",
            file=sys.stderr,
        )
        sys.exit(1)
    except (ConnectionRefusedError, OSError) as exc:
        print(f"\nProof failed: could not connect — {exc}", file=sys.stderr)
        sys.exit(1)
    except AssertionError as exc:
        print(f"\nProof failed: assertion — {exc}", file=sys.stderr)
        sys.exit(1)
