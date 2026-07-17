"""Tests for worker.nodes.decode — VaeDecode node class and registration."""

import subprocess
import sys
import threading

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


def test_vae_decode_class_attributes() -> None:
    """VaeDecode defines all six required class attributes with correct values.

    Verifies NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS
    (2 slots: vae, latent), and OUTPUT_SLOTS (1 slot: image) match the
    values specified in ANVILML_DESIGN.md §10.3.

    This test exercises the class definition and satisfies the class-attribute
    portion of the acceptance criteria.

    Expected outcome: All assertions pass with the correct values.
    """
    from worker.nodes.decode import VaeDecode
    from worker.nodes.base import SlotSpec

    assert VaeDecode.NODE_TYPE == "VaeDecode"
    assert VaeDecode.CATEGORY == "Decoding"
    assert VaeDecode.DISPLAY_NAME == "VAE Decode"
    assert (
        VaeDecode.DESCRIPTION
        == "Decodes a denoised latent to a PIL image using the explicitly provided VAE."
    )

    # Verify INPUT_SLOTS: 2 slots with correct names and types.
    assert len(VaeDecode.INPUT_SLOTS) == 2
    assert VaeDecode.INPUT_SLOTS[0] == SlotSpec("vae", "VAE")
    assert VaeDecode.INPUT_SLOTS[1] == SlotSpec("latent", "LATENT")

    # Verify OUTPUT_SLOTS: 1 slot with correct name and type.
    assert len(VaeDecode.OUTPUT_SLOTS) == 1
    assert VaeDecode.OUTPUT_SLOTS[0] == SlotSpec("image", "IMAGE")


def test_vae_decode_mock_returns_sentinel() -> None:
    """Mock-mode execute() returns the sentinel dict with propagated shape.

    Constructs a NodeContext with mock=True, calls execute() with
    vae={} and latent={"shape": (1, 4, 64, 64)}, and asserts the return
    dict matches the expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"image": {"mock": True, "shape": (1, 4, 64, 64)}}
    is returned.
    """
    from worker.nodes.decode import VaeDecode

    node = VaeDecode()
    ctx = _make_ctx(mock=True)
    result = node.execute(
        ctx,
        vae={},
        latent={"shape": (1, 4, 64, 64)},
    )
    assert result == {"image": {"mock": True, "shape": (1, 4, 64, 64)}}


def test_vae_decode_in_registry() -> None:
    """VaeDecode appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.decode in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["VaeDecode"]
    exists and is the VaeDecode class. This proves auto-import and
    registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_nodes_encoder.py::
    test_clip_text_encode_in_registry.

    Expected outcome: NODE_REGISTRY contains "VaeDecode" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.decode'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'VaeDecode' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['VaeDecode'] is mod.VaeDecode; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout
