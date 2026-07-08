"""Tests for worker.ipc — ZeroMQ DEALER transport module."""

import subprocess
import sys

import msgpack
import pytest
import zmq

import worker.ipc as ipc


def _teardown_ipc() -> None:
    """Close the DEALER socket and reset module globals for test isolation.

    This is called at the end of every test that uses ipc.connect() to ensure
    subsequent tests start with a clean slate — _sock is None, _ctx is None.
    """
    try:
        if ipc._sock is not None:
            ipc._sock.close()
    except zmq.ZMQError:
        pass  # Socket may already be closed
    finally:
        ipc._sock = None
        ipc._ctx = None


def _make_test_router(port: int) -> tuple[zmq.Socket, zmq.Context]:
    """Create a ROUTER socket on a **fresh** context for test isolation.

    The ROUTER must be on the same context as the DEALER for ZeroMQ's
    ROUTER-to-DEALER identity notification to work reliably — the ROUTER
    tracks connections through its context's endpoint table.

    Returns the ROUTER socket and the owning context (so the caller can
    terminate the context on teardown).
    """
    ctx = zmq.Context()
    router = ctx.socket(zmq.ROUTER)
    router.bind(f"tcp://127.0.0.1:{port}")
    router.setsockopt(zmq.RCVTIMEO, 5000)
    return router, ctx


class TestConnectIdentity:
    """Tests for connect() and identity setup."""

    def setup_method(self) -> None:
        """Reset module globals before each test."""
        _teardown_ipc()

    def teardown_method(self) -> None:
        """Ensure cleanup even on test failure."""
        _teardown_ipc()

    def test_connect_sets_identity(self) -> None:
        """DEALER socket connects with correct ZEROMQ identity.

        Starts a ROUTER socket on a random port, calls connect() on a DEALER
        socket, then sends a ping from the DEALER and verifies the ROUTER
        receives a message from the correct identity. The ROUTER socket's
        first recv() returns the identity frame, the second returns the
        msgpack payload.
        """
        port = 15555
        worker_id = "test-worker"

        # Create a fresh ROUTER on its own context — ZeroMQ ROUTER identity
        # notifications require the ROUTER to share a context with the DEALER.
        router, router_ctx = _make_test_router(port)
        try:
            # Connect the DEALER via the ipc module (same approach the real
            # worker uses).
            ipc.connect(port, worker_id)

            # Send a ping from the DEALER — in ZeroMQ 4+, the ROUTER only
            # returns the identity frame when the DEALER sends a message.
            ipc.send_event({"_type": "Ping"})

            # ROUTER's first recv() returns the identity frame.
            identity = router.recv()
            assert identity == worker_id.encode(), (
                f"Expected identity {worker_id!r}, got {identity!r}"
            )

            # Second recv() returns the msgpack payload.
            raw = router.recv()
            payload = msgpack.unpackb(raw, raw=False)
            assert payload == {"_type": "Ping"}, (
                f"Expected ping payload, got {payload!r}"
            )
        finally:
            router.close()
            router_ctx.term()


class TestPreConnectErrors:
    """Tests for pre-connect RuntimeError on send_event and recv_message."""

    def setup_method(self) -> None:
        """Reset module globals before each test."""
        _teardown_ipc()

    def teardown_method(self) -> None:
        """Ensure cleanup even on test failure."""
        _teardown_ipc()

    def test_send_event_before_connect_raises(self) -> None:
        """send_event raises RuntimeError when not connected.

        Calls send_event() without calling connect() first and asserts
        RuntimeError is raised with the expected message.
        """
        with pytest.raises(RuntimeError, match="ipc: not connected"):
            ipc.send_event({"_type": "Ping"})

    def test_recv_message_before_connect_raises(self) -> None:
        """recv_message raises RuntimeError when not connected.

        Calls recv_message() without calling connect() first and asserts
        RuntimeError is raised with the expected message.
        """
        with pytest.raises(RuntimeError, match="ipc: not connected"):
            ipc.recv_message()


class TestRoundtrip:
    """Tests for full send/recv round-trip against a live ROUTER socket."""

    def setup_method(self) -> None:
        """Reset module globals before each test."""
        _teardown_ipc()

    def teardown_method(self) -> None:
        """Ensure cleanup even on test failure."""
        _teardown_ipc()

    def test_roundtrip_send_recv(self) -> None:
        """Full msgpack round-trip via ROUTER/DEALER pair.

        Sets up a ROUTER socket in the test process, connects a DEALER via
        ipc.connect(), sends a dict via ipc.send_event(), receives it from the
        ROUTER side via router.recv() (identity frame) + router.recv()
        (payload), unpacks with msgpack.unpackb(raw=False), and asserts the
        dict matches the sent payload. This is the real integration test that
        proves the full send/recv path works end-to-end within a single process.
        """
        port = 15556
        worker_id = "roundtrip-worker"
        expected = {"_type": "Ping", "payload": "hello"}

        # Create a fresh ROUTER on its own context for isolation.
        router, router_ctx = _make_test_router(port)
        try:
            # Connect the DEALER via the ipc module.
            ipc.connect(port, worker_id)

            # Send a dict via ipc.send_event().
            ipc.send_event(expected)

            # ROUTER's first recv() returns the identity frame.
            router.recv()
            # Second recv() returns the msgpack payload.
            raw = router.recv()
            received = msgpack.unpackb(raw, raw=False)

            assert received == expected, (
                f"Expected {expected}, got {received}"
            )
        finally:
            router.close()
            router_ctx.term()


class TestNoTorchImport:
    """Tests for module import isolation (no transitive torch dependency)."""

    def test_module_no_torch_import(self) -> None:
        """Module does not transitively import torch.

        Uses subprocess.run() to spawn a fresh Python process that imports
        worker.ipc and asserts "torch" not in sys.modules. This confirms the
        module has no transitive torch dependency at import time (required by
        the mock-mode CI jobs that install only base.txt without torch).

        Uses subprocess isolation (not sys.modules manipulation) per
        ENVIRONMENT.md §11.3 — sys.modules.pop("torch") crashed the WSL2
        agent VM at the OS level in prior development.
        """
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import worker.ipc; import sys; "
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


class TestContextReuse:
    """Tests for zmq.Context singleton reuse."""

    def teardown_method(self) -> None:
        """Ensure cleanup even on test failure."""
        _teardown_ipc()

    def test_connect_twice_reuses_context(self) -> None:
        """Second connect() call reuses zmq.Context singleton.

        Calls connect() twice with different worker IDs; verifies the second
        call reuses the existing context but creates a new socket with the new
        identity. This tests the singleton pattern works correctly.
        """
        port = 15557

        # First connect.
        ipc.connect(port, "worker-a")
        first_ctx = ipc._ctx
        first_sock = ipc._sock

        # Second connect with different worker ID.
        ipc.connect(port, "worker-b")
        second_ctx = ipc._ctx
        second_sock = ipc._sock

        # The context must be the same singleton instance.
        assert first_ctx is second_ctx, (
            "connect() did not reuse the existing zmq.Context"
        )

        # The socket must be a new instance (the old one is replaced).
        assert first_sock is not second_sock, (
            "connect() did not create a new socket"
        )

        # Create a fresh ROUTER on its own context to verify the new identity.
        router, router_ctx = _make_test_router(port)
        try:
            # Send a message from the DEALER to trigger the identity frame.
            ipc.send_event({"_type": "Ping"})
            # ROUTER's first recv() returns the identity frame.
            identity = router.recv()
            assert identity == b"worker-b", (
                f"Expected new identity b'worker-b', got {identity!r}"
            )
        finally:
            router.close()
            router_ctx.term()


class TestRecvMessage:
    """Tests for recv_message()'s handling of the real ROUTER framing.

    Regression coverage for a gap where recv_message() used a single-frame
    recv(), but the real Rust RouterTransport::send() always sends a
    3-frame ROUTER message [worker_id, empty delimiter, payload] -- ROUTER
    consumes frame 0 for routing before transmitting, so the DEALER peer
    always receives exactly 2 frames on the wire: [empty delimiter,
    payload]. A single-frame recv() only ever picked up the empty
    delimiter, so every message the supervisor ever sent to a real worker
    failed with "Unpack failed: incomplete input". The existing
    TestRoundtrip.test_roundtrip_send_recv only exercises the opposite
    direction (worker -> supervisor via send_event()), which is why this
    was never caught.
    """

    def setup_method(self) -> None:
        """Reset module globals before each test."""
        _teardown_ipc()

    def teardown_method(self) -> None:
        """Ensure cleanup even on test failure."""
        _teardown_ipc()

    def test_recv_message_decodes_real_router_framing(self) -> None:
        """recv_message() correctly decodes a real 2-frame ROUTER message.

        Sets up a test ROUTER, connects a DEALER via ipc.connect(), then
        sends a message the same way RouterTransport::send() actually does
        on the Rust side: router.send_multipart([identity, b"", payload])
        -- ROUTER consumes the identity frame for routing and transmits
        [b"", payload] to the DEALER, matching production exactly (unlike
        a hand-rolled single-frame send, which would not reproduce this
        bug). Asserts recv_message() returns the correct decoded dict.

        Preconditions: A test ROUTER can bind to the given port.
        Expected output: The dict sent by the ROUTER, decoded correctly.
        """
        port = 15558
        worker_id = "recv-test-worker"
        expected = {"_type": "Ping", "seq": 42}

        router, router_ctx = _make_test_router(port)
        try:
            ipc.connect(port, worker_id)

            # The ROUTER only learns a DEALER's route once that DEALER has
            # sent at least one message — the TCP handshake alone isn't
            # enough for pyzmq's ROUTER to address a reply back to it. Send
            # a throwaway message to register, then drain both frames the
            # ROUTER sees for it (identity, payload) before the real test.
            ipc.send_event({"_type": "Ping", "seq": 0})
            router.recv()  # identity frame
            router.recv()  # throwaway payload frame

            # Send exactly as RouterTransport::send() does: identity frame
            # (consumed by ROUTER for routing, never reaches the wire),
            # empty delimiter frame, payload frame. The DEALER receives
            # only the last two frames.
            router.send_multipart(
                [
                    worker_id.encode(),
                    b"",
                    msgpack.packb(expected, use_bin_type=True),
                ]
            )

            received = ipc.recv_message()
            assert received == expected, (
                f"Expected {expected}, got {received}"
            )
        finally:
            router.close()
            router_ctx.term()

    def test_recv_message_raises_before_connect(self) -> None:
        """recv_message() raises RuntimeError when not connected.

        Duplicates the existing coverage in TestPreConnectErrors for this
        function specifically, alongside the new multipart-framing test in
        this class, so this class is a self-contained regression suite for
        recv_message()'s behavior.

        Preconditions: ipc._sock is None (fresh module state).
        Expected output: RuntimeError containing "ipc: not connected".
        """
        with pytest.raises(RuntimeError, match="ipc: not connected"):
            ipc.recv_message()
