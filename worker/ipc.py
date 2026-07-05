"""ZeroMQ DEALER transport for AnvilML worker IPC.

The Rust supervisor binds a ROUTER socket. This worker connects a DEALER socket
with a stable identity equal to ANVILML_WORKER_ID. Identity frames are handled
automatically by ZeroMQ; application code sends/receives plain msgpack dicts.
"""

import zmq
import msgpack

_ctx: zmq.Context | None = None
_sock: zmq.Socket | None = None


def connect(port: int, worker_id: str) -> None:
    """Connect DEALER socket to the ROUTER at *port*, using *worker_id* as identity.

    Must be called exactly once before any send/recv operation.

    Args:
        port: TCP port on 127.0.0.1 where the Rust ROUTER is bound.
        worker_id: Stable worker identity string — the bare device index as
            injected via ANVILML_WORKER_ID in production (e.g. "0").
    """
    global _ctx, _sock
    _ctx = zmq.Context.instance()
    _sock = _ctx.socket(zmq.DEALER)
    # Set identity before connect — required by ZeroMQ ROUTER socket topology;
    # the ROUTER uses this identity to route replies back to the correct DEALER.
    _sock.setsockopt(zmq.IDENTITY, worker_id.encode())
    _sock.connect(f"tcp://127.0.0.1:{port}")


def send_event(data: dict) -> None:
    """Send a WorkerEvent dict to the Rust supervisor.

    Args:
        data: Dict with '_type' key and event fields.

    Raises:
        RuntimeError: If connect() has not been called.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect() first")
    _sock.send(msgpack.packb(data, use_bin_type=True))


def recv_message() -> dict:
    """Receive the next WorkerMessage from the Rust supervisor. Blocks until a
    message arrives.

    Returns:
        Dict with '_type' key and message fields.

    Raises:
        RuntimeError: If connect() has not been called.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect() first")
    data = _sock.recv()
    return msgpack.unpackb(data, raw=False)
