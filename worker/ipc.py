"""IPC communication module for the AnvilML worker.

Handles bidirectional frame-based communication between the
`sindristudio` launcher and the Python worker process via
standard I/O (stdio).

Design notes
------------
On Windows, stdio must be opened in binary mode to prevent
CRLF translation from corrupting length-prefixed binary frames.
This module applies that guard at import time so all subsequent
I/O is correct regardless of call-site.
"""

import sys
import os


# ---------------------------------------------------------------------------
# Windows binary-stdio guard (ANVILML_DESIGN.md §7.1)
# ---------------------------------------------------------------------------
if sys.platform == "win32":
    import msvcrt

    # Re-open stdin/stdout in binary mode so that length-prefixed
    # binary frames are not corrupted by \r\n translation.
    msvcrt.setmode(sys.stdin.fileno(), os.O_BINARY)
    msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)


def read_frame() -> bytes:
    """Read a single length-prefixed binary frame from stdin.

    Expected wire format (future implementation, P2-B2):
        4-byte big-endian unsigned length (N)
        N raw bytes of payload

    Returns
    -------
    bytes
        The decoded payload.
    """
    # TODO: implement actual framing logic (deferred to P2-B2).
    raise NotImplementedError("read_frame stub — not implemented")


def write_frame(data: bytes) -> None:
    """Write a single length-prefixed binary frame to stdout.

    Wire format (future implementation, P2-B2):
        4-byte big-endian unsigned length (N)
        N raw bytes of payload

    Parameters
    ----------
    data : bytes
        The payload to encode and send.
    """
    # TODO: implement actual framing logic (deferred to P2-B2).
    raise NotImplementedError("write_frame stub — not implemented")
