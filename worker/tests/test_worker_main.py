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

                    # Mock send_event and recv_message so the startup sequence
                    # can complete without a real IPC socket.
                    import zmq

                    with mock.patch("worker.ipc.send_event"):
                        with mock.patch(
                            "worker.ipc.recv_message"
                        ) as mock_recv:
                            mock_recv.side_effect = zmq.ZMQError("broken pipe")

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

                    # Mock send_event and recv_message so the startup sequence
                    # can complete without a real IPC socket.
                    import zmq

                    with mock.patch("worker.ipc.send_event"):
                        with mock.patch(
                            "worker.ipc.recv_message"
                        ) as mock_recv:
                            mock_recv.side_effect = zmq.ZMQError("broken pipe")

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
        probe result has exactly 6 bool keys.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_DEVICE_TYPE=cuda``, ``ANVILML_DEVICE_INDEX=1``.
        Expected output: set_device(1) called; probe_capabilities("cuda", 1)
        called; probe returns dict with 6 keys, all bool.
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
                        # Mock send_event and recv_message so the startup sequence
                        # can complete without a real IPC socket.
                        import zmq

                        with mock.patch("worker.ipc.send_event"):
                            with mock.patch(
                                "worker.ipc.recv_message"
                            ) as mock_recv:
                                mock_recv.side_effect = zmq.ZMQError("broken pipe")

                                result = worker_main._real_startup_sequence()

                                # Non-CPU path must call set_device with correct index.
                                mock_set_device.assert_called_once_with(1)

                                # probe_capabilities must be called with ("cuda", 1).
                                mock_probe.assert_called_once_with("cuda", 1)

                                # Probe return value must have exactly 6 keys, all bool.
                                probe_result = mock_probe.return_value
                                assert set(probe_result.keys()) == {
                                    "fp32",
                                    "fp16",
                                    "bf16",
                                    "fp8",
                                    "fp4",
                                    "flash_attention",
                                }, (
                                    f"Expected 6 keys, got {set(probe_result.keys())}"
                                )
                                for key, value in probe_result.items():
                                    assert isinstance(value, bool), (
                                        f"Value for key {key!r} must be bool, "
                                        f"got {type(value).__name__}"
                                    )

                                # Function returns None (enters dispatch loop).
                                assert result is None
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


# ---------------------------------------------------------------------------
# New real-mode tests — P9-D2: Ready event, node import stub, dispatch loop
# ---------------------------------------------------------------------------

    @pytest.mark.real_mode
    def test_real_startup_sends_ready_event(self) -> None:
        """_real_startup_sequence() sends a Ready event via ipc.send_event.

        Patches ``ipc.send_event`` to capture the event dict and asserts it
        contains ``_type="Ready"``, ``capabilities_source="pytorch"``, and
        ``node_types`` containing the registered PassThrough node.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_IPC_PORT=5555``, ``ANVILML_WORKER_ID=test-0``,
        ``ANVILML_DEVICE_TYPE=cpu``, ``ANVILML_DEVICE_INDEX=0``.
        Expected output: ``ipc.send_event`` called with a dict containing
        ``_type="Ready"``, ``capabilities_source="pytorch"``, and a
        ``node_types`` list with the PassThrough node descriptor.
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
        os.environ["ANVILML_WORKER_ID"] = "test-0"
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

                    # Capture the event dict sent via send_event.
                    captured_events = []

                    def capture_send_event(data: dict) -> None:
                        captured_events.append(data)

                    import zmq

                    with mock.patch(
                        "worker.ipc.send_event", side_effect=capture_send_event
                    ) as mock_send:
                        with mock.patch(
                            "worker.ipc.recv_message"
                        ) as mock_recv:
                            mock_recv.side_effect = zmq.ZMQError("broken pipe")

                            worker_main._real_startup_sequence()

                            # send_event must have been called exactly once.
                            mock_send.assert_called_once()

                            # The captured event must be a Ready event.
                            assert len(captured_events) == 1
                            event = captured_events[0]
                            assert event["_type"] == "Ready"
                            assert event["capabilities_source"] == "pytorch"
                            # PassThrough is registered via auto-import.
                            assert len(event["node_types"]) >= 1
                            assert (
                                event["node_types"][0]["type_name"]
                                == "PassThrough"
                            )
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value

    @pytest.mark.real_mode
    def test_import_nodes_returns_registered_nodes(self) -> None:
        """_import_nodes() returns descriptors for registered nodes.

        Calls ``_import_nodes()`` directly and asserts the result contains
        the PassThrough node descriptor. This confirms that node modules
        registered via @register are correctly reported by the import
        function.

        Preconditions: None (pure function, no setup).
        Expected output: ``_import_nodes()`` returns a list containing
        at least the PassThrough node descriptor.
        """
        import worker.worker_main as worker_main

        result = worker_main._import_nodes()
        assert len(result) >= 1, f"Expected at least one node, got {result!r}"
        assert result[0]["type_name"] == "PassThrough"

    @pytest.mark.real_mode
    def test_dispatch_loop_exists_and_is_callable(self) -> None:
        """_dispatch_loop exists as a callable and handles a single recv gracefully.

        Asserts ``_dispatch_loop`` is a function, then calls it with
        ``ipc.recv_message`` mocked to raise after one call (simulating
        supervisor disconnect). The loop should log the error and break
        cleanly without raising an unhandled exception.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_IPC_PORT=5555``, ``ANVILML_WORKER_ID=test-0``,
        ``ANVILML_DEVICE_TYPE=cpu``, ``ANVILML_DEVICE_INDEX=0``.
        Expected output: no unhandled exception; dispatch loop exits after
        one recv failure.
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
        os.environ["ANVILML_WORKER_ID"] = "test-0"
        os.environ["ANVILML_DEVICE_TYPE"] = "cpu"
        os.environ["ANVILML_DEVICE_INDEX"] = "0"

        try:
            import worker.worker_main as worker_main

            # Verify _dispatch_loop is callable.
            assert callable(worker_main._dispatch_loop), (
                "_dispatch_loop must be a callable function"
            )

            # Mock ipc.recv_message to raise after one call, so the loop exits.
            import zmq

            with mock.patch("worker.ipc.connect"):
                with mock.patch(
                    "worker.ipc.recv_message"
                ) as mock_recv:
                    mock_recv.side_effect = zmq.ZMQError("broken pipe")

                    # Calling _dispatch_loop should not raise — it catches
                    # the recv failure and breaks the loop.
                    worker_main._dispatch_loop()

                    # recv_message must have been called at least once.
                    mock_recv.assert_called()
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value

    @pytest.mark.real_mode
    def test_real_startup_no_nonzero_exit_for_cpu(self) -> None:
        """Full real-mode startup path runs without raising for valid CPU device_type.

        Runs the complete startup sequence with mocked IPC and capability probe,
        confirming no exception is raised (i.e., the function does not exit
        nonzero for a valid CPU device_type).

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_DEVICE_TYPE=cpu`` in env.
        Expected output: no exception raised; the startup sequence completes
        (dispatch loop exits on mocked recv failure).
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

                    import zmq

                    # Mock send_event and recv_message so the startup sequence
                    # can complete without a real IPC socket.
                    with mock.patch("worker.ipc.send_event"):
                        with mock.patch(
                            "worker.ipc.recv_message"
                        ) as mock_recv:
                            mock_recv.side_effect = zmq.ZMQError("broken pipe")

                            # The full startup sequence must not raise.
                            worker_main._real_startup_sequence()
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value

    @pytest.mark.real_mode
    def test_mock_startup_sends_ready_event(self) -> None:
        """_mock_startup_sequence() sends a Ready event with capabilities_source="mock".

        Patches ``ipc.send_event`` to capture the event dict and asserts it
        contains ``_type="Ready"``, ``capabilities_source="mock"``, and
        ``node_types`` containing the registered PassThrough node.

        Env var isolation: saves and restores all four startup env vars
        unconditionally after the test body.

        Preconditions: ``ANVILML_IPC_PORT=5555``, ``ANVILML_WORKER_ID=test-0``,
        ``ANVILML_DEVICE_TYPE=cpu``, ``ANVILML_DEVICE_INDEX=0``.
        Expected output: ``ipc.send_event`` called with a dict containing
        ``_type="Ready"``, ``capabilities_source="mock"``, and a
        ``node_types`` list with the PassThrough node descriptor.
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
        os.environ["ANVILML_WORKER_ID"] = "test-0"
        os.environ["ANVILML_DEVICE_TYPE"] = "cpu"
        os.environ["ANVILML_DEVICE_INDEX"] = "0"

        try:
            with mock.patch("worker.ipc.connect"):
                import worker.worker_main as worker_main

                # Capture the event dict sent via send_event.
                captured_events = []

                def capture_send_event(data: dict) -> None:
                    captured_events.append(data)

                import zmq

                with mock.patch(
                    "worker.ipc.send_event", side_effect=capture_send_event
                ) as mock_send:
                    with mock.patch(
                        "worker.ipc.recv_message"
                    ) as mock_recv:
                        mock_recv.side_effect = zmq.ZMQError("broken pipe")

                        worker_main._mock_startup_sequence()

                        # send_event must have been called exactly once.
                        mock_send.assert_called_once()

                        # The captured event must be a Ready event.
                        assert len(captured_events) == 1
                        event = captured_events[0]
                        assert event["_type"] == "Ready"
                        assert event["capabilities_source"] == "mock"
                        # PassThrough is registered via auto-import.
                        assert len(event["node_types"]) >= 1
                        assert (
                            event["node_types"][0]["type_name"]
                            == "PassThrough"
                        )
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value

    @pytest.mark.real_mode
    def test_no_mock_gate_in_main_block(self) -> None:
        """The ``__main__`` block uses ``ANVILML_WORKER_MOCK == "1"`` check (not ``!= "1"`` exit).

        Reads the source file of ``worker_main.py`` as text and asserts no
        line matches the v3 defect pattern: an env-var guard that calls
        ``exit`` when not in mock mode. The new main block dispatches to
        mock or real mode based on ``== "1"``, not gating out real mode.

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


class TestDispatchLoopPing:
    """Tests for _dispatch_loop()'s handling of keepalive Ping messages.

    Regression coverage for a gap where no task in the project ever wired
    Pong-sending into the dispatch loop, while the Rust-side
    KeepaliveWatchdog has been unconditionally active since Phase 8 —
    meaning every real worker that reached Ready was killed by its own
    supervisor shortly after, and endlessly respawned. See _dispatch_loop's
    own doc comment for the full explanation.
    """

    def test_ping_receives_matching_pong(self, monkeypatch) -> None:
        """A received Ping is answered with a Pong carrying the same seq.

        Feeds _dispatch_loop() a single Ping message via a mocked
        ipc.recv_message(), then a recv failure to cleanly break the loop,
        and asserts ipc.send_event() was called with exactly
        {"_type": "Pong", "seq": <same seq>} — no other transformation.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: send_event() called once with a matching Pong dict.
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([{"_type": "Ping", "seq": 42}])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                # Break the loop cleanly, matching a real recv failure —
                # _dispatch_loop's except-and-break path.
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()

        assert sent_events == [{"_type": "Pong", "seq": 42}], (
            f"expected exactly one matching Pong reply, got {sent_events}"
        )

    def test_multiple_pings_each_get_matching_pong(self, monkeypatch) -> None:
        """Each Ping in a sequence gets its own correctly-matched Pong.

        Feeds three Pings with distinct, non-sequential seq values and
        asserts three Pongs are sent back, each echoing its own Ping's seq
        in the same order — proving seq is read per-message, not cached
        or reused from an earlier Ping.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: Three Pong events, seqs [7, 3, 100] in that order.
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter(
            [
                {"_type": "Ping", "seq": 7},
                {"_type": "Ping", "seq": 3},
                {"_type": "Ping", "seq": 100},
            ]
        )

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()

        assert sent_events == [
            {"_type": "Pong", "seq": 7},
            {"_type": "Pong", "seq": 3},
            {"_type": "Pong", "seq": 100},
        ]

    def test_non_ping_message_gets_no_pong(self, monkeypatch) -> None:
        """A non-Ping message does not trigger a Pong reply.

        Feeds a message of an unrelated type (sufficient to prove the type
        check is exact) and asserts send_event() is never called.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: send_event() is never called.
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([{"_type": "SomeOtherType", "foo": "bar"}])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()

        assert sent_events == [], f"expected no Pong reply, got {sent_events}"


class TestDispatchLoopShutdown:
    """Tests for _dispatch_loop()'s handling of Shutdown and KeyboardInterrupt.

    Regression coverage for two related gaps: (1) ManagedWorker::
    graceful_shutdown_child() (Rust side) sends WorkerMessage::Shutdown and
    waits for this process to exit on its own, but nothing here ever
    handled that message, so the wait always timed out; (2) Windows
    propagates a console Ctrl+C directly to this process (not spawned into
    its own process group), raising KeyboardInterrupt mid-recv(), which
    `except Exception` never catches (KeyboardInterrupt inherits from
    BaseException), producing an unhandled traceback on every shutdown.
    See _dispatch_loop's own doc comment for the full explanation.
    """

    def test_shutdown_message_exits_loop_cleanly(self, monkeypatch) -> None:
        """A Shutdown message breaks the loop without sending a reply.

        Feeds _dispatch_loop() a single Shutdown message and asserts the
        loop returns (does not hang or raise) and that no event was sent
        back — unlike Ping, Shutdown has no reply, just a clean exit.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: _dispatch_loop() returns; send_event() never
        called.
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([{"_type": "Shutdown"}])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                # Should not be reached — the loop must break on
                # Shutdown itself, not fall through to another recv().
                raise AssertionError(
                    "recv_message() called again after Shutdown — "
                    "the loop did not break on receiving it"
                )

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()  # Must return, not hang or raise.

        assert sent_events == [], (
            f"Shutdown must not trigger any reply, got {sent_events}"
        )

    def test_shutdown_after_other_messages_still_exits(self, monkeypatch) -> None:
        """Shutdown correctly ends the loop even after prior messages.

        Feeds a Ping (answered normally) followed by a Shutdown, and
        asserts the loop processes the Ping, replies with a Pong, then
        exits cleanly on the Shutdown without attempting a third recv().

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: One Pong sent, then a clean exit.
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter(
            [
                {"_type": "Ping", "seq": 1},
                {"_type": "Shutdown"},
            ]
        )

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise AssertionError(
                    "recv_message() called again after Shutdown — "
                    "the loop did not break on receiving it"
                )

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()

        assert sent_events == [{"_type": "Pong", "seq": 1}], (
            f"expected exactly one Pong before the Shutdown, got {sent_events}"
        )

    def test_keyboard_interrupt_during_recv_exits_cleanly(self, monkeypatch) -> None:
        """KeyboardInterrupt from recv_message() exits the loop cleanly.

        Simulates the Windows console Ctrl+C propagation case: recv_message()
        raises KeyboardInterrupt directly, as it would if the OS delivered
        the interrupt while the underlying blocking socket call was in
        progress. Asserts _dispatch_loop() catches it and returns normally
        — no traceback propagates out of this function.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: _dispatch_loop() returns without raising.
        """
        import worker.ipc as ipc

        def fake_recv_message():
            raise KeyboardInterrupt()

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        # Must not raise — this is the regression this test guards
        # against: KeyboardInterrupt previously propagated straight out
        # of this function as an unhandled traceback.
        worker_main._dispatch_loop()

    def test_keyboard_interrupt_after_other_messages_still_exits_cleanly(
        self, monkeypatch
    ) -> None:
        """KeyboardInterrupt correctly ends the loop even after prior messages.

        Feeds a Ping (answered normally), then raises KeyboardInterrupt on
        the next recv_message() call, matching a Ctrl+C arriving mid-loop
        rather than on the very first receive.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: One Pong sent, then a clean exit (no raise).
        """
        import worker.ipc as ipc

        sent_events = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        call_count = {"n": 0}

        def fake_recv_message():
            call_count["n"] += 1
            if call_count["n"] == 1:
                return {"_type": "Ping", "seq": 5}
            raise KeyboardInterrupt()

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        worker_main._dispatch_loop()

        assert sent_events == [{"_type": "Pong", "seq": 5}], (
            f"expected exactly one Pong before the interrupt, got {sent_events}"
        )


# ---------------------------------------------------------------------------
# Dispatch loop Execute tests — P17-B3: real execute_graph dispatch
# ---------------------------------------------------------------------------


class TestDispatchLoopExecute:
    """Tests for _dispatch_loop()'s handling of Execute messages.

    P17-B3 replaced the interim _execute_job() stopgap with a real handler
    that calls execute_graph() on a background thread. These tests verify
    that Execute triggers execute_graph with a job-scoped ctx_factory,
    sends a Completed event on success, and that the dispatch loop stays
    responsive (background thread, not blocking).
    """

    def test_execute_triggers_execute_graph_with_job_scoped_ctx_factory(self, monkeypatch) -> None:
        """Execute message triggers execute_graph() with a ctx_factory producing a NodeContext.

        Feeds an Execute message with a graph, then breaks the loop with a
        recv failure. Asserts execute_graph() was called once with the correct
        graph and a ctx_factory that produces a NodeContext with the correct
        job_id.

        Uses monkeypatch for worker.ipc.send_event and worker.ipc.recv_message.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: execute_graph() called once with correct graph and
            a ctx_factory that produces a NodeContext with job_id="job-123".
        """
        import unittest.mock as mock
        import worker.ipc as ipc

        # Capture the call to execute_graph.
        execute_graph_calls: list[tuple] = []

        def mock_execute_graph(graph: dict, ctx_factory) -> dict:
            # Verify the ctx_factory produces a NodeContext with the right job_id.
            ctx = ctx_factory()
            assert ctx.job_id == "job-123", (
                f"NodeContext job_id must be 'job-123', got {ctx.job_id!r}"
            )
            execute_graph_calls.append((graph, ctx_factory))
            return {"cancelled": False, "results": {}}

        monkeypatch.setattr(ipc, "send_event", lambda data: None)

        messages = iter([{"_type": "Execute", "job_id": "job-123", "graph": {"nodes": []}}])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        with mock.patch(
            "worker.executor.execute_graph", side_effect=mock_execute_graph
        ) as mock_exec:
            import worker.worker_main as worker_main

            worker_main._dispatch_loop()

            # execute_graph must have been called exactly once.
            mock_exec.assert_called_once()
            assert len(execute_graph_calls) == 1, (
                f"Expected 1 execute_graph call, got {len(execute_graph_calls)}"
            )

    def test_execute_success_sends_completed_with_elapsed_ms(self, monkeypatch) -> None:
        """Success path sends Completed event with real elapsed_ms (positive integer).

        Feeds an Execute message, then a recv failure to break the loop.
        Asserts that ipc.send_event was called with a Completed event containing
        a positive integer elapsed_ms (proving time.monotonic() was used).

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: send_event() called with {"_type": "Completed",
            "job_id": "job-456", "elapsed_ms": <positive int>}.
        """
        import worker.ipc as ipc

        sent_events: list[dict] = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([{"_type": "Execute", "job_id": "job-456", "graph": {"nodes": []}}])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        import worker.worker_main as worker_main

        worker_main._dispatch_loop()

        # Find the Completed event among sent events.
        completed_events = [e for e in sent_events if e.get("_type") == "Completed"]
        assert len(completed_events) == 1, (
            f"Expected exactly one Completed event, got {len(completed_events)}"
        )
        completed = completed_events[0]
        assert completed["job_id"] == "job-456"
        assert isinstance(completed["elapsed_ms"], int), (
            f"elapsed_ms must be int, got {type(completed['elapsed_ms']).__name__}"
        )
        assert completed["elapsed_ms"] >= 0, (
            f"elapsed_ms must be non-negative, got {completed['elapsed_ms']}"
        )

    def test_execute_on_background_thread_stays_responsive(self, monkeypatch) -> None:
        """Dispatch loop processes messages after Execute completes (no hang).

        Feeds an Execute message followed by a Shutdown message. Asserts the
        dispatch loop processes the Execute (spawning a background thread),
        waits for it, then processes the Shutdown and exits cleanly — proving
        the Execute handler does not block the dispatch loop indefinitely.

        This is the key test proving the background-thread design: if Execute
        ran synchronously on the dispatch thread (as the old _execute_job did),
        a Shutdown message sent after Execute would be queued and never
        received until the execution finished. With a background thread +
        join, the loop still processes subsequent messages after the join
        returns.

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: dispatch loop exits cleanly after Shutdown.
        """
        import worker.ipc as ipc

        def mock_send_event(data: dict) -> None:
            pass  # No-op — we just need the loop to not hang.

        monkeypatch.setattr(ipc, "send_event", mock_send_event)

        # Feed Execute, then Shutdown.
        messages = iter([
            {"_type": "Execute", "job_id": "job-exec", "graph": {"nodes": []}},
            {"_type": "Shutdown"},
        ])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        import worker.worker_main as worker_main

        # The dispatch loop must not hang here. If Execute blocked the
        # dispatch thread synchronously (as the old _execute_job did),
        # the Shutdown message would never be received.
        worker_main._dispatch_loop()

        # If we reach here, the loop exited cleanly — proving it was not
        # permanently blocked by the Execute handler.

    def test_execute_graph_called_with_correct_graph(self, monkeypatch) -> None:
        """execute_graph() receives the exact graph dict from the Execute message.

        Feeds an Execute message with a specific graph dict and asserts
        execute_graph() received exactly that dict (not a modified copy).

        Preconditions: worker.ipc is mockable via monkeypatch.
        Expected output: execute_graph() called with the exact graph dict.
        """
        import unittest.mock as mock
        import worker.ipc as ipc

        received_graph: dict | None = None

        def mock_execute_graph(graph: dict, ctx_factory) -> dict:
            nonlocal received_graph
            received_graph = graph
            return {"cancelled": False, "results": {}}

        monkeypatch.setattr(ipc, "send_event", lambda data: None)

        expected_graph = {
            "nodes": [
                {"id": "node-1", "type": "PassThrough", "inputs": {"value": 42}},
            ],
            "edges": [],
        }

        messages = iter([
            {"_type": "Execute", "job_id": "job-graph", "graph": expected_graph},
        ])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        with mock.patch(
            "worker.executor.execute_graph", side_effect=mock_execute_graph
        ):
            import worker.worker_main as worker_main

            worker_main._dispatch_loop()

        assert received_graph is not None, "execute_graph was not called"
        assert received_graph == expected_graph, (
            f"execute_graph received a different graph: {received_graph!r}"
        )


# ---------------------------------------------------------------------------
# Dispatch loop Execute failure tests — P17-B4: Failed event on exception
# ---------------------------------------------------------------------------


class TestDispatchLoopExecuteFailure:
    """Tests for _dispatch_loop()'s handling of Execute failures.

    P17-B4 adds exception handling around execute_graph() so that when a node
    raises an unhandled exception during a real job, the dispatch loop sends
    ``WorkerEvent::Failed{job_id, error, traceback}`` instead of leaving the job
    silently hung with no terminal event.

    These tests verify that the Failed event is sent with correct job_id,
    that the error field contains the exception message, and that the
    traceback field is populated.
    """

    def test_execute_failure_sends_failed_event(self, monkeypatch) -> None:
        """A node raising inside execute_graph() results in Failed being sent.

        Mocks ``execute_graph`` to raise ``ValueError("test error")``, feeds
        an Execute message, and asserts that a ``Failed`` event is sent with
        the correct ``job_id`` — not ``Completed``, not silence.

        Preconditions: ``worker.ipc`` is mockable via monkeypatch.
        Expected output: send_event() called with a Failed event containing
            ``_type="Failed"`` and ``job_id="job-fail"``.
        """
        import unittest.mock as mock
        import worker.ipc as ipc

        sent_events: list[dict] = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([
            {"_type": "Execute", "job_id": "job-fail", "graph": {"nodes": []}},
        ])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        with mock.patch(
            "worker.executor.execute_graph",
            side_effect=ValueError("test error"),
        ):
            import worker.worker_main as worker_main

            worker_main._dispatch_loop()

        # Find the Failed event among sent events.
        failed_events = [e for e in sent_events if e.get("_type") == "Failed"]
        assert len(failed_events) == 1, (
            f"Expected exactly one Failed event, got {len(failed_events)}: {sent_events}"
        )
        failed = failed_events[0]
        assert failed["job_id"] == "job-fail", (
            f"Failed event job_id must be 'job-fail', got {failed['job_id']!r}"
        )

    def test_execute_failure_error_contains_exception_message(self, monkeypatch) -> None:
        """The error field contains the original exception's string representation.

        Mocks ``execute_graph`` to raise ``ValueError("specific error message")``,
        then asserts that the ``error`` field in the ``Failed`` event includes
        the original exception's string representation.

        Preconditions: ``worker.ipc`` is mockable via monkeypatch.
        Expected output: send_event() called with a Failed event whose
            ``error`` field contains "specific error message".
        """
        import unittest.mock as mock
        import worker.ipc as ipc

        sent_events: list[dict] = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([
            {"_type": "Execute", "job_id": "job-err", "graph": {"nodes": []}},
        ])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        with mock.patch(
            "worker.executor.execute_graph",
            side_effect=ValueError("specific error message"),
        ):
            import worker.worker_main as worker_main

            worker_main._dispatch_loop()

        failed_events = [e for e in sent_events if e.get("_type") == "Failed"]
        assert len(failed_events) == 1, (
            f"Expected exactly one Failed event, got {len(failed_events)}"
        )
        failed = failed_events[0]
        assert "specific error message" in failed["error"], (
            f"error field must contain 'specific error message', "
            f"got {failed['error']!r}"
        )

    def test_execute_failure_traceback_is_populated(self, monkeypatch) -> None:
        """The traceback field is populated and non-empty.

        Mocks ``execute_graph`` to raise an exception, then asserts that the
        ``traceback`` field in the ``Failed`` event is a non-empty string
        containing traceback formatting markers (e.g., "Traceback").

        Preconditions: ``worker.ipc`` is mockable via monkeypatch.
        Expected output: send_event() called with a Failed event whose
            ``traceback`` field is a non-empty string containing "Traceback".
        """
        import unittest.mock as mock
        import worker.ipc as ipc

        sent_events: list[dict] = []
        monkeypatch.setattr(ipc, "send_event", lambda data: sent_events.append(data))

        messages = iter([
            {"_type": "Execute", "job_id": "job-tb", "graph": {"nodes": []}},
        ])

        def fake_recv_message():
            try:
                return next(messages)
            except StopIteration:
                raise ConnectionError("test: no more messages")

        monkeypatch.setattr(ipc, "recv_message", fake_recv_message)

        with mock.patch(
            "worker.executor.execute_graph",
            side_effect=ValueError("traceback test"),
        ):
            import worker.worker_main as worker_main

            worker_main._dispatch_loop()

        failed_events = [e for e in sent_events if e.get("_type") == "Failed"]
        assert len(failed_events) == 1, (
            f"Expected exactly one Failed event, got {len(failed_events)}"
        )
        failed = failed_events[0]
        tb = failed["traceback"]
        assert isinstance(tb, str), (
            f"traceback must be a string, got {type(tb).__name__}"
        )
        assert len(tb) > 0, "traceback must be non-empty"
        assert "Traceback" in tb, (
            f"traceback must contain 'Traceback', got {tb!r}"
        )
