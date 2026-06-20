"""Tests for worker.worker_main — mock-mode worker entry point.

Each test spawns the worker as a subprocess with its own ROUTER socket
on a random port, ensuring complete isolation between tests. The worker
process is cleaned up unconditionally in a ``finally`` block.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time

import zmq

from worker import ipc


def _reset_ipc_state() -> None:
    """Reset module-level globals to pre-connect state.

    Closes the old DEALER socket before discarding the reference to
    prevent lingering connections that interfere with subsequent tests.
    """
    if ipc._sock is not None:
        ipc._sock.close(linger=0)
    ipc._ctx = None
    ipc._sock = None


def _make_worker_env(extra: dict[str, str] | None = None) -> dict[str, str]:
    """Build a subprocess environment dict for the worker.

    Copies the parent's environment and sets all required AnvilML
    variables so the worker can start without relying on inherited
    state (os.environ is not inherited through subprocess unless
    the env parameter is explicitly passed).

    Args:
        extra: Additional environment variables to include.

    Returns:
        A complete environment dict ready for subprocess.Popen.
    """
    env = os.environ.copy()
    env["ANVILML_WORKER_MOCK"] = "1"
    if extra:
        env.update(extra)
    return env


def test_mock_startup_sends_ready():
    """Verify the worker emits a valid Ready event on mock startup.

    Preconditions:
        A ROUTER socket is bound on a random port.

    Expects:
        The worker subprocess starts, connects IPC, and sends a Ready
        event with all required fields matching the mock mode spec.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to connect and send its Ready event.
            # The worker needs time to start, connect, and emit Ready.
            time.sleep(0.3)

            # ROUTER receives multipart: [identity, data]
            router.recv()  # identity frame
            raw = router.recv()
            import msgpack

            ready = msgpack.unpackb(raw, raw=False)

            # Verify all required fields from the mock Ready event spec.
            assert ready["_type"] == "Ready"
            assert ready["worker_id"] == "worker-0"
            assert ready["device_index"] == 0
            assert ready["device_name"] == "Mock"
            assert ready["device_type"] == "cpu"
            assert ready["vram_total_mib"] == 8192
            assert ready["vram_free_mib"] == 8192
            assert ready["torch_version"] == "mock"
            assert ready["fp16"] is True
            assert ready["bf16"] is True
            assert ready["fp8"] is True
            assert ready["flash_attention"] is True
            # SaveImage node is now registered, so node_types is non-empty.
            # Verify it contains exactly one entry (SaveImage).
            assert isinstance(ready["node_types"], list)
            assert len(ready["node_types"]) == 1
            assert ready["node_types"][0]["type_name"] == "SaveImage"
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_ping_returns_pong():
    """Verify the worker responds to Ping with a matching Pong.

    Preconditions:
        The worker is running in mock mode; ROUTER connected to worker.

    Expects:
        A Ping{seq: 42} sent via ROUTER yields a Pong{seq: 42} response.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            import msgpack

            # Consume the Ready event that the worker sends on startup.
            # This is required because the ROUTER has the Ready event
            # queued before we send our Ping — we must drain it first.
            router.recv()  # identity frame
            raw = router.recv()
            msgpack.unpackb(raw, raw=False)  # Ready event — discard

            # Send a Ping message to the worker via ROUTER.
            ping_msg = msgpack.packb({"_type": "Ping", "seq": 42}, use_bin_type=True)
            router.send_multipart([b"worker-0", ping_msg])

            # Receive the Pong response.
            router.recv()  # identity frame
            raw = router.recv()
            pong = msgpack.unpackb(raw, raw=False)

            assert pong["_type"] == "Pong"
            assert pong["seq"] == 42
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_shutdown_exits_cleanly():
    """Verify the worker exits with code 0 on Shutdown message.

    Preconditions:
        The worker is running in mock mode; ROUTER connected.

    Expects:
        A Shutdown message sent via ROUTER causes the subprocess to
        exit with code 0 within the timeout period.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            import msgpack

            # Send a Shutdown message to the worker.
            shutdown_msg = msgpack.packb({"_type": "Shutdown"}, use_bin_type=True)
            router.send_multipart([b"worker-0", shutdown_msg])

            # Wait for the worker to exit (within timeout).
            proc.wait(timeout=10)
            assert proc.returncode == 0, f"Expected exit code 0, got {proc.returncode}"
        finally:
            # Ensure the process is terminated if it hasn't exited yet.
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_env_vars_read_from_environment():
    """Verify the worker reads custom env vars and includes them in Ready.

    Preconditions:
        A ROUTER socket is bound on a random port; custom env vars set.

    Expects:
        The Ready event contains the custom worker_id, device_index, and
        device_type values passed via environment variables.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env(
        {
            "ANVILML_IPC_PORT": str(port),
            "ANVILML_WORKER_ID": "custom-worker",
            "ANVILML_DEVICE_INDEX": "3",
            "ANVILML_DEVICE_TYPE": "cuda",
        }
    )
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            import msgpack

            # ROUTER receives multipart: [identity, data]
            router.recv()  # identity frame
            raw = router.recv()
            ready = msgpack.unpackb(raw, raw=False)

            assert ready["_type"] == "Ready"
            assert ready["worker_id"] == "custom-worker"
            assert ready["device_index"] == 3
            assert ready["device_type"] == "cuda"
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)
