"""Tests for :mod:`worker.ipc` ZeroMQ transport."""

from unittest import mock

import zmq

import msgpack
import pytest

import worker.ipc as ipc


def _zmq_pair():
    """Create an in-process PAIR socket pair and return (a, b, ctx)."""
    ctx = zmq.Context()
    a = ctx.socket(zmq.PAIR)
    b = ctx.socket(zmq.PAIR)
    a.bind("tcp://127.0.0.1:0")
    addr = a.getsockopt(zmq.LAST_ENDPOINT).decode()
    b.connect(addr)
    return a, b, ctx


def _monkeypatch_sock(sock: zmq.Socket) -> mock.MagicMock:
    """Monkeypatch ``ipc._sock`` with a zmq socket and return the patch for cleanup."""
    return mock.patch.object(ipc, "_sock", sock)


class TestReadFrame:
    """Tests for :func:`worker.ipc.read_frame` and :func:`worker.ipc.write_frame`."""

    def test_write_read_roundtrip(self) -> None:
        """write_frame + read_frame preserves data correctly."""
        payload = {
            "job_id": "test-1",
            "status": "running",
            "meta": {"node": "zit", "seed": 42},
        }

        sock_a, sock_b, ctx = _zmq_pair()
        try:
            with _monkeypatch_sock(sock_a):
                # Data written to sock_b is readable from sock_a.
                # Data written to sock_a is readable from sock_b.
                # So: write to sock_b → read_frame reads from sock_a → gets payload.
                sock_b.send(msgpack.packb(payload, use_bin_type=True))
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()
            ctx.term()

    def test_roundtrip_with_bytes(self) -> None:
        """Roundtrip preserves raw bytes payloads."""
        payload = {"image": b"\x89PNG\r\n\x1a\n"}

        sock_a, sock_b, ctx = _zmq_pair()
        try:
            with _monkeypatch_sock(sock_a):
                sock_b.send(msgpack.packb(payload, use_bin_type=True))
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()
            ctx.term()

    def test_roundtrip_empty_dict(self) -> None:
        """Roundtrip works with an empty dict."""
        payload: dict = {}

        sock_a, sock_b, ctx = _zmq_pair()
        try:
            with _monkeypatch_sock(sock_a):
                sock_b.send(msgpack.packb(payload, use_bin_type=True))
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()
            ctx.term()


class TestSocketRoundtrip:
    """Tests that exercise real ZeroMQ I/O via PAIR sockets."""

    def test_socketpair_roundtrip(self) -> None:
        """write_frame + read_frame round-trip over a real PAIR socket pair."""
        payload = {
            "_type": "Ready",
            "worker_id": "worker-0",
            "device_index": 0,
            "vram_total_mib": 8192,
            "vram_free_mib": 8192,
            "arch": "gfx1100",
            "fp16": True,
            "bf16": True,
            "flash_attention": False,
        }

        sock_a, sock_b, ctx = _zmq_pair()
        try:
            with _monkeypatch_sock(sock_a):
                # write_frame writes to sock_a; test reads from sock_b.
                ipc.write_frame(payload)
                # Read the frame from sock_b.
                result = msgpack.unpackb(sock_b.recv(), raw=False)
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()
            ctx.term()

    def test_full_bidirectional_roundtrip(self) -> None:
        """Server sends message, worker reads and responds."""
        server_msg = {"_type": "Ping", "seq": 42}
        worker_response = {"_type": "Pong", "seq": 42}

        sock_a, sock_b, ctx = _zmq_pair()
        try:
            with _monkeypatch_sock(sock_a):
                # Server writes to sock_b; worker reads from sock_a.
                sock_b.send(msgpack.packb(server_msg, use_bin_type=True))
                # Worker reads.
                result = ipc.read_frame()
                assert result == server_msg
                # Worker writes response to sock_a; server reads from sock_b.
                ipc.write_frame(worker_response)
                response = msgpack.unpackb(sock_b.recv(), raw=False)
                assert response == worker_response
        finally:
            sock_a.close()
            sock_b.close()
            ctx.term()

    def test_read_frame_eof(self) -> None:
        """read_frame raises when the other side is closed."""
        sock_a, sock_b, ctx = _zmq_pair()
        try:
            sock_b.close()  # Close the other end
            sock_a.setsockopt(zmq.RCVTIMEO, 1000)  # Timeout so recv doesn't hang
            with _monkeypatch_sock(sock_a):
                with pytest.raises((zmq.Again, zmq.ZMQError, EOFError, OSError)):
                    ipc.read_frame()
        finally:
            sock_a.close()
            ctx.term()
