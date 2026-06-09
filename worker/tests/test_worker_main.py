"""Tests for :mod:`worker.worker_main` message loop (mock mode).

Spawns the worker as a subprocess with ``ANVILML_WORKER_MOCK=1`` and
exercises the IPC framing protocol: InitializeHardware -> Ready,
Ping -> Pong, MemoryQuery -> MemoryReport, Shutdown -> Dying + exit 0.
"""

import os
import struct
import subprocess
import sys
import time

import msgpack
import pytest

# The worker script path (absolute so it works regardless of cwd).
# __file__ is worker/tests/test_worker_main.py, so we go up two levels to repo root.
_WORKER_SCRIPT = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "worker_main.py",
)


def _make_frame(data: dict) -> bytes:
    """Build a length-prefixed msgpack frame."""
    payload = msgpack.packb(data, use_bin_type=True)
    header = struct.pack(">I", len(payload))
    return header + payload


class TestWorkerMain:
    """Integration tests that spawn the worker subprocess."""

    @staticmethod
    def _parse_frames(data: bytes) -> list[dict]:
        """Parse length-prefixed msgpack frames from raw bytes."""
        frames = []
        offset = 0
        while offset + 4 <= len(data):
            length = struct.unpack(">I", data[offset:offset + 4])[0]
            offset += 4
            if offset + length > len(data):
                break  # Incomplete frame.
            payload = data[offset:offset + length]
            frames.append(msgpack.unpackb(payload, raw=False))
            offset += length
        return frames

    def _spawn_worker(self, worker_id: str = "test-0", device_index: int = 0):
        """Spawn the worker in mock mode."""
        env = os.environ.copy()
        env["ANVILML_WORKER_MOCK"] = "1"

        proc = subprocess.Popen(
            [sys.executable, _WORKER_SCRIPT, "--worker-id", worker_id,
             "--device-index", str(device_index)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            env=env,
        )
        return proc

    def test_ready_on_init_hardware(self):
        """InitializeHardware triggers Ready{} with mock values."""
        proc = self._spawn_worker()

        try:
            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)
            ready = next(f for f in parsed if f["_type"] == "Ready")
            dying = next(f for f in parsed if f["_type"] == "Dying")

            assert ready["worker_id"] == "test-0"
            assert ready["device_index"] == 0
            assert dying["reason"] == "shutdown"
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_mock_values(self):
        """Mock Ready payload matches spec values."""
        proc = self._spawn_worker()

        try:
            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)
            ready = next(f for f in parsed if f["_type"] == "Ready")

            assert ready["vram_total_mib"] == 8192
            assert ready["vram_free_mib"] == 8192
            assert ready["arch"] == "gfx1100"
            assert ready["fp16"] is True
            assert ready["bf16"] is True
            assert ready["flash_attention"] is False
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_ping_pong(self):
        """Worker receives Ping{seq} and responds with Pong{seq}."""
        proc = self._spawn_worker()

        try:
            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame({"_type": "Ping", "seq": 42})
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)
            ready = next(f for f in parsed if f["_type"] == "Ready")
            pong = next(f for f in parsed if f["_type"] == "Pong")

            assert ready["worker_id"] == "test-0"
            assert pong["seq"] == 42
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_memory_query_report(self):
        """Worker receives MemoryQuery{} and responds with MemoryReport{0, 0}."""
        proc = self._spawn_worker()

        try:
            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame({"_type": "MemoryQuery"})
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)
            ready = next(f for f in parsed if f["_type"] == "Ready")
            mem_report = next(f for f in parsed if f["_type"] == "MemoryReport")

            assert ready["worker_id"] == "test-0"
            assert mem_report["vram_used_mib"] == 0
            assert mem_report["ram_used_mib"] == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_shutdown_dying_exit(self):
        """Worker receives Shutdown{}, responds Dying{reason: shutdown}, exits 0."""
        proc = self._spawn_worker()

        try:
            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)
            dying = next(f for f in parsed if f["_type"] == "Dying")

            assert dying["reason"] == "shutdown"
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_double_init_exits(self):
        """Sending two InitializeHardware frames: first produces Ready, second
        produces no response event. Worker responds to Shutdown with Dying +
        exit 0. Guards against re-introduction of the double-InitializeHardware
        write bug (P10-B1). The Python worker's `ready_sent` guard ensures
        exactly one Ready is emitted regardless of how many InitializeHardware
        frames arrive."""
        proc = self._spawn_worker()

        try:
            # Send two InitializeHardware frames in sequence.
            init_frame = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            proc.stdin.write(init_frame)
            proc.stdin.write(init_frame)
            # Then send Shutdown to trigger a clean exit.
            shutdown_frame = _make_frame({"_type": "Shutdown"})
            proc.stdin.write(shutdown_frame)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)

            # There should be exactly one Ready event.
            ready_events = [f for f in parsed if f["_type"] == "Ready"]
            assert len(ready_events) == 1, (
                f"expected exactly one Ready, got {len(ready_events)}"
            )
            assert ready_events[0]["worker_id"] == "test-0"
            assert ready_events[0]["device_index"] == 0

            # There should be exactly one Dying event (from Shutdown).
            dying_events = [f for f in parsed if f["_type"] == "Dying"]
            assert len(dying_events) == 1, (
                f"expected exactly one Dying, got {len(dying_events)}"
            )
            assert dying_events[0]["reason"] == "shutdown"

            # The process should exit cleanly.
            assert proc.returncode == 0, f"worker exited with code {proc.returncode}"
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()

    def test_execute_progress_completed(self):
        """Execute{job_id, graph, settings, device_index} triggers N Progress
        events followed by a single Completed event."""
        proc = self._spawn_worker()

        try:
            # Build a mock graph with 3 nodes.
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

            frames = _make_frame({"_type": "InitializeHardware", "device_str": "cuda:0"})
            frames += _make_frame(execute_msg)
            frames += _make_frame({"_type": "Shutdown"})
            proc.stdin.write(frames)
            proc.stdin.close()

            stdout_data = proc.stdout.read(4096)
            proc.wait(timeout=5)

            parsed = self._parse_frames(stdout_data)

            # Exactly one Ready event with correct worker_id.
            ready_events = [f for f in parsed if f["_type"] == "Ready"]
            assert len(ready_events) == 1, (
                f"expected exactly one Ready, got {len(ready_events)}"
            )
            assert ready_events[0]["worker_id"] == "test-0"

            # N Progress events (N = number of nodes).
            progress_events = [f for f in parsed if f["_type"] == "Progress"]
            assert len(progress_events) == len(nodes), (
                f"expected {len(nodes)} Progress events, got {len(progress_events)}"
            )
            for i, node in enumerate(nodes):
                pe = progress_events[i]
                assert pe["job_id"] == "exec-test-1"
                assert pe["node_index"] == i
                assert pe["node_total"] == len(nodes)
                assert pe["node_type"] == node["type"]

            # Exactly one Completed event.
            completed_events = [f for f in parsed if f["_type"] == "Completed"]
            assert len(completed_events) == 1, (
                f"expected exactly one Completed, got {len(completed_events)}"
            )
            completed = completed_events[0]
            assert completed["job_id"] == "exec-test-1"
            assert completed["elapsed_ms"] >= 0

            # Exactly one Dying event, exit code 0.
            dying_events = [f for f in parsed if f["_type"] == "Dying"]
            assert len(dying_events) == 1
            assert dying_events[0]["reason"] == "shutdown"
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
