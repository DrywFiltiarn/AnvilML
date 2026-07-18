"""Tests for worker.nodes.loader — LoadModel node class and registration."""

import subprocess
import sys
import threading
import pytest

from worker.nodes.base import NodeContext


def _make_ctx(
    mock: bool = True,
    pipeline_cache: object | None = None,
    device: str = "cpu",
) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.
        pipeline_cache: Optional pipeline cache to use. Defaults to
            an empty dict for backward compatibility with existing tests.
        device: The torch device string. Defaults to "cpu" for backward
            compatibility with existing tests; regression tests for
            device propagation (P901 retrofit) override this explicitly.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    return NodeContext(
        job_id="test-job",
        device=device,
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


def test_load_model_passes_ctx_device_to_arch_load() -> None:
    """LoadModel.execute() forwards ctx.device to module.load() (P901 retrofit).

    Prior to this retrofit, LoadModel called ``module.load(model_id, caps)``
    with no device argument, silently relying on every arch module's
    ``device: str = "cpu"`` default regardless of the worker's actual
    assigned device. Patches ``worker.nodes.arch.diffusion.get_module`` to
    return a stub module whose ``load()`` records its call arguments, sets
    ``ctx.device`` to a non-default value, and asserts that exact value was
    passed through as the third positional/matching argument — a case a
    device="cpu"-only assertion on the loaded result can never catch, since
    the default and an explicit "cpu" are indistinguishable that way.

    Expected outcome: the captured call to ``module.load()`` includes
    ``device="cuda:0"`` (matching ``ctx.device``), not the "cpu" default.
    """
    from unittest.mock import patch, MagicMock

    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    captured_calls: list[tuple] = []

    stub_module = MagicMock()

    def _fake_load(model_id, caps, device):
        captured_calls.append((model_id, caps, device))
        return {"stub": True}

    stub_module.load.side_effect = _fake_load

    node = LoadModel()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache(), device="cuda:0")

    with patch(
        "worker.nodes.arch.diffusion.get_module", return_value=stub_module
    ):
        node.execute(ctx, model_id="some-model-id")

    assert len(captured_calls) == 1, f"expected 1 call to load(), got {len(captured_calls)}"
    _, _, device_arg = captured_calls[0]
    assert device_arg == "cuda:0", (
        f"expected ctx.device ('cuda:0') to be forwarded to module.load(), "
        f"got device={device_arg!r}"
    )



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


def test_load_vae_passes_ctx_device_to_arch_load() -> None:
    """LoadVae.execute() forwards ctx.device to module.load() (P901 retrofit).

    See test_load_model_passes_ctx_device_to_arch_load's docstring for the
    full rationale; this is the LoadVae counterpart.

    Expected outcome: the captured call to ``module.load()`` includes
    ``device="cuda:0"`` (matching ``ctx.device``), not the "cpu" default.
    """
    from unittest.mock import patch, MagicMock

    from worker.nodes.loader import LoadVae
    from worker.pipeline_cache import PipelineCache

    captured_calls: list[tuple] = []

    stub_module = MagicMock()

    def _fake_load(model_id, caps, device):
        captured_calls.append((model_id, caps, device))
        return {"stub": True}

    stub_module.load.side_effect = _fake_load

    node = LoadVae()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache(), device="cuda:0")

    with patch("worker.nodes.arch.vae.get_module", return_value=stub_module):
        node.execute(ctx, model_id="some-vae-id")

    assert len(captured_calls) == 1, f"expected 1 call to load(), got {len(captured_calls)}"
    _, _, device_arg = captured_calls[0]
    assert device_arg == "cuda:0", (
        f"expected ctx.device ('cuda:0') to be forwarded to module.load(), "
        f"got device={device_arg!r}"
    )


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


def test_load_clip_passes_ctx_device_to_arch_load() -> None:
    """LoadClip.execute() forwards ctx.device to module.load() (P901 retrofit).

    See test_load_model_passes_ctx_device_to_arch_load's docstring for the
    full rationale; this is the LoadClip counterpart.

    Expected outcome: the captured call to ``module.load()`` includes
    ``device="cuda:0"`` (matching ``ctx.device``), not the "cpu" default.
    """
    from unittest.mock import patch, MagicMock

    from worker.nodes.loader import LoadClip
    from worker.pipeline_cache import PipelineCache

    captured_calls: list[tuple] = []

    stub_module = MagicMock()

    def _fake_load(model_id, caps, device):
        captured_calls.append((model_id, caps, device))
        return {"stub": True}

    stub_module.load.side_effect = _fake_load

    node = LoadClip()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache(), device="cuda:0")

    with patch("worker.nodes.arch.clip.get_module", return_value=stub_module):
        node.execute(ctx, model_id="some-clip-id", clip_type="qwen3")

    assert len(captured_calls) == 1, f"expected 1 call to load(), got {len(captured_calls)}"
    _, _, device_arg = captured_calls[0]
    assert device_arg == "cuda:0", (
        f"expected ctx.device ('cuda:0') to be forwarded to module.load(), "
        f"got device={device_arg!r}"
    )


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


# ---------------------------------------------------------------------------
# EmptyLatent tests
# ---------------------------------------------------------------------------


def test_empty_latent_mock_returns_placeholder_shape() -> None:
    """Mock-mode EmptyLatent.execute() returns a {"mock": True, "shape": ...} sentinel.

    Constructs a NodeContext with mock=True, calls execute() with
    width=64 and height=64, and asserts the return dict has a "latent"
    key containing the sentinel dict {"mock": True, "shape": (1, 4, 8, 8)}
    — not a real torch.Tensor. A prior version of this test asserted
    isinstance(latent, torch.Tensor) and required `import torch`, which
    is precisely what caused the mock branch under test to import torch
    and crash CI's torch-free mock job (ModuleNotFoundError: No module
    named 'torch', on both Linux and Windows) — see ANVILML_DESIGN.md
    §11.2/§17.2 and every other node's mock branch (LoadModel, LoadVae,
    LoadClip, Sampler, VaeDecode, ClipTextEncode), none of which import
    torch or construct real tensors in their mock branch.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: result["latent"] == {"mock": True, "shape": (1, 4, 8, 8)}.
    """
    from worker.nodes.loader import EmptyLatent

    node = EmptyLatent()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, width=64, height=64)

    assert result == {"latent": {"mock": True, "shape": (1, 4, 8, 8)}}


def test_empty_latent_mock_ignores_model_input() -> None:
    """Mock-mode EmptyLatent.execute() ignores the optional model input.

    Constructs a NodeContext with mock=True, calls execute() with
    width=128, height=128, and a model input. Asserts the result has
    a "latent" key with the sentinel {"mock": True, "shape": (1, 4, 16, 16)},
    identical to calling without the model input — verifying that mock
    mode ignores the model input per §10.3.

    This test exercises the mock code path and confirms the "ignores
    model" contract.

    Expected outcome: result["latent"] == {"mock": True, "shape": (1, 4, 16, 16)},
    and the model input has no effect on the output.
    """
    from worker.nodes.loader import EmptyLatent

    node = EmptyLatent()
    ctx = _make_ctx(mock=True)

    # Call with a model input — mock mode should ignore it.
    result = node.execute(
        ctx,
        width=128,
        height=128,
        model={"mock": True, "model_id": "ignored"},
    )

    assert result == {"latent": {"mock": True, "shape": (1, 4, 16, 16)}}


def test_empty_latent_in_registry() -> None:
    """EmptyLatent appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.loader in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["EmptyLatent"]
    exists and equals the imported class. This proves auto-import and
    registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_passthrough.py::
    test_node_in_registry_after_import.

    Expected outcome: NODE_REGISTRY contains "EmptyLatent" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.loader'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'EmptyLatent' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['EmptyLatent'] is mod.EmptyLatent; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


@pytest.mark.real_mode
def test_empty_latent_real_raises_not_implemented() -> None:
    """EmptyLatent.execute() raises NotImplementedError in real mode.

    Constructs a NodeContext with mock=False, calls execute() with
    width=64 and height=64, and asserts that NotImplementedError is
    raised with a message mentioning "P24-C2".

    This test exercises the real code path (the stub) and satisfies
    the REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError is raised with "P24-C2"
    in the error message.
    """
    from worker.nodes.loader import EmptyLatent

    node = EmptyLatent()
    ctx = _make_ctx(mock=False)

    with pytest.raises(NotImplementedError, match="P24-C2"):
        node.execute(ctx, width=64, height=64)


