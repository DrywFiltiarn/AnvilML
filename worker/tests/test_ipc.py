"""Tests for worker.ipc — ZeroMQ DEALER transport module.

Each test creates its own zmq.socket instances to ensure complete isolation.
All tests share zmq.Context.instance() (singleton) but use unique ports.
"""

from __future__ import annotations

import msgpack
import time
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
        ipc._sock is not None after connect() returns.
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


def test_send_event_roundtrip():
    """Verify send_event + recv_message roundtrip preserves msgpack dict content.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().

    Expects:
        recv_message() returns a dict identical to what send_event() sent.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        payload = {"_type": "Ready", "node_types": []}
        ipc.send_event(payload)

        # ROUTER receives multipart: [identity, data] (no delimiter frame for
        # single-part messages from DEALER)
        identity = router.recv()
        raw = router.recv()
        received = msgpack.unpackb(raw, raw=False)
        assert received == payload
    finally:
        router.close(linger=0)


def test_send_event_type_discriminator():
    """Verify the _type key survives msgpack roundtrip correctly.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().

    Expects:
        recv_message()["_type"] == "Ready".
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        payload = {"_type": "Ready", "node_types": ["LoadCheckpoints"]}
        ipc.send_event(payload)

        # ROUTER receives multipart: [identity, data]
        identity = router.recv()
        raw = router.recv()
        received = msgpack.unpackb(raw, raw=False)
        assert received["_type"] == "Ready"
    finally:
        router.close(linger=0)


def test_recv_message_before_connect_raises():
    """Verify recv_message() raises RuntimeError when connect() was not called.

    Preconditions:
        No prior call to connect(); _sock is None.

    Expects:
        RuntimeError is raised.
    """
    _reset_ipc_state()
    try:
        ipc.recv_message()
    except RuntimeError:
        pass
    else:
        raise AssertionError("recv_message() should raise RuntimeError before connect()")


def test_send_event_before_connect_raises():
    """Verify send_event() raises RuntimeError when connect() was not called.

    Preconditions:
        No prior call to connect(); _sock is None.

    Expects:
        RuntimeError is raised.
    """
    _reset_ipc_state()
    try:
        ipc.send_event({})
    except RuntimeError:
        pass
    else:
        raise AssertionError("send_event() should raise RuntimeError before connect()")


def test_identity_attached():
    """Verify DEALER socket identity frame is set correctly and visible on ROUTER.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected with id "test-identity".

    Expects:
        The ROUTER's multipart receive yields an identity frame equal to b"test-identity".
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-identity")

        ipc.send_event({"_type": "Ready"})

        # ROUTER returns multipart: [identity, data]
        identity = router.recv()
        assert identity == b"test-identity"
    finally:
        router.close(linger=0)


def test_recv_message_from_router():
    """Verify recv_message() can receive a message sent from a ROUTER socket.

    Preconditions:
        A ROUTER socket is bound on a random port; DEALER connected via ipc.connect().
        The ROUTER sends a msgpack-serialised dict back to the DEALER.

    Expects:
        recv_message() returns the dict that the ROUTER sent.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    try:
        ipc.connect(port, "test-worker")

        # Brief wait for ZeroMQ connection to establish between ROUTER and DEALER.
        time.sleep(0.1)

        payload = {"_type": "DispatchJob", "job_id": "abc-123"}
        # ROUTER sends multipart: [identity, data] to the DEALER
        router.send_multipart([b"test-worker", msgpack.packb(payload, use_bin_type=True)])

        received = ipc.recv_message()
        assert received == payload
    finally:
        router.close(linger=0)
