"""Unit tests for the ClipTextEncode node and MockConditioning sentinel.

Tests cover registry registration, mock-mode execution, metadata attribute
verification, and optional-negative-text handling.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
from typing import Any

import pytest

from worker.nodes import NODE_REGISTRY
from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def registry_clean() -> None:
    """Clear NODE_REGISTRY before each test to ensure isolation.

    The @register decorator modifies NODE_REGISTRY globally.
    This fixture clears it before each test so tests don't
    leak state into one another.
    """
    NODE_REGISTRY.clear()


@pytest.fixture
def mock_context() -> NodeContext:
    """Build a NodeContext with a captured emit callable.

    The emit callable stores all emitted events in a list so tests
    can inspect them. The cancel_flag is a list (mutable container)
    and the pipeline_cache is an empty dict.

    Returns:
        A NodeContext instance ready for use in tests.
    """
    emitted_events: list[dict[str, Any]] = []

    def capture_emit(data: dict[str, Any]) -> None:
        """Capture an emitted event for test inspection."""
        emitted_events.append(data)

    return NodeContext(
        job_id="test-job-1",
        device="cpu",
        cancel_flag=[False],
        emit=capture_emit,
        pipeline_cache={},
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_cliptextencode_registered_in_registry() -> None:
    """Verify ``ClipTextEncode`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.encoder`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``encoder`` module, assert that
        ``"ClipTextEncode"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"ClipTextEncode"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "ClipTextEncode"``.
    """
    # Re-import the encoder module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.encoder

    importlib.reload(worker.nodes.encoder)
    from worker.nodes.encoder import ClipTextEncode

    assert "ClipTextEncode" in NODE_REGISTRY
    assert NODE_REGISTRY["ClipTextEncode"] is ClipTextEncode
    assert ClipTextEncode.NODE_TYPE == "ClipTextEncode"


def test_cliptextencode_execute_returns_mock_conditioning() -> None:
    """Verify ``execute()`` returns a ``MockConditioning`` with correct text in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``ClipTextEncode`` with a ``mock_context``, call
        ``execute(clip=MockClip(), text="a cat sitting on a fence")``,
        and assert the returned dict contains a ``MockConditioning``
        with the correct text.

    Expected output:
        ``result["conditioning"]`` is a ``MockConditioning`` instance with
        ``result["conditioning"].text == "a cat sitting on a fence"``.
    """
    import worker.nodes.encoder

    importlib.reload(worker.nodes.encoder)
    from worker.nodes.encoder import ClipTextEncode, MockConditioning

    # Import MockClip from loader — it is the standard mock CLIP sentinel.
    from worker.nodes.loader import MockClip

    node = ClipTextEncode(mock_context)
    result = node.execute(clip=MockClip(), text="a cat sitting on a fence")

    assert "conditioning" in result
    assert isinstance(result["conditioning"], MockConditioning)
    assert result["conditioning"].text == "a cat sitting on a fence"


def test_cliptextencode_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``ClipTextEncode``.

    Preconditions:
        ``ClipTextEncode`` class is accessible via direct import from
        ``worker.nodes.encoder``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type.

    Expected output:
        ``NODE_TYPE == "ClipTextEncode"``, ``CATEGORY == "Conditioning"``,
        ``DISPLAY_NAME == "Clip Text Encode"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has three specs
        (clip CLIP required, text STRING required, negative_text STRING
        optional), and ``OUTPUT_SLOTS`` has one
        ``SlotSpec("conditioning", "CONDITIONING")``.
    """
    import worker.nodes.encoder

    importlib.reload(worker.nodes.encoder)
    from worker.nodes.encoder import ClipTextEncode

    assert ClipTextEncode.NODE_TYPE == "ClipTextEncode"
    assert ClipTextEncode.CATEGORY == "Conditioning"
    assert ClipTextEncode.DISPLAY_NAME == "Clip Text Encode"
    assert isinstance(ClipTextEncode.DESCRIPTION, str)
    assert len(ClipTextEncode.DESCRIPTION) > 0

    # Verify INPUT_SLOTS — three specs: clip (required), text (required),
    # negative_text (optional).
    assert len(ClipTextEncode.INPUT_SLOTS) == 3

    clip_spec = ClipTextEncode.INPUT_SLOTS[0]
    assert isinstance(clip_spec, SlotSpec)
    assert clip_spec.name == "clip"
    assert clip_spec.slot_type == "CLIP"
    assert clip_spec.optional is False

    text_spec = ClipTextEncode.INPUT_SLOTS[1]
    assert isinstance(text_spec, SlotSpec)
    assert text_spec.name == "text"
    assert text_spec.slot_type == "STRING"
    assert text_spec.optional is False

    neg_text_spec = ClipTextEncode.INPUT_SLOTS[2]
    assert isinstance(neg_text_spec, SlotSpec)
    assert neg_text_spec.name == "negative_text"
    assert neg_text_spec.slot_type == "STRING"
    assert neg_text_spec.optional is True

    # Verify OUTPUT_SLOTS.
    assert len(ClipTextEncode.OUTPUT_SLOTS) == 1
    output_spec = ClipTextEncode.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "conditioning"
    assert output_spec.slot_type == "CONDITIONING"
    assert output_spec.optional is False


def test_cliptextencode_negative_text_defaults_to_empty() -> None:
    """Verify ``execute()`` accepts inputs without ``negative_text`` without error.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Call ``execute()`` without providing a ``negative_text`` key in
        the inputs dict. The mock code path ignores negative_text
        entirely (consistent with how other mock nodes handle optional
        inputs).

    Expected output:
        ``result["conditioning"]`` is a ``MockConditioning`` instance with
        ``result["conditioning"].text == "hello"``.
    """
    import worker.nodes.encoder

    importlib.reload(worker.nodes.encoder)
    from worker.nodes.encoder import ClipTextEncode, MockConditioning

    from worker.nodes.loader import MockClip

    node = ClipTextEncode(mock_context)
    result = node.execute(clip=MockClip(), text="hello")

    assert "conditioning" in result
    assert isinstance(result["conditioning"], MockConditioning)
    assert result["conditioning"].text == "hello"
