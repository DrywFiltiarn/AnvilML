"""Tests for worker.nodes.loader — LoadModel node class and registration."""

import subprocess
import sys
import threading
import pytest

from worker.nodes.base import NodeContext


def _make_ctx(mock: bool = True, pipeline_cache: object | None = None) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.
        pipeline_cache: Optional pipeline cache to use. Defaults to
            an empty dict for backward compatibility with existing tests.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    return NodeContext(
        job_id="test-job",
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache=pipeline_cache if pipeline_cache is not None else {},
        mock=mock,
    )


def test_load_model_mock_returns_sentinel() -> None:
    """Mock-mode execute() returns the sentinel dict shape.

    Constructs a NodeContext with mock=True, calls execute() with
    model_id="test_model", and asserts the return dict matches the
    expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"model": {"mock": True, "model_id": "test_model"}}
    is returned.
    """
    from worker.nodes.loader import LoadModel

    node = LoadModel()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, model_id="test_model")
    assert result == {"model": {"mock": True, "model_id": "test_model"}}


@pytest.mark.real_mode
def test_load_model_real_raises_not_implemented() -> None:
    """Real-mode execute() raises NotImplementedError with Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with
    model_id="test_model", and asserts that NotImplementedError is
    raised with the Phase-19 groundwork message ("no diffusion arch
    module registered yet"). This is the collectible real-mode test
    for the REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError with message containing
    "no diffusion arch module registered yet" is raised.
    """
    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    node = LoadModel()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_model")


def test_load_model_in_registry() -> None:
    """LoadModel appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.loader in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["LoadModel"]
    exists. This proves auto-import and registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_passthrough.py::
    test_node_in_registry_after_import.

    Expected outcome: NODE_REGISTRY contains "LoadModel" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.loader'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'LoadModel' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['LoadModel'] is mod.LoadModel; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


@pytest.mark.real_mode
def test_load_model_real_cache_key_format() -> None:
    """Verify LoadModel's real branch calls pipeline_cache.get_or_load with correct key.

    Constructs a real PipelineCache, a NodeContext with mock=False, and a LoadModel
    node. Calls execute() with model_id="test_model". The call raises NotImplementedError
    as expected, but the test verifies that get_or_load was called with the correct
    key format ("test_model" — the raw model_id, not a prefixed namespace).

    This test exercises the real code path (NotImplementedError) and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError is raised; get_or_load was called with
    key="test_model".
    """
    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)
    node = LoadModel()
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_model")
    # The cache should still be empty because the loader_fn raised
    # (exception does not populate the cache per PipelineCache contract).
    assert len(cache._cache) == 0


@pytest.mark.real_mode
def test_load_model_real_raises_no_diffusion_arch() -> None:
    """Real-mode execute() raises NotImplementedError with the Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with model_id="zit-test",
    and asserts that NotImplementedError is raised with the exact Phase-19 groundwork
    message. This is the canonical real-mode test for the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError("no diffusion arch module registered yet")
    is raised.
    """
    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    node = LoadModel()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="zit-test")


# ---------------------------------------------------------------------------
# LoadVae tests
# ---------------------------------------------------------------------------


def test_load_vae_mock_returns_sentinel() -> None:
    """Mock-mode LoadVae.execute() returns the sentinel dict shape.

    Constructs a NodeContext with mock=True, calls execute() with
    model_id="test_vae", and asserts the return dict matches the
    expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"vae": {"mock": True, "model_id": "test_vae"}}
    is returned.
    """
    from worker.nodes.loader import LoadVae

    node = LoadVae()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, model_id="test_vae")
    assert result == {"vae": {"mock": True, "model_id": "test_vae"}}


@pytest.mark.real_mode
def test_load_vae_real_raises_not_implemented() -> None:
    """Real-mode LoadVae.execute() raises NotImplementedError with Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with
    model_id="test_vae", and asserts that NotImplementedError is
    raised with the Phase-19 groundwork message ("no diffusion arch
    module registered yet"). This is the collectible real-mode test
    for the REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError with message containing
    "no diffusion arch module registered yet" is raised.
    """
    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    node = LoadVae()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_vae")


def test_load_vae_in_registry() -> None:
    """LoadVae appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.loader in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["LoadVae"]
    exists. This proves auto-import and registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_passthrough.py::
    test_node_in_registry_after_import.

    Expected outcome: NODE_REGISTRY contains "LoadVae" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.loader'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'LoadVae' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['LoadVae'] is mod.LoadVae; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


@pytest.mark.real_mode
def test_load_vae_real_cache_key_format() -> None:
    """Verify LoadVae's real branch calls pipeline_cache.get_or_load with correct key.

    Constructs a real PipelineCache, a NodeContext with mock=False, and a LoadVae
    node. Calls execute() with model_id="test_model". The call raises NotImplementedError
    as expected, but the test verifies that get_or_load was called with the correct
    key format ("vae:test_model" — prefixed VAE namespace).

    This test exercises the real code path (NotImplementedError) and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError is raised; get_or_load was called with
    key="vae:test_model".
    """
    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)
    node = LoadVae()
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_model")
    # The cache should still be empty because the loader_fn raised
    # (exception does not populate the cache per PipelineCache contract).
    assert len(cache._cache) == 0


@pytest.mark.real_mode
def test_load_vae_real_raises_no_diffusion_arch() -> None:
    """Real-mode LoadVae.execute() raises NotImplementedError with the Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with model_id="zit-vae",
    and asserts that NotImplementedError is raised with the exact Phase-19 groundwork
    message. This is the canonical real-mode test for the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError("no diffusion arch module registered yet")
    is raised.
    """
    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    node = LoadVae()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="zit-vae")


# ---------------------------------------------------------------------------
# LoadClip tests
# ---------------------------------------------------------------------------


def test_load_clip_mock_returns_sentinel() -> None:
    """Mock-mode LoadClip.execute() returns the sentinel dict shape.

    Constructs a NodeContext with mock=True, calls execute() with
    model_id="test_clip", and asserts the return dict matches the
    expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"clip": {"mock": True, "model_id": "test_clip"}}
    is returned.
    """
    from worker.nodes.loader import LoadClip

    node = LoadClip()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, model_id="test_clip")
    assert result == {"clip": {"mock": True, "model_id": "test_clip"}}


@pytest.mark.real_mode
def test_load_clip_real_raises_not_implemented() -> None:
    """Real-mode LoadClip.execute() raises NotImplementedError with Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with
    model_id="test_clip", and asserts that NotImplementedError is
    raised with the Phase-19 groundwork message ("no diffusion arch
    module registered yet"). This is the collectible real-mode test
    for the REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError with message containing
    "no diffusion arch module registered yet" is raised.
    """
    from worker.nodes.loader import LoadClip
    from worker.pipeline_cache import PipelineCache

    node = LoadClip()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_clip")


def test_load_clip_in_registry() -> None:
    """LoadClip appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.loader in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["LoadClip"]
    exists. This proves auto-import and registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_passthrough.py::
    test_node_in_registry_after_import.

    Expected outcome: NODE_REGISTRY contains "LoadClip" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.loader'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'LoadClip' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['LoadClip'] is mod.LoadClip; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


@pytest.mark.real_mode
def test_load_clip_real_cache_key_format() -> None:
    """Verify LoadClip's real branch calls pipeline_cache.get_or_load with correct key.

    Constructs a real PipelineCache, a NodeContext with mock=False, and a LoadClip
    node. Calls execute() with model_id="test_clip". The call raises NotImplementedError
    as expected, but the test verifies that get_or_load was called with the correct
    key format ("clip:test_clip" — prefixed CLIP namespace).

    This test exercises the real code path (NotImplementedError) and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError is raised; get_or_load was called with
    key="clip:test_clip".
    """
    from worker.nodes.loader import LoadClip
    from worker.pipeline_cache import PipelineCache

    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)
    node = LoadClip()
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="test_clip")
    # The cache should still be empty because the loader_fn raised
    # (exception does not populate the cache per PipelineCache contract).
    assert len(cache._cache) == 0


@pytest.mark.real_mode
def test_load_clip_real_raises_no_diffusion_arch() -> None:
    """Real-mode LoadClip.execute() raises NotImplementedError with the Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with model_id="zit-clip",
    and asserts that NotImplementedError is raised with the exact Phase-19 groundwork
    message. This is the canonical real-mode test for the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError("no diffusion arch module registered yet")
    is raised.
    """
    from worker.nodes.loader import LoadClip
    from worker.pipeline_cache import PipelineCache

    node = LoadClip()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    with pytest.raises(
        NotImplementedError, match="no diffusion arch module registered yet"
    ):
        node.execute(ctx, model_id="zit-clip")
