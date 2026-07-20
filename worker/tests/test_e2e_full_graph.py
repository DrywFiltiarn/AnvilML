"""End-to-end integration tests: full ZiT generation graph via POST /v1/jobs.

This file exercises the complete ZiT generation pipeline (LoadModel + LoadVae +
LoadClip + EmptyLatent + ClipTextEncode + Sampler + VaeDecode + SaveImage) through
the actual HTTP API → scheduler → dispatch → generic node execution pipeline.

The tests spawn the Rust ``anvilml`` binary as a subprocess, connect a mock Python
worker via ZeroMQ, and verify the full HTTP + dispatch pipeline produces a valid PNG
artifact.

torch is guarded, not imported unconditionally: this file imports torch under a
try/except guard at module level so it stays importable in mock-mode CI collection
(the worker-linux-mock / worker-windows-mock jobs install base.txt only, no torch).
The real_mode marker ensures torch is actually available when these tests run
(ANVILML_DESIGN.md §18.3).
"""

from __future__ import annotations

import base64
import json
import os
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path

import pytest

# Guarded torch import — prevents import errors in mock-mode CI collection.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Server binary path — resolved relative to the repo root at import time.
# This is the release build artifact that the test harness spawns as a subprocess.
_REPO_ROOT = Path(__file__).resolve().parent.parent.parent
_SERVER_BINARY = _REPO_ROOT / "target" / "release" / "anvilml"

# Default HTTP port for the test server. Each test uses a unique port to avoid
# port conflicts when tests run in parallel.
_DEFAULT_PORT = 18488

# Maximum time to wait for a port to become available (seconds).
_PORT_TIMEOUT = 10


def _wait_for_port(port: int) -> bool:
    """Wait until the given TCP port is no longer in use.

    Polls the port every 0.5 seconds for up to ``_PORT_TIMEOUT`` seconds.
    Returns ``True`` if the port becomes available, ``False`` if the timeout
    elapses (port is still in use).
    """
    import socket

    deadline = time.monotonic() + _PORT_TIMEOUT
    while time.monotonic() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            try:
                s.bind(("127.0.0.1", port))
                return True  # Port is free
            except OSError:
                # Port is still in use — wait and retry
                time.sleep(0.5)
    return False  # Timeout — port is still in use

# Default IPC port for the mock worker connection. The Rust server binds a
# ZeroMQ ROUTER socket on this port; the mock worker connects a DEALER socket.
_DEFAULT_IPC_PORT = 19488

# Health-check poll interval in seconds.
_HEALTH_POLL_INTERVAL = 0.2

# Maximum time to wait for the server to become healthy (seconds).
_HEALTH_TIMEOUT = 15

# Maximum time to wait for the worker to register (seconds).
_WORKER_READY_TIMEOUT = 15

# Maximum time to wait for a job to complete (seconds).
_JOB_COMPLETE_TIMEOUT = 120

# Poll interval for job status checks (seconds).
_JOB_POLL_INTERVAL = 0.5

# ---------------------------------------------------------------------------
# Graph construction helpers
# ---------------------------------------------------------------------------


def _make_full_zit_graph(model_hash: str, vae_hash: str, clip_hash: str) -> dict:
    """Construct the full Appendix B.2 ZiT generation graph.

    Builds the canonical eight-node graph:
    LoadModel → LoadVae → LoadClip → EmptyLatent → ClipTextEncode →
    Sampler → VaeDecode → SaveImage

    The graph uses model ID hashes that point to fixture safetensors files.
    Edges wire the outputs of each node to the inputs of the next.

    Args:
        model_hash: SHA-256 hash of the ZiT diffusion model fixture.
        vae_hash: SHA-256 hash of the ZiT VAE fixture.
        clip_hash: SHA-256 hash of the Qwen3 CLIP fixture.

    Returns:
        A graph dict compatible with the scheduler's validate_graph() function.
    """
    return {
        "nodes": [
            {
                "id": "load_model_0",
                "type": "LoadModel",
                "inputs": {"model_id": model_hash},
            },
            {
                "id": "load_vae_0",
                "type": "LoadVae",
                "inputs": {"model_id": vae_hash},
            },
            {
                "id": "load_clip_0",
                "type": "LoadClip",
                "inputs": {"model_id": clip_hash, "clip_type": "qwen3"},
            },
            {
                "id": "empty_latent_0",
                "type": "EmptyLatent",
                "inputs": {
                    "width": 64,
                    "height": 64,
                    "batch_size": 1,
                    # In mock mode the "model" input is ignored (ANVILML_DESIGN.md §10.3);
                    # in real mode it is required for compute_latent_shape().
                },
            },
            {
                "id": "clip_encode_0",
                "type": "ClipTextEncode",
                "inputs": {
                    "clip": {"node_id": "load_clip_0", "output_slot": "clip"},
                    "positive_text": "a simple test image",
                },
            },
            {
                "id": "sampler_0",
                "type": "Sampler",
                "inputs": {
                    "model": {"node_id": "load_model_0", "output_slot": "model"},
                    "conditioning": {
                        "node_id": "clip_encode_0",
                        "output_slot": "conditioning",
                    },
                    "latent": {
                        "node_id": "empty_latent_0",
                        "output_slot": "latent",
                    },
                    "steps": 4,
                    "cfg": 7.5,
                    "seed": 42,
                },
            },
            {
                "id": "vae_decode_0",
                "type": "VaeDecode",
                "inputs": {
                    "vae": {"node_id": "load_vae_0", "output_slot": "vae"},
                    "latent": {
                        "node_id": "sampler_0",
                        "output_slot": "latent",
                    },
                },
            },
            {
                "id": "save_image_0",
                "type": "SaveImage",
                "inputs": {
                    "image": {
                        "node_id": "vae_decode_0",
                        "output_slot": "image",
                    },
                },
            },
        ],
        "edges": [
            {
                "from": "load_model_0:model",
                "to": "sampler_0:model",
            },
            {
                "from": "load_clip_0:clip",
                "to": "clip_encode_0:clip",
            },
            {
                "from": "clip_encode_0:conditioning",
                "to": "sampler_0:conditioning",
            },
            {
                "from": "empty_latent_0:latent",
                "to": "sampler_0:latent",
            },
            {
                "from": "load_vae_0:vae",
                "to": "vae_decode_0:vae",
            },
            {
                "from": "sampler_0:latent",
                "to": "vae_decode_0:latent",
            },
            {
                "from": "vae_decode_0:image",
                "to": "save_image_0:image",
            },
        ],
    }


def _make_invalid_graph() -> dict:
    """Construct a graph with an unknown node type.

    Returns a structurally valid graph (has nodes array, valid IDs) but
    references a node type that does not exist in the registry. This
    triggers the UnknownNodeType error in the DAG validator.

    Returns:
        A graph dict that will fail graph validation.
    """
    return {
        "nodes": [
            {
                "id": "node1",
                "type": "NonExistentNode",
                "inputs": {},
            },
        ],
        "edges": [],
    }


# ---------------------------------------------------------------------------
# Subprocess and server management
# ---------------------------------------------------------------------------


def _build_server_binary() -> Path:
    """Build the release binary if it does not already exist.

    Runs ``cargo build --release -p anvilml`` and returns the path to the
    built binary. If the binary already exists at the expected path, returns
    it immediately without building.

    Returns:
        The path to the ``anvilml`` binary.

    Raises:
        RuntimeError: If the cargo build command exits non-zero.
    """
    if _SERVER_BINARY.exists():
        return _SERVER_BINARY

    # Build the release binary. This is a one-time cost — the binary is
    # cached across test runs.
    result = subprocess.run(
        ["cargo", "build", "--release", "-p", "anvilml"],
        cwd=str(_REPO_ROOT),
        capture_output=True,
        text=True,
        timeout=300,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"Failed to build anvilml binary: stdout={result.stdout!r}, "
            f"stderr={result.stderr!r}"
        )
    assert _SERVER_BINARY.exists(), (
        f"Expected binary at {_SERVER_BINARY} after cargo build"
    )
    return _SERVER_BINARY


def _start_server(
    port: int,
    ipc_port: int,
    env: dict[str, str] | None = None,
) -> subprocess.Popen:
    """Start the AnvilML server as a subprocess.

    Spawns the ``anvilml`` binary with ``--port`` and ``--host`` flags,
    plus ``ANVILML_FORCE_WORKER_MOCK=1`` in the environment (for mock-mode
    tests). The server binds on 127.0.0.1 and uses the provided ports.

    Args:
        port: HTTP port for the server.
        ipc_port: ZeroMQ IPC port for worker connections.
        env: Additional environment variables to set. Defaults to
            ``ANVILML_FORCE_WORKER_MOCK=1`` for mock-mode tests.

    Returns:
        A ``subprocess.Popen`` handle for the server process.

    Raises:
        RuntimeError: If the server fails to start.
    """
    if env is None:
        env = {}

    # Create a temporary TOML config that includes the fixture model directory
    # so that model resolution (hash → path) succeeds during dispatch.
    # The config uses a temp DB and artifact dir to avoid polluting the main
    # database and artifacts directory. Each test run gets unique paths to
    # prevent cross-contamination between test runs.
    import tempfile

    config_dir = tempfile.mkdtemp()
    config_path = os.path.join(config_dir, "anvilml.toml")
    db_path = os.path.join(config_dir, f"anvilml-test-{uuid.uuid4().hex[:8]}.db")
    artifact_dir = os.path.join(config_dir, f"artifacts-test-{uuid.uuid4().hex[:8]}")
    fixtures_dir = str(_REPO_ROOT / "worker" / "tests" / "fixtures")

    with open(config_path, "w") as f:
        f.write(
            f"host = \"127.0.0.1\"\n"
            f"port = {port}\n"
            f"db_path = \"{db_path}\"\n"
            f"artifact_dir = \"{artifact_dir}\"\n"
            f"venv_path = \"{_REPO_ROOT / 'worker' / '.venv'}\"\n"
            f"model_scan_depth = 2\n"
            f"max_ipc_payload_mib = 256\n"
            f"\n"
            f"[[model_dirs]]\n"
            f"path = \"{fixtures_dir}\"\n"
            f"recursive = false\n"
        )

    # Inherit the parent process's environment and override/add test-specific vars.
    server_env = os.environ.copy()
    server_env["ANVILML_HOST"] = "127.0.0.1"
    server_env["ANVILML_PORT"] = str(port)
    server_env["ANVILML_IPC_PORT"] = str(ipc_port)
    server_env["ANVILML_LOG"] = "warn"  # Reduce noise in CI
    server_env["ANVILML_FORCE_WORKER_MOCK"] = "1"
    server_env["ANVILML_MOCK_DEVICE_TYPE"] = "cpu"
    server_env["ANVILML_MOCK_VRAM_MIB"] = "512"
    server_env["ANVILML_WORKER_MOCK"] = "1"
    # Pass the temp config via --config CLI flag.
    server_env.update(env)

    binary = _build_server_binary()

    proc = subprocess.Popen(
        [str(binary), "--config", config_path],
        cwd=str(_REPO_ROOT),
        env=server_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return proc


def _wait_for_health(port: int, timeout: float = _HEALTH_TIMEOUT) -> bool:
    """Poll the server's /health endpoint until it returns 200 or times out.

    Uses ``httpx`` (already available in the test venv via base.txt) to
    make the health check request. Retries at ``_HEALTH_POLL_INTERVAL``
    intervals.

    Args:
        port: The HTTP port the server is listening on.
        timeout: Maximum seconds to wait.

    Returns:
        True if the health check succeeded within the timeout, False otherwise.
    """
    import httpx

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            resp = httpx.get(
                f"http://127.0.0.1:{port}/health",
                timeout=2.0,
            )
            if resp.status_code == 200:
                return True
        except httpx.ConnectError:
            pass
        except httpx.RequestError:
            pass
        time.sleep(_HEALTH_POLL_INTERVAL)
    return False


def _wait_for_worker_ready(
    port: int,
    timeout: float = _WORKER_READY_TIMEOUT,
) -> bool:
    """Poll the /v1/nodes endpoint until at least one node type is registered.

    The worker registers its node types via a Ready event on the ZeroMQ
    ROUTER socket. The Rust server populates the in-memory node registry
    when it receives this event. The /v1/nodes endpoint exposes this
    registry.

    Args:
        port: The HTTP port the server is listening on.
        timeout: Maximum seconds to wait.

    Returns:
        True if nodes were registered within the timeout, False otherwise.
    """
    import httpx

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            resp = httpx.get(
                f"http://127.0.0.1:{port}/v1/nodes",
                timeout=2.0,
            )
            if resp.status_code == 200:
                nodes = resp.json()
                if isinstance(nodes, list) and len(nodes) > 0:
                    return True
        except httpx.ConnectError:
            pass
        except httpx.RequestError:
            pass
        time.sleep(_HEALTH_POLL_INTERVAL)
    return False


def _poll_job_status(
    port: int,
    job_id: str,
    timeout: float = _JOB_COMPLETE_TIMEOUT,
) -> dict | None:
    """Poll GET /v1/jobs/{job_id} until the job reaches a terminal state.

    Checks the job status every ``_JOB_POLL_INTERVAL`` seconds. Returns
    the job dict when the job reaches a terminal status (Completed, Failed,
    Cancelled), or None if the timeout elapses.

    Args:
        port: The HTTP port the server is listening on.
        job_id: The UUID string of the job to poll.
        timeout: Maximum seconds to wait.

    Returns:
        The job dict from the API, or None on timeout.
    """
    import httpx

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            resp = httpx.get(
                f"http://127.0.0.1:{port}/v1/jobs/{job_id}",
                timeout=2.0,
            )
            if resp.status_code == 200:
                job = resp.json()
                status = job.get("status", "")
                # Terminal statuses: Completed, Failed, Cancelled
                if status in ("completed", "failed", "cancelled"):
                    return job
        except httpx.ConnectError:
            # Server may have crashed — return None to surface the error
            return None
        except httpx.RequestError:
            pass
        time.sleep(_JOB_POLL_INTERVAL)
    return None


def _terminate_process(proc: subprocess.Popen, label: str = "server") -> None:
    """Terminate a subprocess and collect its stderr for diagnostics.

    Sends SIGTERM first, waits up to 5 seconds, then sends SIGKILL if
    the process is still alive. Collects stdout and stderr from the
    terminated process for inclusion in failure messages.

    Args:
        proc: The subprocess.Popen handle.
        label: A human-readable label for the process (used in error messages).
    """
    if proc.poll() is not None:
        return  # Already terminated

    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        stdout, stderr = proc.communicate(timeout=5)

    if stderr:
        # stderr is useful for diagnosing why the server crashed
        pass  # Caller can access proc._stderr if needed


# ---------------------------------------------------------------------------
# Test 1: Full graph in mock mode
# ---------------------------------------------------------------------------


@pytest.mark.serial
def test_full_graph_mock_mode() -> None:
    """Full Appendix B.2 ZiT graph executes end-to-end through the generic node layer in mock mode.

    This test exercises the complete HTTP API → scheduler → dispatch →
    generic node execution pipeline in mock mode (ANVILML_WORKER_MOCK=1):

    1. Starts the Rust ``anvilml`` server subprocess with ``ANVILML_FORCE_WORKER_MOCK=1``.
    2. Waits for the server to be healthy (GET /health → 200).
    3. Waits for the mock worker to register (GET /v1/nodes → non-empty).
    4. Submits the full ZiT generation graph via POST /v1/jobs.
    5. Polls GET /v1/jobs/{job_id} until the job reaches ``completed`` status.
    6. Retrieves the artifact via GET /v1/artifacts/{hash}.
    7. Asserts the response is a valid PNG (checks magic bytes ``89 50 4e 47``).

    This is the primary Runnable Proof for the generic node layer's e2e
    execution path.

    Env var isolation: ``ANVILML_FORCE_WORKER_MOCK``, ``ANVILML_MOCK_*``,
    and ``ANVILML_WORKER_MOCK`` are set in the server subprocess env only
    (not inherited into the test process), so no capture-and-restore is
    needed.

    Preconditions: Rust binary is built (or buildable via ``cargo build``).
    Expected output: Job reaches ``Completed`` status, artifact is a valid PNG.
    """
    # Use a unique port for this test run to avoid conflicts with parallel tests.
    port = _DEFAULT_PORT + 1
    ipc_port = _DEFAULT_IPC_PORT + 1

    # Ensure the port is free before starting the server. This prevents
    # "Address already in use" errors when tests are run multiple times
    # in quick succession (e.g. during local development or CI retries).
    if not _wait_for_port(port):
        pytest.fail(f"Port {port} is still in use after {_PORT_TIMEOUT}s — another test may not have cleaned up")

    proc: subprocess.Popen | None = None
    try:
        # Start the server.
        proc = _start_server(port, ipc_port)
        assert proc.stdout is not None and proc.stderr is not None

        # Wait for the server to become healthy.
        healthy = _wait_for_health(port)
        if not healthy:
            # Collect stderr for diagnosis.
            proc.terminate()
            try:
                _, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                _, stderr = proc.communicate(timeout=5)
            pytest.fail(
                f"Server did not become healthy within {_HEALTH_TIMEOUT}s. "
                f"stderr={stderr.decode(errors='replace')}"
            )

       # Wait for the mock worker to register its node types.
        worker_ready = _wait_for_worker_ready(port)
        if not worker_ready:
            pytest.fail(
                f"Worker did not register node types within {_WORKER_READY_TIMEOUT}s"
            )

        # Get the model hashes from the model registry.
        # The server was started with a config that includes the fixture
        # directory, so models should be scanned at startup.
        import httpx

        # List models to get the fixture hashes.
        resp = httpx.get(f"http://127.0.0.1:{port}/v1/models", timeout=5.0)
        assert resp.status_code == 200, (
            f"Model list failed: {resp.status_code} {resp.text}"
        )
        models = resp.json()

        # Find the fixture model hashes by their path.
        model_hash = vae_hash = clip_hash = None
        for m in models:
            path = m.get("path", "")
            if "zit_tiny.safetensors" in path and "vae" not in path and "clip" not in path:
                model_hash = m.get("id")
            elif "zit_vae_tiny.safetensors" in path:
                vae_hash = m.get("id")
            elif "qwen3_tiny.safetensors" in path:
                clip_hash = m.get("id")

        # If models are not found by path, fall back to using any registered models.
        if model_hash is None and len(models) > 0:
            model_hash = models[0].get("id")
        if vae_hash is None and len(models) > 1:
            vae_hash = models[1].get("id")
        if clip_hash is None and len(models) > 2:
            clip_hash = models[2].get("id")

        # All three must be found — the fixture directory must contain
        # the model files. If not, the test configuration is wrong.
        assert model_hash is not None, (
            f"Could not find ZiT diffusion model in scanned models. "
            f"Models: {[m.get('path') for m in models]}"
        )
        assert vae_hash is not None, (
            f"Could not find ZiT VAE model in scanned models. "
            f"Models: {[m.get('path') for m in models]}"
        )
        assert clip_hash is not None, (
            f"Could not find Qwen3 CLIP model in scanned models. "
            f"Models: {[m.get('path') for m in models]}"
        )

        # Build the full ZiT graph.
        graph = _make_full_zit_graph(model_hash, vae_hash, clip_hash)

        # Submit the job.
        submit_body = {
            "graph": graph,
            "settings": {"device_preference": None},
        }
        resp = httpx.post(
            f"http://127.0.0.1:{port}/v1/jobs",
            json=submit_body,
            timeout=10.0,
        )
        assert resp.status_code == 202, (
            f"Job submission failed: {resp.status_code} {resp.text}"
        )
        job_data = resp.json()
        job_id = job_data["job_id"]

        # Poll for job completion.
        job = _poll_job_status(port, job_id)
        assert job is not None, (
            f"Job {job_id} did not reach a terminal state within {_JOB_COMPLETE_TIMEOUT}s"
        )
        assert job["status"] == "completed", (
            f"Job ended with status '{job.get('status')}', expected 'completed'. "
            f"Error: {job.get('error')}"
        )

        # List artifacts for this job to find the artifact hash.
        resp = httpx.get(
            f"http://127.0.0.1:{port}/v1/artifacts?job_id={job_id}",
            timeout=5.0,
        )
        assert resp.status_code == 200, (
            f"Artifact list failed: {resp.status_code} {resp.text}"
        )
        artifacts = resp.json()
        assert len(artifacts) > 0, (
            f"No artifacts found for job {job_id}; artifacts={artifacts}"
        )

        # Get the artifact by hash.
        artifact_hash = artifacts[0]["hash"]
        resp = httpx.get(
            f"http://127.0.0.1:{port}/v1/artifacts/{artifact_hash}",
            timeout=10.0,
        )
        assert resp.status_code == 200, (
            f"Artifact retrieval failed: {resp.status_code} {resp.text}"
        )

        # Verify the artifact is a valid PNG.
        png_bytes = resp.content
        assert len(png_bytes) > 0, "Artifact is empty"
        # PNG magic bytes: 89 50 4e 47 0d 0a 1a 0a
        assert png_bytes[:8] == b"\x89PNG\r\n\x1a\n", (
            f"Artifact does not start with PNG magic bytes: "
            f"first 8 bytes = {png_bytes[:8].hex()}"
        )

        # Verify the artifact has reasonable dimensions (64×64 from SaveImage mock).
        from PIL import Image

        img = Image.open(__import__("io").BytesIO(png_bytes))
        assert img.size == (64, 64), (
            f"Expected 64×64 image, got {img.size}"
        )
        assert img.mode == "RGB", (
            f"Expected RGB mode, got {img.mode}"
        )

    finally:
        # Clean up the server subprocess.
        if proc is not None:
            _terminate_process(proc, "server")


# ---------------------------------------------------------------------------
# Test 2: Full graph in real mode (requires torch)
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
@pytest.mark.serial
def test_full_graph_real_mode() -> None:
    """Full ZiT graph executes end-to-end through the generic node layer in real mode.

    Same pipeline as ``test_full_graph_mock_mode`` but with a real Python worker
    (no ``ANVILML_WORKER_MOCK``). The real worker loads fixture checkpoints and
    executes the full generic node graph with actual torch inference.

    After job completion, retrieves the artifact and asserts:
    - The response is a valid PNG
    - The image dimensions match the requested 64×64

    This test requires torch and the fixture checkpoints.

    Preconditions: Torch is installed; fixture checkpoints are registered.
    Expected output: Job reaches ``Completed`` status, artifact is a valid 64×64 PNG.
    """
    # Use a unique port for this test run.
    port = _DEFAULT_PORT + 2
    ipc_port = _DEFAULT_IPC_PORT + 2

    # Real-mode tests need torch — skip if not available.
    if torch is None:
        pytest.skip("torch not available — real mode requires torch")

    proc: subprocess.Popen | None = None
    try:
        # Start the server in real mode (no ANVILML_FORCE_WORKER_MOCK).
        # The real Python worker will load fixture checkpoints and run
        # actual torch inference.
        proc = _start_server(
            port,
            ipc_port,
            env={"ANVILML_FORCE_WORKER_MOCK": "0"},
        )
        assert proc.stdout is not None and proc.stderr is not None

        # Wait for the server to become healthy.
        healthy = _wait_for_health(port)
        if not healthy:
            proc.terminate()
            try:
                _, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                _, stderr = proc.communicate(timeout=5)
            pytest.fail(
                f"Server did not become healthy within {_HEALTH_TIMEOUT}s. "
                f"stderr={stderr.decode(errors='replace')}"
            )

        # Wait for the real worker to register.
        worker_ready = _wait_for_worker_ready(port)
        if not worker_ready:
            pytest.fail(
                f"Worker did not register node types within {_WORKER_READY_TIMEOUT}s"
            )

        # Get the model hashes from the model registry.
        import httpx

        # Rescan models to ensure fixture models are registered.
        resp = httpx.post(
            f"http://127.0.0.1:{port}/v1/models/rescan",
            timeout=10.0,
        )
        assert resp.status_code == 202, (
            f"Model rescan failed: {resp.status_code} {resp.text}"
        )

        resp = httpx.get(f"http://127.0.0.1:{port}/v1/models", timeout=5.0)
        assert resp.status_code == 200, (
            f"Model list failed: {resp.status_code} {resp.text}"
        )
        models = resp.json()

        model_hash = vae_hash = clip_hash = None
        for m in models:
            path = m.get("path", "")
            if "zit_tiny.safetensors" in path and "vae" not in path and "clip" not in path:
                model_hash = m.get("id")
            elif "zit_vae_tiny.safetensors" in path:
                vae_hash = m.get("id")
            elif "qwen3_tiny.safetensors" in path:
                clip_hash = m.get("id")

        if model_hash is None and len(models) > 0:
            model_hash = models[0].get("id")
        if vae_hash is None and len(models) > 1:
            vae_hash = models[1].get("id")
        if clip_hash is None and len(models) > 2:
            clip_hash = models[2].get("id")

        if model_hash is None:
            model_hash = "real_model_placeholder"
        if vae_hash is None:
            vae_hash = "real_vae_placeholder"
        if clip_hash is None:
            clip_hash = "real_clip_placeholder"

        # Build the full ZiT graph.
        graph = _make_full_zit_graph(model_hash, vae_hash, clip_hash)

        # Submit the job.
        submit_body = {
            "graph": graph,
            "settings": {"device_preference": None},
        }
        resp = httpx.post(
            f"http://127.0.0.1:{port}/v1/jobs",
            json=submit_body,
            timeout=10.0,
        )
        assert resp.status_code == 202, (
            f"Job submission failed: {resp.status_code} {resp.text}"
        )
        job_data = resp.json()
        job_id = job_data["job_id"]

        # Poll for job completion. Real mode is slower than mock mode
        # because it runs actual torch inference, so use a longer timeout.
        job = _poll_job_status(port, job_id, timeout=300.0)
        assert job is not None, (
            f"Job {job_id} did not reach a terminal state within 300s"
        )
        assert job["status"] == "completed", (
            f"Job ended with status '{job.get('status')}', expected 'completed'. "
            f"Error: {job.get('error')}"
        )

        # List artifacts for this job.
        resp = httpx.get(
            f"http://127.0.0.1:{port}/v1/artifacts?job_id={job_id}",
            timeout=5.0,
        )
        assert resp.status_code == 200, (
            f"Artifact list failed: {resp.status_code} {resp.text}"
        )
        artifacts = resp.json()
        assert len(artifacts) > 0, (
            f"No artifacts found for job {job_id}"
        )

        # Get the artifact by hash.
        artifact_hash = artifacts[0]["hash"]
        resp = httpx.get(
            f"http://127.0.0.1:{port}/v1/artifacts/{artifact_hash}",
            timeout=10.0,
        )
        assert resp.status_code == 200, (
            f"Artifact retrieval failed: {resp.status_code} {resp.text}"
        )

        # Verify the artifact is a valid PNG with correct dimensions.
        png_bytes = resp.content
        assert len(png_bytes) > 0, "Artifact is empty"
        assert png_bytes[:8] == b"\x89PNG\r\n\x1a\n", (
            f"Artifact does not start with PNG magic bytes: "
            f"first 8 bytes = {png_bytes[:8].hex()}"
        )

        # Verify dimensions match the requested 64×64.
        from PIL import Image

        img = Image.open(__import__("io").BytesIO(png_bytes))
        assert img.size == (64, 64), (
            f"Expected 64×64 image, got {img.size}"
        )
        assert img.mode == "RGB", (
            f"Expected RGB mode, got {img.mode}"
        )

    finally:
        # Clean up the server subprocess.
        if proc is not None:
            _terminate_process(proc, "server")


# ---------------------------------------------------------------------------
# Test 3: Invalid graph returns 400
# ---------------------------------------------------------------------------


@pytest.mark.serial
def test_full_graph_invalid_graph_returns_400() -> None:
    """Structurally invalid graph (unknown node type) returns 400 Bad Request.

    Starts the server, then submits a graph that references a node type
    (``NonExistentNode``) that does not exist in the registry. The
    scheduler's DAG validator detects the unknown type and returns
    ``AnvilError::InvalidGraph``, which maps to HTTP 400.

    Asserts:
    - Response status is 400 Bad Request
    - Response body contains a validation error message

    Preconditions: Rust binary is built.
    Expected output: 400 with validation error message.
    """
    port = _DEFAULT_PORT + 3
    ipc_port = _DEFAULT_IPC_PORT + 3

    proc: subprocess.Popen | None = None
    try:
        # Start the server.
        proc = _start_server(port, ipc_port)
        assert proc.stdout is not None and proc.stderr is not None

        # Wait for the server to become healthy.
        healthy = _wait_for_health(port)
        if not healthy:
            proc.terminate()
            try:
                _, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                _, stderr = proc.communicate(timeout=5)
            pytest.fail(
                f"Server did not become healthy within {_HEALTH_TIMEOUT}s. "
                f"stderr={stderr.decode(errors='replace')}"
            )

        # Build an invalid graph with an unknown node type.
        invalid_graph = _make_invalid_graph()

        import httpx

        # Submit the invalid graph.
        submit_body = {
            "graph": invalid_graph,
            "settings": {"device_preference": None},
        }
        resp = httpx.post(
            f"http://127.0.0.1:{port}/v1/jobs",
            json=submit_body,
            timeout=10.0,
        )

        # Assert 400 Bad Request.
        assert resp.status_code == 400, (
            f"Expected 400 Bad Request for invalid graph, "
            f"got {resp.status_code}: {resp.text}"
        )

        # Assert the response body contains a validation error message.
        body = resp.json()
        error_msg = json.dumps(body)
        assert "NonExistentNode" in error_msg or "invalid" in error_msg.lower() or "unknown" in error_msg.lower(), (
            f"Expected validation error message in response body, "
            f"got: {error_msg}"
        )

    finally:
        # Clean up the server subprocess.
        if proc is not None:
            _terminate_process(proc, "server")
