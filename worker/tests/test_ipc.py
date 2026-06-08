"""Tests for :mod:`worker.ipc` framing protocol."""

import socket
import struct
import sys
from unittest import mock

import msgpack
import pytest

import worker.ipc as ipc


def _monkeypatch_sock(sock: socket.socket) -> mock.MagicMock:
    """Monkeypatch ``ipc._sock`` and return the patch for cleanup."""
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

        sock_a, sock_b = socket.socketpair()
        try:
            with _monkeypatch_sock(sock_a):
                # Data written to sock_b is readable from sock_a.
                # Data written to sock_a is readable from sock_b.
                # So: write to sock_b → read_frame reads from sock_a → gets payload.
                sock_b.sendall(
                    struct.pack(">I", len(msgpack.packb(payload)))
                    + msgpack.packb(payload, use_bin_type=True)
                )
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()

    def test_roundtrip_with_bytes(self) -> None:
        """Roundtrip preserves raw bytes payloads."""
        payload = {"image": b"\x89PNG\r\n\x1a\n"}

        sock_a, sock_b = socket.socketpair()
        try:
            with _monkeypatch_sock(sock_a):
                sock_b.sendall(
                    struct.pack(">I", len(msgpack.packb(payload)))
                    + msgpack.packb(payload, use_bin_type=True)
                )
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()

    def test_roundtrip_empty_dict(self) -> None:
        """Roundtrip works with an empty dict."""
        payload: dict = {}

        sock_a, sock_b = socket.socketpair()
        try:
            with _monkeypatch_sock(sock_a):
                sock_b.sendall(
                    struct.pack(">I", len(msgpack.packb(payload)))
                    + msgpack.packb(payload, use_bin_type=True)
                )
                result = ipc.read_frame()
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()


class TestSocketRoundtrip:
    """Tests that exercise real socket I/O via socketpair."""

    def test_socketpair_roundtrip(self) -> None:
        """write_frame + read_frame round-trip over a real socket pair."""
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

        sock_a, sock_b = socket.socketpair()
        try:
            with _monkeypatch_sock(sock_a):
                # write_frame writes to sock_a; test reads from sock_b.
                ipc.write_frame(payload)
                # Read the frame from sock_b.
                length_bytes = b""
                while len(length_bytes) < 4:
                    chunk = sock_b.recv(4 - len(length_bytes))
                    if not chunk:
                        raise EOFError("read_frame: unexpected end of input")
                    length_bytes += chunk
                length = struct.unpack(">I", length_bytes)[0]
                payload_bytes = b""
                while len(payload_bytes) < length:
                    chunk = sock_b.recv(length - len(payload_bytes))
                    if not chunk:
                        raise EOFError(
                            f"expected {length} bytes, got EOF after "
                            f"{len(payload_bytes)}"
                        )
                    payload_bytes += chunk
                result = msgpack.unpackb(payload_bytes, raw=False)
                assert result == payload
        finally:
            sock_a.close()
            sock_b.close()

    def test_full_bidirectional_roundtrip(self) -> None:
        """Server sends message, worker reads and responds."""
        server_msg = {"_type": "Ping", "seq": 42}
        worker_response = {"_type": "Pong", "seq": 42}

        sock_a, sock_b = socket.socketpair()
        try:
            with _monkeypatch_sock(sock_a):
                # Server writes to sock_b; worker reads from sock_a.
                sock_b.sendall(
                    struct.pack(">I", len(msgpack.packb(server_msg)))
                    + msgpack.packb(server_msg, use_bin_type=True)
                )
                # Worker reads.
                result = ipc.read_frame()
                assert result == server_msg
                # Worker writes response to sock_a; server reads from sock_b.
                ipc.write_frame(worker_response)
                length_bytes = b""
                while len(length_bytes) < 4:
                    chunk = sock_b.recv(4 - len(length_bytes))
                    if not chunk:
                        raise EOFError("server: unexpected end of input")
                    length_bytes += chunk
                length = struct.unpack(">I", length_bytes)[0]
                payload_bytes = b""
                while len(payload_bytes) < length:
                    chunk = sock_b.recv(length - len(payload_bytes))
                    if not chunk:
                        raise EOFError("server: unexpected end of input")
                    payload_bytes += chunk
                response = msgpack.unpackb(payload_bytes, raw=False)
                assert response == worker_response
        finally:
            sock_a.close()
            sock_b.close()

    def test_read_frame_eof(self) -> None:
        """read_frame raises EOFError when the other end is closed."""
        sock_a, sock_b = socket.socketpair()
        try:
            sock_b.close()  # Close the other end → sock_a.recv returns b""
            with _monkeypatch_sock(sock_a):
                with pytest.raises(EOFError, match="read_frame"):
                    ipc.read_frame()
        finally:
            sock_a.close()
