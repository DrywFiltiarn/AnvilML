"""Tests for :mod:`worker.ipc` framing protocol."""

import io
import re
import sys
from unittest import mock

import pytest

import worker.ipc as ipc


class TestReadFrame:
    """Tests for :func:`worker.ipc.read_frame`."""

    def test_write_read_roundtrip(self) -> None:
        """write_frame + read_frame preserves data correctly."""
        payload = {
            "job_id": "test-1",
            "status": "running",
            "meta": {"node": "zit", "seed": 42},
        }

        buf = io.BytesIO()
        stdout_mock = mock.MagicMock()
        stdout_mock.buffer = buf

        stdin_mock = mock.MagicMock()
        stdin_mock.buffer = buf

        with mock.patch.object(sys, "stdout", stdout_mock):
            with mock.patch.object(sys, "stdin", stdin_mock):
                ipc.write_frame(payload)

        # Reset buffer position so read_frame reads from the start.
        buf.seek(0)

        with mock.patch.object(sys, "stdin", stdin_mock):
            result = ipc.read_frame()

        assert result == payload

    def test_roundtrip_with_bytes(self) -> None:
        """Roundtrip preserves raw bytes payloads."""
        payload = {"image": b"\x89PNG\r\n\x1a\n"}

        buf = io.BytesIO()
        stdout_mock = mock.MagicMock()
        stdout_mock.buffer = buf

        stdin_mock = mock.MagicMock()
        stdin_mock.buffer = buf

        with mock.patch.object(sys, "stdout", stdout_mock):
            ipc.write_frame(payload)

        buf.seek(0)

        with mock.patch.object(sys, "stdin", stdin_mock):
            result = ipc.read_frame()

        assert result == payload

    def test_roundtrip_empty_dict(self) -> None:
        """Roundtrip works with an empty dict."""
        payload: dict = {}

        buf = io.BytesIO()
        stdout_mock = mock.MagicMock()
        stdout_mock.buffer = buf

        stdin_mock = mock.MagicMock()
        stdin_mock.buffer = buf

        with mock.patch.object(sys, "stdout", stdout_mock):
            ipc.write_frame(payload)

        buf.seek(0)

        with mock.patch.object(sys, "stdin", stdin_mock):
            result = ipc.read_frame()

        assert result == payload


class TestWindowsGuard:
    """Tests for the Windows binary-stdio guard."""

    @pytest.mark.skipif(sys.platform != "win32", reason="Windows-only guard")
    def test_windows_binary_mode_guard_present(self) -> None:
        """msvcrt.setmode is called with os.O_BINARY on stdin and stdout."""
        source = ipc.__file__
        with open(source, encoding="utf-8") as f:
            text = f.read()

        # Check that the guard pattern exists in the source.
        pattern = r"msvcrt\.setmode\s*\(\s*sys\.stdin\.fileno\(\)\s*,\s*os\.O_BINARY\s*\)"
        assert re.search(pattern, text), "stdin binary-mode guard not found"

        pattern_stdout = (
            r"msvcrt\.setmode\s*\(\s*sys\.stdout\.fileno\(\)\s*,\s*os\.O_BINARY\s*\)"
        )
        assert re.search(pattern_stdout, text), "stdout binary-mode guard not found"

    def test_guard_code_exists_in_source(self) -> None:
        """The msvcrt.setmode pattern exists regardless of platform.

        This variant runs on all platforms to ensure the guard cannot be
        accidentally removed during development.
        """
        source = ipc.__file__
        with open(source, encoding="utf-8") as f:
            text = f.read()

        # The guard should reference msvcrt.setmode in the source.
        assert "msvcrt.setmode" in text, (
            "Windows binary-mode guard (msvcrt.setmode) not found in ipc.py"
        )
