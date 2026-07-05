"""Tests for worker.worker_main — mock capability probe."""

import subprocess
import sys

import pytest

import worker.worker_main as worker_main


class TestMockProbeCapabilities:
    """Tests for _mock_probe_capabilities() return value."""

    def test_returns_six_required_keys(self) -> None:
        """Function returns a dict with exactly the 6 required keys.

        Calls ``_mock_probe_capabilities()`` and asserts the result has
        exactly the 6 expected keys matching ``InferenceCaps`` field names:
        ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``.

        Preconditions: None (pure function, no setup).
        Expected output: Dict with exactly 6 keys, no more, no fewer.
        """
        result = worker_main._mock_probe_capabilities()

        expected_keys = {
            "fp32",
            "fp16",
            "bf16",
            "fp8",
            "fp4",
            "flash_attention",
        }
        assert set(result.keys()) == expected_keys, (
            f"Expected keys {expected_keys}, got {set(result.keys())}"
        )

    def test_all_values_are_bool(self) -> None:
        """All 6 values in the returned dict are bool type.

        Iterates over all returned values and asserts each is
        ``isinstance(value, bool)`` — not ``int``, ``str``, or other type.

        Preconditions: None (pure function, no setup).
        Expected output: Every ``isinstance(v, bool)`` is True.
        """
        result = worker_main._mock_probe_capabilities()

        for key, value in result.items():
            assert isinstance(value, bool), (
                f"Value for key {key!r} must be bool, "
                f"got {type(value).__name__}"
            )

    def test_fp4_is_false(self) -> None:
        """The fp4 key specifically maps to False.

        Asserts that ``result["fp4"] is False`` — the one deliberate
        exception in the synthetic values. Torch 2.x has no native fp4
        dtype, so this is universally False.

        Preconditions: None (pure function, no setup).
        Expected output: ``result["fp4"] is False``.
        """
        result = worker_main._mock_probe_capabilities()
        assert result["fp4"] is False, (
            f"fp4 must be False (no native torch.float4 dtype), "
            f"got {result['fp4']!r}"
        )


class TestNoTorchImport:
    """Tests for module import isolation (no transitive torch dependency)."""

    def test_no_torch_import_on_module_load(self) -> None:
        """Importing worker_main does not transitively import torch.

        Spawns a fresh subprocess via ``subprocess.run()`` that imports
        ``worker.worker_main`` and asserts ``"torch" not in sys.modules``.
        This confirms the module has no transitive torch dependency at
        import time (required by the mock-mode CI jobs that install only
        base.txt without torch).

        Uses subprocess isolation (not ``sys.modules`` manipulation) per
        ENVIRONMENT.md §11.3 — ``sys.modules.pop("torch")`` crashed the
        WSL2 agent VM at the OS level in prior development.

        Uses ``timeout=10`` per ENVIRONMENT.md §11.3 bounded-wait pattern.

        Preconditions: None.
        Expected output: Subprocess exit code 0, stdout contains "OK".
        """
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import worker.worker_main; import sys; "
                "assert 'torch' not in sys.modules; print('OK')",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        assert result.returncode == 0, (
            f"Subprocess failed: stdout={result.stdout!r}, "
            f"stderr={result.stderr!r}"
        )


# ---------------------------------------------------------------------------
# Real-mode tests — require torch and real IPC environment
# ---------------------------------------------------------------------------

import os

import pytest


class TestRealStartupSequence:
    """Tests for _real_startup_sequence() real-mode startup path."""

    @pytest.mark.real_mode
    def test_real_startup_calls_ipc_connect(self) -> None:
        """_real_startup_sequence() calls ipc.connect with correct env var values.

        Patches ``ipc.connect`` to verify it is called with the port and
        worker_id values read from ``ANVILML_IPC_PORT`` and
        ``ANVILML_WORKER_ID`` environment variables.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_IPC_PORT=5555``, ``ANVILML_WORKER_ID=test-worker-0``,
        ``ANVILML_DEVICE_TYPE=cpu``, ``ANVILML_DEVICE_INDEX=0`` set in env.
        Expected output: ``ipc.connect(5555, "test-worker-0")`` called exactly once.
        """
        import unittest.mock as mock

        # Save env vars before mutating — process-global, must restore.
        saved = {
            "ANVILML_IPC_PORT": os.environ.get("ANVILML_IPC_PORT"),
            "ANVILML_WORKER_ID": os.environ.get("ANVILML_WORKER_ID"),
            "ANVILML_DEVICE_TYPE": os.environ.get("ANVILML_DEVICE_TYPE"),
            "ANVILML_DEVICE_INDEX": os.environ.get("ANVILML_DEVICE_INDEX"),
        }

        # Set test env vars.
        os.environ["ANVILML_IPC_PORT"] = "5555"
        os.environ["ANVILML_WORKER_ID"] = "test-worker-0"
        os.environ["ANVILML_DEVICE_TYPE"] = "cpu"
        os.environ["ANVILML_DEVICE_INDEX"] = "0"

        try:
            with mock.patch("worker.ipc.connect") as mock_connect:
                with mock.patch(
                    "worker.capability.probe_capabilities"
                ) as mock_probe:
                    mock_probe.return_value = {
                        "fp32": True,
                        "fp16": True,
                        "bf16": True,
                        "fp8": False,
                        "fp4": False,
                        "flash_attention": True,
                    }
                    import worker.worker_main as worker_main

                    worker_main._real_startup_sequence()

                    # Verify ipc.connect was called with correct args.
                    mock_connect.assert_called_once_with(5555, "test-worker-0")
        finally:
            # Unconditional restore — runs even on assertion failure.
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


    @pytest.mark.real_mode
    def test_real_startup_cpu_skips_cuda_set_device(self) -> None:
        """CPU device_type skips torch.cuda.set_device entirely.

        Sets ``ANVILML_DEVICE_TYPE=cpu`` and verifies that
        ``torch.cuda.set_device`` is NOT called — CPU has no per-device
        selection, torch uses "cpu" implicitly.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_DEVICE_TYPE=cpu`` in env.
        Expected output: ``torch.cuda.set_device`` not called;
        ``probe_capabilities("cpu", 0)`` called.
        """
        import unittest.mock as mock

        # Save env vars before mutating.
        saved = {
            "ANVILML_IPC_PORT": os.environ.get("ANVILML_IPC_PORT"),
            "ANVILML_WORKER_ID": os.environ.get("ANVILML_WORKER_ID"),
            "ANVILML_DEVICE_TYPE": os.environ.get("ANVILML_DEVICE_TYPE"),
            "ANVILML_DEVICE_INDEX": os.environ.get("ANVILML_DEVICE_INDEX"),
        }

        os.environ["ANVILML_IPC_PORT"] = "5555"
        os.environ["ANVILML_WORKER_ID"] = "cpu-worker"
        os.environ["ANVILML_DEVICE_TYPE"] = "cpu"
        os.environ["ANVILML_DEVICE_INDEX"] = "0"

        try:
            with mock.patch("worker.ipc.connect"):
                with mock.patch(
                    "worker.capability.probe_capabilities"
                ) as mock_probe:
                    mock_probe.return_value = {
                        "fp32": True,
                        "fp16": True,
                        "bf16": True,
                        "fp8": False,
                        "fp4": False,
                        "flash_attention": True,
                    }
                    import worker.worker_main as worker_main

                    with mock.patch(
                        "torch.cuda.set_device"
                    ) as mock_set_device:
                        worker_main._real_startup_sequence()

                        # CPU path must NOT call set_device.
                        mock_set_device.assert_not_called()

                        # probe_capabilities must be called with ("cpu", 0).
                        mock_probe.assert_called_once_with("cpu", 0)
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


    @pytest.mark.real_mode
    def test_real_startup_calls_probe_capabilities(self) -> None:
        """Non-CPU device calls torch.cuda.set_device AND probe_capabilities.

        Sets ``ANVILML_DEVICE_TYPE=cuda`` and ``ANVILML_DEVICE_INDEX=1``,
        then verifies both ``torch.cuda.set_device(1)`` and
        ``capability.probe_capabilities("cuda", 1)`` are called, and the
        returned dict has exactly 6 bool keys.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_DEVICE_TYPE=cuda``, ``ANVILML_DEVICE_INDEX=1``.
        Expected output: set_device(1) called; probe_capabilities("cuda", 1)
        called; returned dict has 6 keys, all bool.
        """
        import unittest.mock as mock

        # Save env vars before mutating.
        saved = {
            "ANVILML_IPC_PORT": os.environ.get("ANVILML_IPC_PORT"),
            "ANVILML_WORKER_ID": os.environ.get("ANVILML_WORKER_ID"),
            "ANVILML_DEVICE_TYPE": os.environ.get("ANVILML_DEVICE_TYPE"),
            "ANVILML_DEVICE_INDEX": os.environ.get("ANVILML_DEVICE_INDEX"),
        }

        os.environ["ANVILML_IPC_PORT"] = "5555"
        os.environ["ANVILML_WORKER_ID"] = "cuda-worker"
        os.environ["ANVILML_DEVICE_TYPE"] = "cuda"
        os.environ["ANVILML_DEVICE_INDEX"] = "1"

        try:
            with mock.patch("worker.ipc.connect"):
                with mock.patch(
                    "worker.capability.probe_capabilities"
                ) as mock_probe:
                    expected_caps = {
                        "fp32": True,
                        "fp16": True,
                        "bf16": True,
                        "fp8": True,
                        "fp4": False,
                        "flash_attention": True,
                    }
                    mock_probe.return_value = expected_caps

                    import worker.worker_main as worker_main

                    with mock.patch(
                        "torch.cuda.set_device"
                    ) as mock_set_device:
                        result = worker_main._real_startup_sequence()

                        # Non-CPU path must call set_device with correct index.
                        mock_set_device.assert_called_once_with(1)

                        # probe_capabilities must be called with ("cuda", 1).
                        mock_probe.assert_called_once_with("cuda", 1)

                        # Returned dict must have exactly 6 keys, all bool.
                        assert set(result.keys()) == {
                            "fp32",
                            "fp16",
                            "bf16",
                            "fp8",
                            "fp4",
                            "flash_attention",
                        }, (
                            f"Expected 6 keys, got {set(result.keys())}"
                        )
                        for key, value in result.items():
                            assert isinstance(value, bool), (
                                f"Value for key {key!r} must be bool, "
                                f"got {type(value).__name__}"
                            )
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


class TestNoMockGate:
    """Mechanical checks that no mock-only gate exists in worker_main.py."""

    @pytest.mark.real_mode
    def test_no_mock_gate_exit_path(self) -> None:
        """No ``if ANVILML_WORKER_MOCK != "1": exit(1)`` gate in worker_main.py.

        Reads the source file of ``worker_main.py`` as text and asserts no
        line matches the v3 defect pattern: an env-var guard that calls
        ``exit`` when not in mock mode.

        Preconditions: None.
        Expected output: Zero lines match the mock-gate pattern.
        """
        import pathlib

        source = pathlib.Path(__file__).resolve().parent.parent / "worker_main.py"
        text = source.read_text()

        # Mechanical check: no line contains the v3 defect pattern of
        # guarding real-mode with an env-var exit.
        for line in text.splitlines():
            stripped = line.strip()
            # Skip comments and string literals.
            if stripped.startswith("#"):
                continue
            assert 'ANVILML_WORKER_MOCK' not in stripped or "exit" not in stripped, (
                f"Mock gate pattern found: {stripped!r}"
            )
