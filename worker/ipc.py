"""ZeroMQ DEALER transport for AnvilML worker IPC.

The Rust supervisor binds a ROUTER socket. This worker connects a DEALER socket
with a stable identity equal to ANVILML_WORKER_ID. Identity frames on outgoing
(worker -> supervisor) messages are handled automatically by ZeroMQ, so
send_event() sends a plain single-frame payload. Incoming (supervisor -> worker)
messages carry an extra empty delimiter frame ahead of the payload -- ROUTER's
own outgoing framing convention, see RouterTransport::send() on the Rust side --
so recv_message() uses recv_multipart() and takes the last frame.
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
    # recv_multipart(), not recv(): RouterTransport::send() (Rust side)
    # always sends a 3-frame ROUTER message [worker_id, empty delimiter,
    # payload] — ROUTER consumes frame 0 for its own routing decision
    # before transmitting, so this DEALER always receives exactly 2 frames
    # on the wire: [empty delimiter, payload]. A single-frame recv() only
    # ever picked up the empty delimiter frame, and msgpack.unpackb(b"")
    # on that empty frame is exactly "Unpack failed: incomplete input" —
    # every message the supervisor ever sent to a worker failed to decode
    # this way. The payload is always the *last* frame regardless of
    # exactly how many leading frames precede it, so this is robust to
    # either framing convention without needing to special-case frame count.
    frames = _sock.recv_multipart()
    return msgpack.unpackb(frames[-1], raw=False)
