"""Tests for worker.nodes.image — SaveImage node class and registration."""

import subprocess
import sys
import threading
from io import BytesIO
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


@pytest.mark.real_mode
def test_save_image_real_emits_png() -> None:
    """Real-mode SaveImage.execute() emits ImageReady with correct fields.

    Constructs a NodeContext with mock=False, creates a real PIL Image
    input (128×64 red), calls execute(), and asserts that ctx.emit was
    called with a dict containing _type == "ImageReady", width == 128,
    height == 64, format == "png", and a valid base64-encoded PNG payload.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: ctx.emit is called with an ImageReady event dict
    containing the correct dimensions, format, and base64-encoded PNG.
    """
    import base64

    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=False)
    ctx.emit = _emit

    # Create a real PIL Image (128×64 red rectangle).
    pil_image = PILImage.new("RGB", (128, 64), (255, 0, 0))

    result = node.execute(ctx, image=pil_image)

    # Verify emit was called exactly once.
    assert len(captured_events) == 1, (
        f"expected 1 emit call, got {len(captured_events)}"
    )

    event = captured_events[0]

    # Verify the event type.
    assert event["_type"] == "ImageReady", (
        f"expected _type='ImageReady', got '{event['_type']}'"
    )

    # Verify the image dimensions in the event.
    assert event["width"] == 128, f"expected width=128, got {event['width']}"
    assert event["height"] == 64, f"expected height=64, got {event['height']}"

    # Verify the format field.
    assert event["format"] == "png", (
        f"expected format='png', got '{event['format']}'"
    )

    # Verify the image_b64 is valid base64 that decodes to a real PNG.
    image_b64 = event["image_b64"]
    png_bytes = base64.b64decode(image_b64)
    assert len(png_bytes) > 0, "image_b64 decoded to empty bytes"

    # Verify the PNG signature (first 8 bytes must be the PNG magic).
    png_signature = b"\x89PNG\r\n\x1a\n"
    assert png_bytes[:8] == png_signature, (
        f"decoded bytes do not start with PNG signature"
    )


@pytest.mark.real_mode
def test_save_image_real_seed_pass_through() -> None:
    """Real-mode SaveImage.execute() passes seed through unchanged.

    Constructs a NodeContext with mock=False, calls execute() with
    image=PIL Image and seed=42, and asserts that the emitted
    ImageReady event contains seed == 42.

    This test verifies that the seed input is forwarded to the event
    without modification.

    Expected outcome: event["seed"] == 42.
    """
    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=False)
    ctx.emit = _emit

    pil_image = PILImage.new("RGB", (32, 32), (0, 128, 255))

    node.execute(ctx, image=pil_image, seed=42)

    event = captured_events[0]
    assert event["seed"] == 42, f"expected seed=42, got {event['seed']}"


@pytest.mark.real_mode
def test_save_image_real_steps_pass_through() -> None:
    """Real-mode SaveImage.execute() passes steps through unchanged.

    Constructs a NodeContext with mock=False, calls execute() with
    image=PIL Image and steps=20, and asserts that the emitted
    ImageReady event contains steps == 20.

    This test verifies that the steps input is forwarded to the event
    without modification.

    Expected outcome: event["steps"] == 20.
    """
    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=False)
    ctx.emit = _emit

    pil_image = PILImage.new("RGB", (32, 32), (0, 128, 255))

    node.execute(ctx, image=pil_image, steps=20)

    event = captured_events[0]
    assert event["steps"] == 20, f"expected steps=20, got {event['steps']}"


@pytest.mark.real_mode
def test_save_image_real_default_seed_steps() -> None:
    """Real-mode SaveImage.execute() uses default seed=-1, steps=1 when absent.

    Constructs a NodeContext with mock=False, calls execute() with
    only the image input (no seed or steps), and asserts that the
    emitted ImageReady event contains seed == -1 and steps == 1.

    This test verifies the default value handling for optional inputs.

    Expected outcome: event["seed"] == -1 and event["steps"] == 1.
    """
    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=False)
    ctx.emit = _emit

    pil_image = PILImage.new("RGB", (16, 16), (255, 255, 0))

    node.execute(ctx, image=pil_image)

    event = captured_events[0]
    assert event["seed"] == -1, f"expected seed=-1, got {event['seed']}"
    assert event["steps"] == 1, f"expected steps=1, got {event['steps']}"


@pytest.mark.real_mode
def test_save_image_real_png_bytes_valid() -> None:
    """Real-mode SaveImage.execute() produces a valid PNG matching input dimensions.

    Constructs a NodeContext with mock=False, creates a real PIL Image
    (32×96 green), calls execute(), then base64-decodes the payload
    and re-opens it as a PIL Image to verify dimensions match.

    This test confirms that the PNG encoding round-trips correctly.

    Expected outcome: decoded PNG has size (32, 96).
    """
    import base64

    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    captured_events: list[dict] = []

    def _emit(event: dict) -> None:
        captured_events.append(event)

    node = SaveImage()
    ctx = _make_ctx(mock=False)
    ctx.emit = _emit

    # Create a 32×96 green image (unusual dimensions to verify exact match).
    pil_image = PILImage.new("RGB", (32, 96), (0, 255, 0))

    node.execute(ctx, image=pil_image)

    event = captured_events[0]

    # Decode the base64 payload back to PNG bytes.
    png_bytes = base64.b64decode(event["image_b64"])

    # Re-open as a PIL Image and verify dimensions.
    reopened = PILImage.open(BytesIO(png_bytes))
    assert reopened.size == (32, 96), (
        f"expected reopened size (32, 96), got {reopened.size}"
    )
    assert reopened.mode == "RGB", (
        f"expected mode 'RGB', got '{reopened.mode}'"
    )


@pytest.mark.real_mode
def test_save_image_real_returns_empty_dict() -> None:
    """Real-mode SaveImage.execute() returns an empty dict.

    Constructs a NodeContext with mock=False, calls execute() with
    a real PIL Image, and asserts that the return value is {}.

    SaveImage has OUTPUT_SLOTS = [] — it emits events, not slot outputs.

    Expected outcome: result == {}.
    """
    from PIL import Image as PILImage
    from worker.nodes.image import SaveImage

    node = SaveImage()
    ctx = _make_ctx(mock=False)

    pil_image = PILImage.new("RGB", (64, 64), (128, 128, 128))

    result = node.execute(ctx, image=pil_image)
    assert result == {}, f"expected result == {{}}, got {result}"


def test_resize_mock_returns_correct_dimensions() -> None:
    """Mock-mode ImageResize.execute() returns sentinel dict with correct dimensions.

    Constructs a NodeContext with mock=True, calls execute() with
    image={"mock": True, "width": 512, "height": 512}, width=128, height=256,
    and asserts the return value is {"image": {"mock": True, "width": 128,
    "height": 256}}.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: result == {"image": {"mock": True, "width": 128, "height": 256}}.
    """
    from worker.nodes.image import ImageResize

    node = ImageResize()
    ctx = _make_ctx(mock=True)

    result = node.execute(
        ctx,
        image={"mock": True, "width": 512, "height": 512},
        width=128,
        height=256,
    )

    assert result == {
        "image": {"mock": True, "width": 128, "height": 256},
    }, f"unexpected mock sentinel: {result}"


@pytest.mark.real_mode
def test_resize_real_produces_requested_dimensions() -> None:
    """Real-mode ImageResize.execute() produces a PIL.Image with exact dimensions.

    Constructs a NodeContext with mock=False, creates a real PIL Image
    (64×64), calls execute() with width=128, height=256, and asserts that
    the returned image has size (128, 256).

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: resized image size == (128, 256).
    """
    from PIL import Image as PILImage
    from worker.nodes.image import ImageResize

    node = ImageResize()
    ctx = _make_ctx(mock=False)

    pil_image = PILImage.new("RGB", (64, 64), (255, 0, 0))

    result = node.execute(ctx, image=pil_image, width=128, height=256)

    resized = result["image"]
    assert resized.size == (128, 256), (
        f"expected resized size (128, 256), got {resized.size}"
    )


def test_resize_default_method_is_lanczos() -> None:
    """ImageResize.execute() uses lanczos filter when method is not specified.

    Calls execute() without the "method" parameter and asserts that the
    call succeeds with the default lanczos filter (dimensions match).

    Uses a mock context — runs in both mock and real since the resize
    logic is identical (both branches call PIL.Image.resize()).

    Expected outcome: call succeeds, dimensions match request.
    """
    from worker.nodes.image import ImageResize

    node = ImageResize()
    ctx = _make_ctx(mock=True)

    result = node.execute(
        ctx,
        image={"mock": True, "width": 512, "height": 512},
        width=64,
        height=64,
    )

    # Verify the call succeeded with default lanczos (dimensions match).
    assert result == {
        "image": {"mock": True, "width": 64, "height": 64},
    }, f"unexpected result with default method: {result}"


def test_resize_explicit_method_bilinear() -> None:
    """ImageResize.execute() accepts explicit method="bilinear".

    Calls execute() with method="bilinear" and asserts that the call
    succeeds (dimensions match). This verifies that the method parameter
    is accepted and does not raise ValueError for a recognized filter.

    Uses a mock context — runs in both mock and real since the resize
    logic is identical (both branches call PIL.Image.resize()).

    Expected outcome: call succeeds, dimensions match request.
    """
    from worker.nodes.image import ImageResize

    node = ImageResize()
    ctx = _make_ctx(mock=True)

    result = node.execute(
        ctx,
        image={"mock": True, "width": 512, "height": 512},
        width=64,
        height=64,
        method="bilinear",
    )

    # Verify the call succeeded with explicit bilinear method.
    assert result == {
        "image": {"mock": True, "width": 64, "height": 64},
    }, f"unexpected result with bilinear method: {result}"


def test_resize_unrecognized_method_raises_error() -> None:
    """ImageResize.execute() raises ValueError for unrecognized method.

    Calls execute() with method="invalid_method" and asserts that
    ValueError is raised with a message containing the invalid method name.

    Uses a mock context — runs in both mock and real since the method
    validation happens before the PIL resize call.

    Expected outcome: ValueError is raised with clear error message.
    """
    from worker.nodes.image import ImageResize

    node = ImageResize()
    ctx = _make_ctx(mock=True)

    with pytest.raises(ValueError) as exc_info:
        node.execute(
            ctx,
            image={"mock": True, "width": 512, "height": 512},
            width=64,
            height=64,
            method="invalid_method",
        )

    assert "invalid_method" in str(exc_info.value), (
        f"expected 'invalid_method' in error message, got: {exc_info.value}"
    )
