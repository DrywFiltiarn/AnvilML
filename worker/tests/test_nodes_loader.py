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
def test_load_model_real_loads_zit_fixture() -> None:
    """LoadModel.execute() loads the ZiT fixture checkpoint via the real branch.

    Calls LoadModel.execute() with mock=False against the P20-A1 fixture
    path (zit_tiny.safetensors). Verifies the return dict has a "model"
    key containing a ZiTModel (torch.nn.Module with .arch == "zit"),
    confirming the full real loading chain works end-to-end.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"model": ZiTModel(...)} is returned, not an exception.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "zit_tiny.safetensors"
    )

    node = LoadModel()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(ctx, model_id=fixture_path)

    # Verify the return dict has the expected MODEL slot.
    assert "model" in result
    model = result["model"]

    # Verify the returned object is a torch.nn.Module (loaded model).
    assert isinstance(model, torch.nn.Module)

    # Verify the architecture identifier is set correctly.
    assert model.arch == "zit"

    # Verify parameters are on the real device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"


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
def test_load_vae_real_loads_zit_vae_fixture() -> None:
    """LoadVae.execute() loads the ZiT VAE fixture checkpoint via the real branch.

    Calls LoadVae.execute() with mock=False against the P23-A1 fixture
    path (zit_vae_tiny.safetensors). Verifies the return dict has a 'vae'
    key containing a ZiTVaeModel (torch.nn.Module with .arch == 'zit_vae'),
    confirming the full real loading chain works end-to-end.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"vae": ZiTVaeModel(...)} is returned, not an exception.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "zit_vae_tiny.safetensors"
    )

    node = LoadVae()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(ctx, model_id=fixture_path)

    # Verify the return dict has the expected VAE slot.
    assert "vae" in result
    vae = result["vae"]

    # Verify the returned object is a torch.nn.Module (loaded VAE).
    assert isinstance(vae, torch.nn.Module)

    # Verify the architecture identifier is set correctly.
    assert vae.arch == "zit_vae"

    # Verify parameters are on the real device (not meta).
    for param in vae.parameters():
        assert param.device.type == "cpu"


@pytest.mark.real_mode
def test_load_vae_real_cache_returns_cached_instance() -> None:
    """LoadVae.execute() returns the cached VAE on a second call with the same model_id.

    Calls LoadVae.execute() twice with mock=False and the same fixture path.
    Verifies that both calls return the same object (the PipelineCache LRU
    cache returned the cached value on the second call rather than reloading).

    This test exercises the real code path and confirms the cache integration
    works correctly for VAE loading.

    Expected outcome: both execute() calls return the identical VAE object.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "zit_vae_tiny.safetensors"
    )

    node = LoadVae()
    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)

    result1 = node.execute(ctx, model_id=fixture_path)
    vae1 = result1["vae"]

    result2 = node.execute(ctx, model_id=fixture_path)
    vae2 = result2["vae"]

    # Both calls must return the same cached object.
    assert vae1 is vae2

    # Verify the cached object is still a valid torch.nn.Module.
    assert isinstance(vae1, torch.nn.Module)
    assert vae1.arch == "zit_vae"


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
def test_load_clip_real_loads_qwen3_fixture() -> None:
    """LoadClip.execute() loads the Qwen3 fixture checkpoint via the real branch.

    Calls LoadClip.execute() with mock=False against the P22 fixture
    path (qwen3_tiny.safetensors). Verifies the return dict has a "clip"
    key containing a Qwen3TextEncoder (torch.nn.Module with .arch ==
    "qwen3" and an attached .tokenizer), confirming the full real
    loading chain works end-to-end.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"clip": Qwen3TextEncoder(...)} is returned, not
    an exception.
    """
    from pathlib import Path

    import torch

    from worker.nodes.loader import LoadClip
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "qwen3_tiny.safetensors"
    )

    node = LoadClip()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(ctx, model_id=fixture_path, clip_type="qwen3")

    # Verify the return dict has the expected CLIP slot.
    assert "clip" in result
    clip = result["clip"]

    # Verify the returned object is a torch.nn.Module (loaded model).
    assert isinstance(clip, torch.nn.Module)

    # Verify the architecture identifier is set correctly.
    assert clip.arch == "qwen3"

    # Verify parameters are on CPU device (not meta).
    for param in clip.parameters():
        assert param.device.type == "cpu"

    # Verify the tokenizer was attached (step 4 of the loading contract).
    assert hasattr(clip, "tokenizer")


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



