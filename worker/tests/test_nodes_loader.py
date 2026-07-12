"""Tests for worker.nodes.loader — LoadModel node class and registration."""

import subprocess
import sys
import threading
import pytest

from worker.nodes.base import NodeContext


def _make_ctx(mock: bool = True) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.

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
        pipeline_cache={},
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
    """Real-mode execute() raises NotImplementedError.

    Constructs a NodeContext with mock=False, calls execute() with
    model_id="test_model", and asserts that NotImplementedError is
    raised with a message indicating the deferred implementation.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker. A test asserting the expected exception
    is itself a legitimate, collectible real-mode test per the
    dual-mode parity convention.

    Expected outcome: NotImplementedError is raised.
    """
    from worker.nodes.loader import LoadModel

    node = LoadModel()
    ctx = _make_ctx(mock=False)
    with pytest.raises(NotImplementedError, match="P19-C2"):
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
