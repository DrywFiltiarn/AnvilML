"""Tests for worker.nodes.encoder — ClipTextEncode node class and registration."""

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


def test_clip_text_encode_class_attributes() -> None:
    """ClipTextEncode defines all six required class attributes with correct values.

    Verifies NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS
    (3 slots: clip, positive_text, negative_text), and OUTPUT_SLOTS (1 slot:
    conditioning) match the values specified in the plan.

    This test exercises the class definition and satisfies the class-attribute
    portion of the acceptance criteria.

    Expected outcome: All assertions pass with the correct values.
    """
    from worker.nodes.encoder import ClipTextEncode
    from worker.nodes.base import SlotSpec

    assert ClipTextEncode.NODE_TYPE == "ClipTextEncode"
    assert ClipTextEncode.CATEGORY == "Conditioning"
    assert ClipTextEncode.DISPLAY_NAME == "Clip Text Encode"
    assert (
        ClipTextEncode.DESCRIPTION
        == "Encodes a text prompt using a loaded CLIP-compatible encoder."
    )

    # Verify INPUT_SLOTS: 3 slots with correct names and types.
    assert len(ClipTextEncode.INPUT_SLOTS) == 3
    assert ClipTextEncode.INPUT_SLOTS[0] == SlotSpec("clip", "CLIP")
    assert ClipTextEncode.INPUT_SLOTS[1] == SlotSpec("positive_text", "STRING")
    assert ClipTextEncode.INPUT_SLOTS[2] == SlotSpec(
        "negative_text", "STRING", optional=True
    )

    # Verify OUTPUT_SLOTS: 1 slot with correct name and type.
    assert len(ClipTextEncode.OUTPUT_SLOTS) == 1
    assert ClipTextEncode.OUTPUT_SLOTS[0] == SlotSpec("conditioning", "CONDITIONING")


def test_clip_text_encode_mock_returns_sentinel() -> None:
    """Mock-mode execute() returns the sentinel dict with propagated positive_text.

    Constructs a NodeContext with mock=True, calls execute() with
    clip={} and positive_text="a red fox", and asserts the return dict
    matches the expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"conditioning": {"mock": True, "positive_text": "a red fox"}}
    is returned.
    """
    from worker.nodes.encoder import ClipTextEncode

    node = ClipTextEncode()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, clip={}, positive_text="a red fox")
    assert result == {
        "conditioning": {"mock": True, "positive_text": "a red fox"}
    }


def test_clip_text_encode_mock_without_negative_text() -> None:
    """Omitting optional negative_text input does not cause an error.

    Constructs a NodeContext with mock=True, calls execute() with only
    clip={} and positive_text="hello" (omitting negative_text entirely),
    and verifies the sentinel is returned correctly.

    This tests the optional input slot handling — the node should not
    require negative_text since it is declared as optional.

    Expected outcome: {"conditioning": {"mock": True, "positive_text": "hello"}}
    is returned without error.
    """
    from worker.nodes.encoder import ClipTextEncode

    node = ClipTextEncode()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, clip={}, positive_text="hello")
    assert result == {
        "conditioning": {"mock": True, "positive_text": "hello"}
    }


def test_clip_text_encode_in_registry() -> None:
    """ClipTextEncode appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.encoder in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["ClipTextEncode"]
    exists. This proves auto-import and registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_nodes_loader.py::
    test_load_model_in_registry.

    Expected outcome: NODE_REGISTRY contains "ClipTextEncode" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.encoder'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'ClipTextEncode' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['ClipTextEncode'] is mod.ClipTextEncode; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout
