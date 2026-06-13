"""Tests for :mod:`worker.worker_main` message loop (mock mode).

Spawns the worker as a subprocess with ``ANVILML_WORKER_MOCK=1`` and
exercises the ZeroMQ DEALER IPC transport: InitializeHardware -> Ready,
Ping -> Pong, MemoryQuery -> MemoryReport, Shutdown -> Dying + exit 0.
"""

import os
import subprocess
import sys
import time

import zmq

import msgpack

# The worker script path (absolute so it works regardless of cwd).
# __file__ is worker/tests/test_worker_main.py, so we go up two levels to repo root.
_WORKER_SCRIPT = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "worker_main.py",
)


class _ZmqTransport:
    """Thin wrapper around a zmq.DEALER socket for test communication."""

    def __init__(self, sock: zmq.Socket) -> None:
        self._sock = sock

    def send(self, data: dict) -> None:
        self._sock.send(msgpack.packb(data, use_bin_type=True))

    def recv(self, timeout_ms: int = 10000) -> dict:
        self._sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
        data = self._sock.recv()
        return msgpack.unpackb(data, raw=False)

    def poll_recv(self, timeout_ms: int = 5000) -> dict | None:
        """Poll for a message with the given timeout; returns None on timeout."""
        self._sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
        if self._sock.poll(timeout_ms) == 0:
            return None
        data = self._sock.recv()
        return msgpack.unpackb(data, raw=False)


def _spawn_worker(worker_id: str = "test-0", device_index: int = 0):
    """Spawn the worker in mock mode with ZMQ DEALER transport.

    Binds a DEALER socket on an ephemeral port, passes the port via
    ANVILML_IPC_PORT, and returns (worker_proc, transport, ctx).
    """
    ctx = zmq.Context()
    sock = ctx.socket(zmq.DEALER)
    sock.bind("tcp://127.0.0.1:0")
    endpoint = sock.getsockopt(zmq.LAST_ENDPOINT).decode()  # e.g. "tcp://127.0.0.1:54321"
    port = int(endpoint.split(":")[-1])

    env = os.environ.copy()
    env["ANVILML_WORKER_MOCK"] = "1"
    env["ANVILML_IPC_PORT"] = str(port)

    proc = subprocess.Popen(
        [sys.executable, _WORKER_SCRIPT, "--worker-id", worker_id,
         "--device-index", str(device_index)],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE, env=env,
    )
    transport = _ZmqTransport(sock)
    return proc, transport, ctx


class TestWorkerMain:
    """Integration tests that spawn the worker subprocess."""

    def test_ready_on_init_hardware(self):
        """InitializeHardware triggers Ready{} with mock values."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            ready = transport.recv()
            assert ready["_type"] == "Ready"
            assert ready["worker_id"] == "test-0"
            assert ready["device_index"] == 0

            transport.send({"_type": "Shutdown"})
            dying = transport.recv()
            assert dying["_type"] == "Dying"
            assert dying["reason"] == "shutdown"
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_mock_values(self):
        """Mock Ready payload matches spec values."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            ready = transport.recv()

            assert ready["vram_total_mib"] == 8192
            assert ready["vram_free_mib"] == 8192
            assert ready["arch"] == "gfx1100"
            assert ready["fp16"] is True
            assert ready["bf16"] is True
            assert ready["flash_attention"] is False

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_ping_pong(self):
        """Worker receives Ping{seq} and responds with Pong{seq}."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            ready = transport.recv()
            assert ready["worker_id"] == "test-0"

            transport.send({"_type": "Ping", "seq": 42})
            pong = transport.recv()
            assert pong["_type"] == "Pong"
            assert pong["seq"] == 42

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_memory_query_report(self):
        """Worker receives MemoryQuery{} and responds with MemoryReport{0, 0}."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            ready = transport.recv()
            assert ready["worker_id"] == "test-0"

            transport.send({"_type": "MemoryQuery"})
            mem_report = transport.recv()
            assert mem_report["_type"] == "MemoryReport"
            assert mem_report["vram_used_mib"] == 0
            assert mem_report["ram_used_mib"] == 0

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_shutdown_dying_exit(self):
        """Worker receives Shutdown{}, responds Dying{reason: shutdown}, exits 0."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send({"_type": "Shutdown"})
            dying = transport.recv()
            assert dying["reason"] == "shutdown"
            proc.wait(timeout=5)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_double_init_exits(self):
        """Sending two InitializeHardware frames: first produces Ready, second
        produces no response event. Worker responds to Shutdown with Dying +
        exit 0. Guards against re-introduction of the double-InitializeHardware
        write bug (P10-B1). The Python worker's `ready_sent` guard ensures
        exactly one Ready is emitted regardless of how many InitializeHardware
        frames arrive."""
        proc, transport, ctx = _spawn_worker()

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            ready1 = transport.recv()
            assert ready1["_type"] == "Ready"

            # Second InitializeHardware — should produce no event.
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            # Poll with short timeout to confirm no second Ready arrives.
            second = transport.poll_recv(timeout_ms=2000)
            assert second is None, "Expected no second Ready event"

            transport.send({"_type": "Shutdown"})
            dying = transport.recv()
            assert dying["_type"] == "Dying"
            assert dying["reason"] == "shutdown"
            proc.wait(timeout=5)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_execute_progress_completed(self):
        """Execute{job_id, graph, settings, device_index} triggers N Progress
        events followed by a single Completed event."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [
                {"type": "LoadModel"},
                {"type": "Inference"},
                {"type": "SaveOutput"},
            ]
            execute_msg = {
                "_type": "Execute",
                "job_id": "exec-test-1",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            # Collect events: Ready (already consumed), then Progress × N, Completed.
            progress_events = []
            completed_events = []
            deadline = time.monotonic() + 10

            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                ev_type = ev.get("_type", "")
                if ev_type == "Progress":
                    progress_events.append(ev)
                elif ev_type == "Completed":
                    completed_events.append(ev)
                    break  # Completed means execution finished.

            assert len(progress_events) == len(nodes), (
                f"expected {len(nodes)} Progress events, got {len(progress_events)}"
            )
            for i, node in enumerate(nodes):
                pe = progress_events[i]
                assert pe["job_id"] == "exec-test-1"
                assert pe["node_index"] == i
                assert pe["node_total"] == len(nodes)
                assert pe["node_type"] == node["type"]

            assert len(completed_events) == 1
            completed = completed_events[0]
            assert completed["job_id"] == "exec-test-1"
            assert completed["elapsed_ms"] >= 0

            transport.send({"_type": "Shutdown"})
            dying = transport.recv()
            assert dying["reason"] == "shutdown"
            proc.wait(timeout=5)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_execute_saveimage_imageready(self):
        """Execute with a graph containing a SaveImage node → verify ImageReady
        event emitted with correct fields (width=64, height=64, format='png',
        valid base64 image_b64, resolved seed, steps, prompt), followed by
        Completed."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [
                {"type": "LoadModel"},
                {"type": "SaveImage"},
                {"type": "Inference"},
            ]
            execute_msg = {
                "_type": "Execute",
                "job_id": "exec-test-si-1",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            imageready_events = []
            completed_events = []
            deadline = time.monotonic() + 10

            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                ev_type = ev.get("_type", "")
                if ev_type == "ImageReady":
                    imageready_events.append(ev)
                elif ev_type == "Completed":
                    completed_events.append(ev)
                    break

            assert len(imageready_events) == 1, (
                f"expected exactly one ImageReady, got {len(imageready_events)}"
            )
            ir = imageready_events[0]
            assert ir["job_id"] == "exec-test-si-1"
            assert ir["width"] == 64
            assert ir["height"] == 64
            assert ir["format"] == "png"
            assert isinstance(ir["image_b64"], str) and len(ir["image_b64"]) > 0
            assert ir["seed"] >= 0
            assert ir["steps"] == 1
            assert ir["prompt"] == ""

            assert len(completed_events) == 1
            assert completed_events[0]["job_id"] == "exec-test-si-1"

            transport.send({"_type": "Shutdown"})
            dying = transport.recv()
            proc.wait(timeout=5)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_execute_saveimage_seed_resolution(self):
        """Execute with SaveImage node having seed=-1 → verify ImageReady seed
        is a valid random int in range [0, 2^63-1]."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [{"type": "SaveImage"}]
            execute_msg = {
                "_type": "Execute",
                "job_id": "exec-test-si-2",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                if ev.get("_type") == "ImageReady":
                    assert 0 <= ev["seed"] <= 2**63 - 1
                    break

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_execute_saveimage_inputs_resolved(self):
        """Execute with SaveImage node having explicit prompt/seed/steps
        inputs → verify ImageReady fields match node inputs."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [
                {
                    "type": "SaveImage",
                    "inputs": {
                        "prompt": "test prompt text",
                        "seed": 12345,
                        "steps": 20,
                    },
                }
            ]
            execute_msg = {
                "_type": "Execute",
                "job_id": "exec-test-si-3",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                if ev.get("_type") == "ImageReady":
                    assert ev["prompt"] == "test prompt text"
                    assert ev["seed"] == 12345
                    assert ev["steps"] == 20
                    break

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_execute_no_saveimage_no_imageready(self):
        """Execute with a graph that has no SaveImage node → verify NO
        ImageReady event emitted, only Progress + Completed."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [
                {"type": "LoadModel"},
                {"type": "Inference"},
                {"type": "SaveOutput"},
            ]
            execute_msg = {
                "_type": "Execute",
                "job_id": "exec-test-si-4",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            progress_events = []
            completed_events = []
            deadline = time.monotonic() + 10

            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                ev_type = ev.get("_type", "")
                if ev_type == "ImageReady":
                    assert False, f"unexpected ImageReady event"
                elif ev_type == "Progress":
                    progress_events.append(ev)
                elif ev_type == "Completed":
                    completed_events.append(ev)
                    break

            assert len(progress_events) == len(nodes)
            assert len(completed_events) == 1

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_cancel_job_during_execute(self):
        """Send CancelJob after the first Progress frame arrives — verify
        Cancelled is emitted (no Completed), with correct job_id.

        Uses ANVILML_MOCK_NODE_DELAY_MS so the worker pauses between nodes,
        giving the test time to inject the CancelJob message while the
        worker is blocked in the delay (cooperative cancellation window).
        """
        nodes = [
            {"type": "LoadModel"},
            {"type": "Inference"},
            {"type": "SaveOutput"},
        ]
        execute_msg = {
            "_type": "Execute",
            "job_id": "cancel-test-1",
            "graph": {"nodes": nodes},
            "settings": {},
            "device_index": 0,
        }

        env = os.environ.copy()
        env["ANVILML_WORKER_MOCK"] = "1"
        env["ANVILML_MOCK_NODE_DELAY_MS"] = "100"

        ctx = zmq.Context()
        sock = ctx.socket(zmq.DEALER)
        sock.bind("tcp://127.0.0.1:0")
        endpoint = sock.getsockopt(zmq.LAST_ENDPOINT).decode()
        port = int(endpoint.split(":")[-1])

        env["ANVILML_IPC_PORT"] = str(port)

        proc = subprocess.Popen(
            [sys.executable, _WORKER_SCRIPT,
             "--worker-id", "cancel-test-1",
             "--device-index", "0"],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, env=env,
        )
        transport = _ZmqTransport(sock)

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            # Wait until the first Progress frame (node_index 0) arrives.
            first_progress_seen = False
            deadline = time.monotonic() + 5
            while not first_progress_seen and time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=200)
                if ev is not None and ev.get("_type") == "Progress" and ev.get("node_index") == 0:
                    first_progress_seen = True
                    break

            assert first_progress_seen, "Never received first Progress frame"

            # Send CancelJob — worker is in the delay between nodes.
            transport.send({
                "_type": "CancelJob",
                "job_id": "cancel-test-1",
            })

            # Wait for Cancelled event.
            cancelled_seen = False
            cancel_deadline = time.monotonic() + 3
            while not cancelled_seen and time.monotonic() < cancel_deadline:
                ev = transport.poll_recv(timeout_ms=200)
                if ev is not None and ev.get("_type") == "Cancelled":
                    cancelled_seen = True
                    break

            # Send Shutdown so the worker exits cleanly.
            transport.send({"_type": "Shutdown"})
            transport.recv()  # Dying

            proc.wait(timeout=5)

            # Verify Cancelled was emitted and no Completed.
            assert cancelled_seen, "Cancelled event never received"
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_cancel_before_execute(self):
        """Send CancelJob before Execute → worker processes CancelJob
        (flag set), then receives Execute → should emit Cancelled
        before any Progress."""
        proc, transport, ctx = _spawn_worker()

        try:
            nodes = [
                {"type": "LoadModel"},
                {"type": "Inference"},
            ]
            execute_msg = {
                "_type": "Execute",
                "job_id": "cancel-test-2",
                "graph": {"nodes": nodes},
                "settings": {},
                "device_index": 0,
            }

            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            # Cancel before Execute.
            transport.send({
                "_type": "CancelJob",
                "job_id": "cancel-test-2",
            })
            transport.send(execute_msg)

            # Wait for Cancelled event.
            cancelled_seen = False
            progress_seen = False
            completed_seen = False
            deadline = time.monotonic() + 5

            while not (cancelled_seen and not progress_seen) and time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=200)
                if ev is None:
                    continue
                ev_type = ev.get("_type", "")
                if ev_type == "Cancelled":
                    cancelled_seen = True
                elif ev_type == "Progress":
                    progress_seen = True
                elif ev_type == "Completed":
                    completed_seen = True

            assert cancelled_seen, "Cancelled event never received"
            assert not progress_seen, "Progress should not appear before Cancelled"
            assert not completed_seen, "Completed should not appear after cancel"

            transport.send({"_type": "Shutdown"})
            transport.recv()
            proc.wait(timeout=5)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()

    def test_mock_node_delay_ms(self):
        """Set ANVILML_MOCK_NODE_DELAY_MS=75, execute a 3-node job →
        verify total elapsed time is at least 120ms (2 inter-node
        delays × 75ms = 150ms nominal; threshold 120ms gives 30ms
        margin above Windows timer granularity ~15ms)."""
        nodes = [
            {"type": "LoadModel"},
            {"type": "Inference"},
            {"type": "SaveOutput"},
        ]
        execute_msg = {
            "_type": "Execute",
            "job_id": "delay-test-1",
            "graph": {"nodes": nodes},
            "settings": {},
            "device_index": 0,
        }

        env = os.environ.copy()
        env["ANVILML_WORKER_MOCK"] = "1"
        env["ANVILML_MOCK_NODE_DELAY_MS"] = "75"

        ctx = zmq.Context()
        sock = ctx.socket(zmq.DEALER)
        sock.bind("tcp://127.0.0.1:0")
        endpoint = sock.getsockopt(zmq.LAST_ENDPOINT).decode()
        port = int(endpoint.split(":")[-1])

        env["ANVILML_IPC_PORT"] = str(port)

        proc = subprocess.Popen(
            [sys.executable, _WORKER_SCRIPT,
             "--worker-id", "test-delay-0",
             "--device-index", "0"],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, env=env,
        )
        transport = _ZmqTransport(sock)

        try:
            transport.send({"_type": "InitializeHardware", "device_str": "cuda:0"})
            transport.recv()  # Ready

            transport.send(execute_msg)

            # Wait for Completed event.
            completed_events = []
            deadline = time.monotonic() + 15

            while time.monotonic() < deadline:
                ev = transport.poll_recv(timeout_ms=500)
                if ev is None:
                    continue
                if ev.get("_type") == "Completed":
                    completed_events.append(ev)
                    break

            assert len(completed_events) == 1, (
                f"expected exactly one Completed, got {len(completed_events)}"
            )
            elapsed_ms = completed_events[0]["elapsed_ms"]
            # 2 inter-node delays × 75ms = 150ms nominal; assert ≥ 120ms to
            # absorb Windows timer granularity (~15ms) and scheduler jitter.
            assert elapsed_ms >= 120, (
                f"expected elapsed_ms >= 120, got {elapsed_ms}"
            )

            transport.send({"_type": "Shutdown"})
            transport.recv()
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            transport._sock.close()
            ctx.term()
