"""Stdio framing protocol for AnvilML worker IPC.

Provides binary-framed msgpack serialization over stdin/stdout for
communication between the Rust server and Python worker subprocesses.
"""

import sys
import struct

import msgpack

# Windows binary-stdio guard: ensure stdin/stdout are in binary mode
# so that framing reads/writes are not corrupted by newline translation.
if sys.platform == "win32":
    import msvcrt
    import os
    try:
        msvcrt.setmode(sys.stdin.fileno(),  os.O_BINARY)
        msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)
    except (io.UnsupportedOperation, AttributeError):
        # stdin/stdout are not real file descriptors (e.g. pytest capture).
        # Binary mode only matters when running as a spawned worker subprocess.
        pass


def read_frame() -> object:
    """Read a single framed message from ``sys.stdin.buffer``.

    Reads a 4-byte big-endian unsigned length prefix, then that many
    bytes of msgpack-encoded payload.  Returns the deserialized Python
    object.

    Returns:
        The deserialized payload as a Python object.
    """
    # Read exactly 4 bytes for the length prefix.
    length_bytes = b""
    while len(length_bytes) < 4:
        chunk = sys.stdin.buffer.read(4 - len(length_bytes))
        if not chunk:
            raise EOFError("read_frame: unexpected end of input")
        length_bytes += chunk

    length = struct.unpack(">I", length_bytes)[0]

    # Read exactly N bytes for the payload.
    payload = b""
    while len(payload) < length:
        chunk = sys.stdin.buffer.read(length - len(payload))
        if not chunk:
            raise EOFError(
                f"read_frame: expected {length} bytes, got EOF after "
                f"{len(payload)}"
            )
        payload += chunk

    return msgpack.unpackb(payload, raw=False)


def write_frame(data: object) -> None:
    """Write a single framed message to ``sys.stdout.buffer``.

    Serialises *data* with msgpack (binary mode), prepends a 4-byte
    big-endian length prefix, writes the combined frame to stdout, and
    flushes.

    Args:
        data: Python object to serialise.
    """
    payload = msgpack.packb(data, use_bin_type=True, default=str)
    header = struct.pack(">I", len(payload))
    sys.stdout.buffer.write(header + payload)
    sys.stdout.buffer.flush()
