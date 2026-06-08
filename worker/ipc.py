"""Socket framing protocol for AnvilML worker IPC.

Provides binary-framed msgpack serialization over a Unix domain socket
(or Windows named pipe) for communication between the Rust server and
Python worker processes.

The Rust supervisor creates the socket and passes its path via the
``ANVILML_IPC_SOCKET`` environment variable.  The worker connects to
this socket at startup.  If ``ANVILML_IPC_SOCKET`` is not set
(e.g. during testing), the worker falls back to stdin/stdout transport.

Functions
---------
:func:`connect` — connect to the IPC socket (sets ``_sock``).
:func:`read_frame` — read a framed message from the active transport.
:func:`write_frame` — write a framed message to the active transport.
"""

import io
import os
import socket
import struct
import sys

import msgpack

# Module-level transport handle — set by connect() or lazily resolved.
_sock: socket.socket | None = None


def connect(path: str) -> None:
    """Connect to the IPC socket at *path*.

    On Unix/macOS this opens an AF_UNIX stream socket.
    On Windows it opens a named pipe via ``CreateFileW`` and wraps it
    in a socket-like object.

    Args:
        path: Filesystem path to the Unix socket (Unix) or named pipe
            path (Windows).
    """
    global _sock
    if sys.platform == "win32":
        import ctypes

        GENERIC_READ = 0x80000000
        GENERIC_WRITE = 0x40000000
        OPEN_EXISTING = 3
        FILE_FLAG_OVERLAPPED = 0x40000000

        handle = ctypes.windll.kernel32.CreateFileW(
            path,
            GENERIC_READ | GENERIC_WRITE,
            0,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
        if handle == -1:
            raise OSError(
                f"CreateFileW failed for pipe '{path}': "
                f"{ctypes.FormatError()}"
            )
        _sock = _WindowsPipeSocket(handle)  # type: ignore[assignment]
    else:
        _sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        _sock.connect(path)


def _get_transport() -> object:
    """Return the active transport (socket or stdin/stdout).

    If ``_sock`` is set (via :func:`connect`), returns the socket.
    Otherwise lazily resolves ``ANVILML_IPC_SOCKET`` and connects,
    or falls back to stdin/stdout if the env var is unset.
    """
    if _sock is not None:
        return _sock

    # Try to connect from the env var.
    path = os.environ.get("ANVILML_IPC_SOCKET")
    if path is not None:
        connect(path)
        return _sock

    # Fall back to stdin/stdout.
    return _StdioTransport()


class _StdioTransport:
    """Transport that reads from stdin and writes to stdout."""

    def recv(self, n: int) -> bytes:
        """Read up to *n* bytes from stdin.buffer."""
        return sys.stdin.buffer.read(n)

    def sendall(self, data: bytes) -> None:
        """Write all of *data* to stdout.buffer and flush."""
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()


class _WindowsPipeSocket:
    """Thin wrapper around a Windows named-pipe HANDLE for send/recv."""

    def __init__(self, handle: int) -> None:
        self._handle = handle

    def recv(self, bufsize: int) -> bytes:
        """Read up to *bufsize* bytes from the pipe."""
        import ctypes
        import ctypes.wintypes

        buf = ctypes.create_string_buffer(bufsize)
        bytes_read = ctypes.wintypes.DWORD()
        ok = ctypes.windll.kernel32.ReadFile(
            self._handle,
            buf,
            bufsize,
            ctypes.byref(bytes_read),
            None,
        )
        if not ok:
            raise EOFError("pipe: unexpected end of input")
        return buf.raw[: bytes_read.value]

    def sendall(self, data: bytes) -> None:
        """Send all of *data* to the pipe."""
        import ctypes
        import ctypes.wintypes

        bytes_written = ctypes.wintypes.DWORD()
        ok = ctypes.windll.kernel32.WriteFile(
            self._handle,
            data,
            len(data),
            ctypes.byref(bytes_written),
            None,
        )
        if not ok:
            raise OSError(f"WriteFile failed: {ctypes.FormatError()}")


def read_frame() -> object:
    """Read a single framed message from the active transport.

    Reads a 4-byte big-endian unsigned length prefix, then that many
    bytes of msgpack-encoded payload.  Returns the deserialized Python
    object.

    Returns:
        The deserialized payload as a Python object.

    Raises:
        EOFError: If the transport is closed before the full frame is read.
    """
    transport = _get_transport()

    # Read exactly 4 bytes for the length prefix.
    length_bytes = b""
    while len(length_bytes) < 4:
        chunk = transport.recv(4 - len(length_bytes))
        if not chunk:
            raise EOFError("read_frame: unexpected end of input")
        length_bytes += chunk

    length = struct.unpack(">I", length_bytes)[0]

    # Read exactly N bytes for the payload.
    payload = b""
    while len(payload) < length:
        chunk = transport.recv(length - len(payload))
        if not chunk:
            raise EOFError(
                f"read_frame: expected {length} bytes, got EOF after "
                f"{len(payload)}"
            )
        payload += chunk

    return msgpack.unpackb(payload, raw=False)


def write_frame(data: object) -> None:
    """Write a single framed message to the active transport.

    Serialises *data* with msgpack (binary mode), prepends a 4-byte
    big-endian length prefix, and sends the combined frame.

    Args:
        data: Python object to serialise.
    """
    transport = _get_transport()
    payload = msgpack.packb(data, use_bin_type=True, default=str)
    header = struct.pack(">I", len(payload))
    transport.sendall(header + payload)
