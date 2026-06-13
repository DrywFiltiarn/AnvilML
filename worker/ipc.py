"""ZeroMQ DEALER transport for AnvilML worker IPC.

Provides msgpack-serialised message exchange over a ZeroMQ DEALER socket
bound to ``tcp://127.0.0.1:{port}`` for communication between the Rust
server and Python worker processes.

The Rust supervisor binds a ROUTER socket and passes the port via the
``ANVILML_IPC_PORT`` environment variable.  The worker connects to
``tcp://127.0.0.1:{ANVILML_IPC_PORT}`` at startup.

Functions
---------
:func:`connect` — connect to the ZeroMQ DEALER socket (sets ``_sock``).
:func:`read_frame` — read a msgpack frame from the active transport.
:func:`write_frame` — write a msgpack frame to the active transport.
"""

import zmq

import msgpack

# Module-level transport handle — set by connect().
_sock: zmq.Socket | None = None


def connect(port: int) -> None:
    """Connect to the ZeroMQ DEALER socket at *port*.

    Args:
        port: TCP port to connect to on ``127.0.0.1``.
    """
    global _sock
    ctx = zmq.Context.instance()
    _sock = ctx.socket(zmq.DEALER)
    _sock.connect(f"tcp://127.0.0.1:{port}")


def read_frame() -> object:
    """Read a single msgpack frame from the active transport.

    Returns:
        The deserialized payload as a Python object.

    Raises:
        RuntimeError: If not connected — call :func:`connect` first.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect(port) first")
    data = _sock.recv()
    return msgpack.unpackb(data, raw=False)


def write_frame(data: object) -> None:
    """Write a single msgpack frame to the active transport.

    Args:
        data: Python object to serialise.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect(port) first")
    _sock.send(msgpack.packb(data, use_bin_type=True))
