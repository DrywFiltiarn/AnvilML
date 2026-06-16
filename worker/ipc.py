"""ZeroMQ DEALER transport for AnvilML worker IPC.

This module provides the network communication layer between the Python worker
subprocess and the Rust supervisor's ROUTER socket. It uses ZeroMQ's DEALER
socket type with TCP transport and msgpack serialisation.

The module maintains module-level globals (_ctx, _sock) that are initialised
by a single call to connect() and used by send_event() and recv_message().
"""

from __future__ import annotations

import msgpack
import zmq

_ctx: zmq.Context | None = None
_sock: zmq.Socket | None = None


def connect(port: int, worker_id: str) -> None:
    """Connect DEALER socket to the ROUTER at *port*.

    Must be called exactly once before any send/recv operation. Creates a
    module-level DEALER socket bound to the given TCP port and sets the
    socket identity to *worker_id* (ZeroMQ constraint: identity must be set
    before connect).

    Args:
        port: TCP port on 127.0.0.1 where the Rust ROUTER is bound.
        worker_id: Stable worker identity string (e.g. "worker-0").

    Raises:
        RuntimeError: If connect() has already been called (no-op for idempotency).
    """
    global _ctx, _sock

    # ZeroMQ requires identity to be set on the socket before binding/connecting.
    # This is a hard constraint of the DEALER socket type — setting it after
    # connect() has no effect and the remote side will see an empty identity frame.
    _ctx = zmq.Context.instance()
    _sock = _ctx.socket(zmq.DEALER)
    _sock.setsockopt(zmq.IDENTITY, worker_id.encode())
    _sock.connect(f"tcp://127.0.0.1:{port}")


def send_event(data: dict) -> None:
    """Send a msgpack-serialised event dict to the ROUTER socket.

    The data dict is serialised with msgpack (binary mode) and sent over
    the DEALER socket. A guard check prevents silent failures if the worker
    lifecycle is broken (e.g. connect() was never called).

    Args:
        data: A flat dict with a ``_type`` key as the event discriminator
            (e.g. ``{"_type": "Ready", "node_types": [...]}``).

    Raises:
        RuntimeError: If connect() has not been called yet.
    """
    # Guard check prevents silent failures if worker lifecycle is broken.
    if _sock is None:
        raise RuntimeError("connect() must be called before send_event()")

    _sock.send(msgpack.packb(data, use_bin_type=True))


def recv_message() -> dict:
    """Receive and deserialise a msgpack-encoded message from the ROUTER.

    Waits for a message on the DEALER socket, then deserialises it with
    msgpack. A guard check prevents silent failures if the worker lifecycle
    is broken.

    Returns:
        A flat dict deserialised from the msgpack payload. The ``_type`` key
        identifies the message variant (e.g. ``"DispatchJob"``).

    Raises:
        RuntimeError: If connect() has not been called yet.
    """
    # Guard check prevents silent failures if worker lifecycle is broken.
    if _sock is None:
        raise RuntimeError("connect() must be called before recv_message()")

    data = _sock.recv()
    return msgpack.unpackb(data, raw=False)
