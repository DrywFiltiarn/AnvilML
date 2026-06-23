"""Tests for worker.worker_main — mock-mode worker entry point.

Each test spawns the worker as a subprocess with its own ROUTER socket
on a random port, ensuring complete isolation between tests. The worker
process is cleaned up unconditionally in a ``finally`` block.

All ROUTER receives in this file go through ``_recv_with_timeout``, which
sets an explicit ``zmq.RCVTIMEO`` and surfaces the worker subprocess's
captured stderr if the timeout fires. This exists because an unguarded
``router.recv()`` blocks indefinitely if the worker subprocess dies before
sending the expected message (e.g. on a ``SyntaxError`` at import time) —
see ``docs/ENVIRONMENT.md §11.5`` for the rationale and required pattern.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time

import msgpack
import pytest
import zmq

from worker import ipc

# Default receive timeout (milliseconds) for all bounded ROUTER recv() calls
# in this file. 5 seconds comfortably exceeds the worker's normal startup
# and response latency in mock mode while still failing fast on a dead
# subprocess instead of hanging for the pytest session's outer timeout (or
# indefinitely, with no timeout at all).
_RECV_TIMEOUT_MS = 5000


def _reset_ipc_state() -> None:
    """Reset module-level globals to pre-connect state.

    Closes the old DEALER socket before discarding the reference to
    prevent lingering connections that interfere with subsequent tests.
    """
    if ipc._sock is not None:
        ipc._sock.close(linger=0)
    ipc._ctx = None
    ipc._sock = None


def _make_worker_env(extra: dict[str, str] | None = None) -> dict[str, str]:
    """Build a subprocess environment dict for the worker.

    Copies the parent's environment and sets all required AnvilML
    variables so the worker can start without relying on inherited
    state (os.environ is not inherited through subprocess unless
    the env parameter is explicitly passed).

    Args:
        extra: Additional environment variables to include.

    Returns:
        A complete environment dict ready for subprocess.Popen.
    """
    env = os.environ.copy()
    env["ANVILML_WORKER_MOCK"] = "1"
    if extra:
        env.update(extra)
    return env


def _recv_with_timeout(router: zmq.Socket, proc: subprocess.Popen) -> dict:
    """Receive one [identity, payload] message from *router*, bounded.

    Sets ``zmq.RCVTIMEO`` on *router* before each recv() call so a worker
    subprocess that dies before sending the expected message (e.g. on a
    ``SyntaxError`` at import time, before it ever connects IPC) fails the
    test immediately with a diagnostic message, instead of hanging forever
    on a blocking recv() that can never be satisfied.

    Args:
        router: The bound ROUTER socket to receive from.
        proc: The worker subprocess, used to capture and report stderr if
            the receive times out — this is almost always a worker startup
            failure, not a slow worker, so surfacing stderr turns a silent
            hang into an immediate, self-explanatory diagnosis.

    Returns:
        The msgpack-decoded payload dict (the identity frame is consumed
        and discarded).

    Raises:
        pytest.fail.Exception: If no message arrives within
            ``_RECV_TIMEOUT_MS``. The failure message includes the worker
            subprocess's captured stderr and its exit code if it has
            already terminated.
    """
    router.setsockopt(zmq.RCVTIMEO, _RECV_TIMEOUT_MS)
    try:
        router.recv()  # identity frame
        raw = router.recv()
    except zmq.Again:
        # No message arrived within the timeout. The worker almost
        # certainly died on startup (e.g. a SyntaxError before it could
        # connect IPC and send Ready) rather than being merely slow —
        # surface its stderr and exit status so the failure is
        # immediately diagnosable from the first failed run.
        if proc.poll() is None:
            proc.terminate()
        try:
            _, stderr = proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            _, stderr = proc.communicate(timeout=5)
        pytest.fail(
            f"worker did not send expected message within "
            f"{_RECV_TIMEOUT_MS}ms. worker exit code: {proc.returncode}. "
            f"worker stderr:\n{stderr.decode(errors='replace')}"
        )
    return msgpack.unpackb(raw, raw=False)


def test_mock_startup_sends_ready():
    """Verify the worker emits a valid Ready event on mock startup.

    Preconditions:
        A ROUTER socket is bound on a random port.

    Expects:
        The worker subprocess starts, connects IPC, and sends a Ready
        event with all required fields matching the mock mode spec.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to connect and send its Ready event.
            # The worker needs time to start, connect, and emit Ready.
            time.sleep(0.3)

            # Bounded receive — see module docstring and _recv_with_timeout
            # for why this must never be a raw, unguarded router.recv().
            ready = _recv_with_timeout(router, proc)

            # Verify all required fields from the mock Ready event spec.
            assert ready["_type"] == "Ready"
            assert ready["worker_id"] == "worker-0"
            assert ready["device_index"] == 0
            assert ready["device_name"] == "Mock"
            assert ready["device_type"] == "cpu"
            assert ready["vram_total_mib"] == 8192
            assert ready["vram_free_mib"] == 8192
            assert ready["torch_version"] == "mock"
            assert ready["fp16"] is True
            assert ready["bf16"] is True
            assert ready["fp8"] is True
            assert ready["flash_attention"] is True
            # SaveImage, LoadModel, LoadVae, LoadClip, ClipTextEncode,
            # EmptyLatent, Sampler, and VaeDecode nodes are now
            # registered, so node_types is non-empty. Verify it
            # contains exactly eight entries (the full set of
            # Python-side node types).
            assert isinstance(ready["node_types"], list)
            assert len(ready["node_types"]) == 8
            type_names = {nt["type_name"] for nt in ready["node_types"]}
            assert type_names == {
                "SaveImage",
                "LoadModel",
                "LoadVae",
                "LoadClip",
                "ClipTextEncode",
                "EmptyLatent",
                "Sampler",
                "VaeDecode",
            }
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_ping_returns_pong():
    """Verify the worker responds to Ping with a matching Pong.

    Preconditions:
        The worker is running in mock mode; ROUTER connected to worker.

    Expects:
        A Ping{seq: 42} sent via ROUTER yields a Pong{seq: 42} response.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            # Consume the Ready event that the worker sends on startup.
            # This is required because the ROUTER has the Ready event
            # queued before we send our Ping — we must drain it first.
            # Bounded receive — see module docstring and _recv_with_timeout.
            _recv_with_timeout(router, proc)  # Ready event — discard

            # Send a Ping message to the worker via ROUTER.
            ping_msg = msgpack.packb({"_type": "Ping", "seq": 42}, use_bin_type=True)
            router.send_multipart([b"worker-0", ping_msg])

            # Receive the Pong response. Bounded — see _recv_with_timeout.
            pong = _recv_with_timeout(router, proc)

            assert pong["_type"] == "Pong"
            assert pong["seq"] == 42
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_shutdown_exits_cleanly():
    """Verify the worker exits with code 0 on Shutdown message.

    Preconditions:
        The worker is running in mock mode; ROUTER connected.

    Expects:
        A Shutdown message sent via ROUTER causes the subprocess to
        exit with code 0 within the timeout period.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env({"ANVILML_IPC_PORT": str(port)})
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            # Consume the Ready event that the worker sends on startup.
            # This is required for the same reason as in test_ping_returns_pong:
            # the ROUTER only learns the worker's DEALER identity once a
            # message has actually been received from it, so draining Ready
            # first guarantees the worker is connected before Shutdown is
            # sent — the fixed sleep above is not itself a reliable barrier.
            # Bounded receive — see module docstring and _recv_with_timeout.
            _recv_with_timeout(router, proc)  # Ready event — discard

            # Send a Shutdown message to the worker.
            shutdown_msg = msgpack.packb({"_type": "Shutdown"}, use_bin_type=True)
            router.send_multipart([b"worker-0", shutdown_msg])

            # Wait for the worker to exit. Already bounded — this is the
            # reference pattern cited in docs/ENVIRONMENT.md §11.5.
            proc.wait(timeout=10)
            assert proc.returncode == 0, f"Expected exit code 0, got {proc.returncode}"
        finally:
            # Ensure the process is terminated if it hasn't exited yet.
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_env_vars_read_from_environment():
    """Verify the worker reads custom env vars and includes them in Ready.

    Preconditions:
        A ROUTER socket is bound on a random port; custom env vars set.

    Expects:
        The Ready event contains the custom worker_id, device_index, and
        device_type values passed via environment variables.
    """
    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")
    worker_env = _make_worker_env(
        {
            "ANVILML_IPC_PORT": str(port),
            "ANVILML_WORKER_ID": "custom-worker",
            "ANVILML_DEVICE_INDEX": "3",
            "ANVILML_DEVICE_TYPE": "cuda",
        }
    )
    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            # Bounded receive — see module docstring and _recv_with_timeout.
            ready = _recv_with_timeout(router, proc)

            assert ready["_type"] == "Ready"
            assert ready["worker_id"] == "custom-worker"
            assert ready["device_index"] == 3
            assert ready["device_type"] == "cuda"
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)


def test_pipeline_cache_reused_across_jobs():
    """Verify the same PipelineCache instance is reused across two Execute jobs.

    Preconditions:
        A ROUTER socket is bound on a random port; a temp directory with
        a ``sitecustomize.py`` monkey-patch is on ``PYTHONPATH``.

    Expects:
        Two sequential Execute messages both complete successfully, and the
        captured ``id()`` of the ``pipeline_cache`` argument in
        ``NodeContext.__init__`` is identical for both jobs — proving the
        module-level ``_pipeline_cache`` singleton is shared across jobs.
    """
    import tempfile

    _reset_ipc_state()
    ctx = zmq.Context.instance()
    router = ctx.socket(zmq.ROUTER)
    port = router.bind_to_random_port("tcp://127.0.0.1")

    # Create a temp directory for the monkey-patch sitecustomize module
    # and a temp file for capturing pipeline_cache id() values.
    tmpdir = tempfile.mkdtemp()
    ids_file = os.path.join(tmpdir, "pipeline_cache_ids.txt")

    # Write a sitecustomize.py that monkey-patches NodeContext.__init__
    # to capture the pipeline_cache id() into the ids file.
    # sitecustomize is loaded by Python at startup before any other
    # imports, so the patch is in place before worker.worker_main runs.
    #
    # We must add the repo root to sys.path first, because the venv's
    # site-packages does not include the project root, and the worker
    # package lives there. PYTHONPATH only adds the temp dir (where
    # sitecustomize.py lives), not the repo root.
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    sitecustomize_path = os.path.join(tmpdir, "sitecustomize.py")
    with open(sitecustomize_path, "w") as f:
        f.write(
            f"_PC_IDS_FILE = {repr(ids_file)}\n"
            f"import sys; sys.path.insert(0, {repr(repo_root)})\n"
            f"\n"
            f"import worker.nodes.base as _ncb\n"
            f"\n"
            f"_original_init = _ncb.NodeContext.__init__\n"
            f"\n"
            f"\n"
            f"def _patched_init(self, *args, **kwargs):\n"
            f"    # Capture the id() of the pipeline_cache argument.\n"
            f"    # This proves the same PipelineCache singleton is used\n"
            f"    # for every NodeContext construction in this worker process.\n"
            f"    pc = kwargs.get('pipeline_cache')\n"
            f"    if pc is not None:\n"
            f"        with open(_PC_IDS_FILE, \"a\") as _f:\n"
            f"            _f.write(str(id(pc)) + \"\\n\")\n"
            f"    return _original_init(self, *args, **kwargs)\n"
            f"\n"
            f"\n"
            f"_ncb.NodeContext.__init__ = _patched_init\n"
        )

    worker_env = _make_worker_env({
        "ANVILML_IPC_PORT": str(port),
        "PYTHONPATH": tmpdir,
    })

    try:
        proc = subprocess.Popen(
            [sys.executable, "-m", "worker.worker_main"],
            env=worker_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            # Wait for the worker to start and emit Ready.
            time.sleep(0.3)

            # Drain the Ready event that the worker sends on startup.
            _recv_with_timeout(router, proc)  # Ready — discard

            # Send first Execute message with an empty graph.
            # An empty graph runs zero nodes, so no real model loading
            # occurs — we only care about NodeContext construction.
            # The graph must be a dict with a "nodes" key (per run_graph
            # contract), not a raw list.
            execute_msg1 = msgpack.packb({
                "_type": "Execute",
                "job_id": "job-cache-1",
                "graph": {"nodes": []},
                "settings": {},
            }, use_bin_type=True)
            router.send_multipart([b"worker-0", execute_msg1])

            # Receive the Completed event for job 1.
            completed1 = _recv_with_timeout(router, proc)
            assert completed1["_type"] == "Completed"
            assert completed1["job_id"] == "job-cache-1"

            # Send second Execute message.
            execute_msg2 = msgpack.packb({
                "_type": "Execute",
                "job_id": "job-cache-2",
                "graph": {"nodes": []},
                "settings": {},
            }, use_bin_type=True)
            router.send_multipart([b"worker-0", execute_msg2])

            # Receive the Completed event for job 2.
            completed2 = _recv_with_timeout(router, proc)
            assert completed2["_type"] == "Completed"
            assert completed2["job_id"] == "job-cache-2"

            # Read the captured id() values from the temp file.
            time.sleep(0.1)  # ensure worker has written both entries
            with open(ids_file, "r") as f:
                lines = [l.strip() for l in f.readlines() if l.strip()]

            assert len(lines) == 2, (
                f"Expected 2 id() entries, got {len(lines)}: {lines}"
            )

            # Both ids should be identical — same PipelineCache singleton.
            id1 = int(lines[0])
            id2 = int(lines[1])
            assert id1 == id2, (
                f"PipelineCache id() differs across jobs: "
                f"job1={id1}, job2={id2}"
            )
        finally:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        router.close(linger=0)
        # Clean up temp files and directory
        import shutil
        shutil.rmtree(tmpdir, ignore_errors=True)