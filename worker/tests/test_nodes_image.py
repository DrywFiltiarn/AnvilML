"""Tests for worker.nodes.image — SaveImage node class and registration."""

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
            compatibility with existing tests.

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


def test_save_image_mock_emits_image_ready() -> None:
    """Mock-mode SaveImage.execute() emits ImageReady event dict.

    Constructs a NodeContext with mock=True, calls execute() with
    image={"mock": True, "width": 512, "height": 512}, and asserts that
    ctx.emit was called with a dict containing _type == "ImageReady",
    width == 64, and height == 64.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: ctx.emit is called with an ImageReady event dict
    containing width=64, height=64.
    """
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=True)
    ctx.emit = _emit

    result = node.execute(
        ctx,
        image={"mock": True, "width": 512, "height": 512},
    )

    # Verify emit was called exactly once.
    assert len(captured_events) == 1, f"expected 1 emit call, got {len(captured_events)}"

    event = captured_events[0]

    # Verify the event type.
    assert event["_type"] == "ImageReady", (
        f"expected _type='ImageReady', got '{event['_type']}'"
    )

    # Verify the image dimensions in the event.
    assert event["width"] == 64, f"expected width=64, got {event['width']}"
    assert event["height"] == 64, f"expected height=64, got {event['height']}"

    # Verify the sentinel return value.
    assert result == {"image": {"mock": True, "width": 64, "height": 64}}


def test_save_image_in_registry() -> None:
    """SaveImage appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.image in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["SaveImage"]
    exists and equals the imported class. This proves auto-import and
    registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_nodes_loader.py::
    test_load_model_in_registry.

    Expected outcome: NODE_REGISTRY contains "SaveImage" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.image'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'SaveImage' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['SaveImage'] is mod.SaveImage; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


def test_save_image_missing_image_input_raises() -> None:
    """SaveImage.execute() raises KeyError when image input is missing.

    Constructs a NodeContext with mock=True and calls execute() without
    the required "image" input. Asserts that a KeyError is raised —
    Python's natural behavior when accessing a missing dict key in
    **inputs.

    This test verifies that the node fails gracefully when a required
    input is absent, rather than producing a cryptic error elsewhere.

    Expected outcome: KeyError is raised.
    """
    from worker.nodes.image import SaveImage

    node = SaveImage()
    ctx = _make_ctx(mock=True)

    with pytest.raises(KeyError):
        node.execute(ctx)
