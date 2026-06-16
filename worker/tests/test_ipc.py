"""Tests for worker.ipc — ZeroMQ DEALER transport module.

Each test creates its own zmq socket instances to ensure complete isolation.
All tests share zmq.Context.instance() (singleton) but use unique ports or
in-process PAIR sockets.

Tests use zmq.ROUTER/zmq.DEALER for identity and message-routing tests
(since ROUTER exposes the DEALER's identity frame), and zmq.PAIR for
the pure roundtrip test (PAIR has no identity frames, so it verifies
the msgpack encoding/decoding without the routing layer).
"""

from __future__ import annotations

import msgpack
import zmq

from worker import ipc


def _reset_ipc_state() -> None:
    """Reset module-level globals to pre-connect state.

    This is needed because ipc.connect() mutates module-level _ctx and _sock,
    and tests that verify RuntimeError-before-connect need _sock to be None.
    Closes the old DEALER socket before discarding the reference to prevent
    lingering connections that interfere with subsequent tests.
    """
    if ipc._sock is not None:
        ipc._sock.close(linger=0)
    ipc._ctx = None
    ipc._sock = None


def test_connect_succeeds():
    """Verify that connect() creates a valid DEALER socket and sets _sock.

    Preconditions:
        A ROUTER socket is bound on a random ephemeral port.

    Expects:
        ipc._sock is not None and ipc._ctx is not None after connect() returns.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")
        assert ipc._sock is not None
        assert ipc._ctx is not None
    finally:
        router.close(linger=0)


def test_connect_sets_identity():
    """Verify DEALER socket identity frame is set correctly and visible on ROUTER.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().

    Expects:
        The ROUTER's multipart receive yields an identity frame equal to b"test-worker".
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        ipc.send_event({"_type": "Ping"})

        # ROUTER returns multipart: [identity, data]
        identity = router.recv()
        assert identity == b"test-worker"
    finally:
        router.close(linger=0)


def test_send_event_encodes_type_discriminator():
    """Verify send_event() serialises the _type discriminator key correctly.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().

    Expects:
        msgpack.unpackb on the received data yields a dict with _type == "Ready"
        and all payload fields preserved.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        payload = {"_type": "Ready", "node_types": ["LoadModel"]}
        ipc.send_event(payload)

        # ROUTER receives multipart: [identity, data]
        router.recv()  # identity frame
        raw = router.recv()
        received = msgpack.unpackb(raw, raw=False)
        assert received["_type"] == "Ready"
        assert received["node_types"] == ["LoadModel"]
    finally:
        router.close(linger=0)


def test_recv_message_deserialises_correctly():
    """Verify recv_message() deserialises a msgpack message from the ROUTER correctly.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().
        The ROUTER sends a msgpack-serialised dict back to the DEALER.

    Expects:
        recv_message() returns a dict that exactly matches the payload sent by the ROUTER.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        # Brief wait for ZeroMQ connection to establish between ROUTER and DEALER.
        import time
        time.sleep(0.1)

        payload = {"_type": "DispatchJob", "job_id": "abc-123"}
        # ROUTER sends multipart: [identity, data] to the DEALER
        router.send_multipart(
            [b"test-worker", msgpack.packb(payload, use_bin_type=True)]
        )

        received = ipc.recv_message()
        assert received == payload
    finally:
        router.close(linger=0)


def test_roundtrip_via_pair_sockets():
    """Verify msgpack roundtrip via in-process PAIR sockets.

    This test does not involve the ROUTER/DEALER identity routing layer.
    It creates two PAIR sockets connected in-process, packs data with
    msgpack on one end, and unpacks on the other — verifying the
    encoding/decoding mechanism that ipc.py relies on.

    Preconditions:
        Two PAIR sockets are connected in-process (bind first, then connect).

    Expects:
        msgpack.unpackb(msgpack.packb(data)) returns a dict identical to the original.
    """
    ctx = zmq.Context.instance()
    p1 = ctx.socket(zmq.PAIR)
    p2 = ctx.socket(zmq.PAIR)
    p1.bind("tcp://127.0.0.1:*")
    addr = p1.getsockopt(zmq.LAST_ENDPOINT)
    p2.connect(addr)
    try:
        data = {"_type": "Ping", "seq": 42}
        packed = msgpack.packb(data, use_bin_type=True)
        p1.send(packed)
        raw = p2.recv()
        received = msgpack.unpackb(raw, raw=False)
        assert received == data
    finally:
        p1.close(linger=0)
        p2.close(linger=0)


def test_send_before_connect_raises():
    """Verify send_event() raises RuntimeError when connect() was not called.

    Preconditions:
        No prior call to connect(); _sock is None (after _reset_ipc_state()).

    Expects:
        RuntimeError is raised by send_event().
    """
    _reset_ipc_state()
    try:
        ipc.send_event({})
    except RuntimeError:
        pass
    else:
        raise AssertionError("send_event() should raise RuntimeError before connect()")


def test_recv_before_connect_raises():
    """Verify recv_message() raises RuntimeError when connect() was not called.

    Preconditions:
        No prior call to connect(); _sock is None (after _reset_ipc_state()).

    Expects:
        RuntimeError is raised by recv_message().
    """
    _reset_ipc_state()
    try:
        ipc.recv_message()
    except RuntimeError:
        pass
    else:
        raise AssertionError("recv_message() should raise RuntimeError before connect()")
