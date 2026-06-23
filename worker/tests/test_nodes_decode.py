"""Unit tests for the VaeDecode node and MockImage sentinel.

Tests cover registry registration, mock-mode execution, missing-input
handling, metadata attribute verification, and the real decode path.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import os
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


def test_vaedeode_registered_in_registry() -> None:
    """Verify ``VaeDecode`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.decode`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``decode`` module, assert that
        ``"VaeDecode"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"VaeDecode"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "VaeDecode"``.
    """
    # Re-import the decode module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.decode

    importlib.reload(worker.nodes.decode)
    from worker.nodes.decode import VaeDecode

    assert "VaeDecode" in NODE_REGISTRY
    assert NODE_REGISTRY["VaeDecode"] is VaeDecode
    assert VaeDecode.NODE_TYPE == "VaeDecode"


def test_vaedeode_execute_returns_mock_image() -> None:
    """Verify ``execute()`` returns a ``MockImage`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``VaeDecode`` with a ``mock_context``, call
        ``execute(vae=MockVae(), latent=MockLatent())``, and assert
        the returned dict contains a ``MockImage``.

    Expected output:
        ``result["image"]`` is a ``MockImage`` instance.
    """
    import worker.nodes.decode

    importlib.reload(worker.nodes.decode)
    from worker.nodes.decode import MockImage, VaeDecode
    from worker.nodes.loader import MockVae
    from worker.nodes.sampler import MockLatent

    node = VaeDecode(mock_context)
    result = node.execute(vae=MockVae(), latent=MockLatent(width=8, height=8))

    assert "image" in result
    assert isinstance(result["image"], MockImage)


def test_vaedeode_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``VaeDecode``.

    Preconditions:
        ``VaeDecode`` class is accessible via direct import from
        ``worker.nodes.decode``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type.

    Expected output:
        ``NODE_TYPE == "VaeDecode"``, ``CATEGORY == "Decoding"``,
        ``DISPLAY_NAME == "VAE Decode"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has two specs
        (``vae:VAE`` required, ``latent:LATENT`` required), and
        ``OUTPUT_SLOTS`` has one spec (``image:IMAGE`` required).
    """
    import worker.nodes.decode

    importlib.reload(worker.nodes.decode)
    from worker.nodes.decode import VaeDecode

    assert VaeDecode.NODE_TYPE == "VaeDecode"
    assert VaeDecode.CATEGORY == "Decoding"
    assert VaeDecode.DISPLAY_NAME == "VAE Decode"
    assert isinstance(VaeDecode.DESCRIPTION, str)
    assert len(VaeDecode.DESCRIPTION) > 0

    # Verify INPUT_SLOTS — two specs: vae (VAE, required),
    # latent (LATENT, required).
    assert len(VaeDecode.INPUT_SLOTS) == 2
    vae_spec = VaeDecode.INPUT_SLOTS[0]
    assert isinstance(vae_spec, SlotSpec)
    assert vae_spec.name == "vae"
    assert vae_spec.slot_type == "VAE"
    assert vae_spec.optional is False

    latent_spec = VaeDecode.INPUT_SLOTS[1]
    assert isinstance(latent_spec, SlotSpec)
    assert latent_spec.name == "latent"
    assert latent_spec.slot_type == "LATENT"
    assert latent_spec.optional is False

    # Verify OUTPUT_SLOTS — one spec: image (IMAGE, required).
    assert len(VaeDecode.OUTPUT_SLOTS) == 1
    output_spec = VaeDecode.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "image"
    assert output_spec.slot_type == "IMAGE"
    assert output_spec.optional is False


def test_vaedeode_execute_missing_inputs_returns_mock() -> None:
    """Verify ``execute()`` handles missing inputs gracefully in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Call ``execute()`` without providing any inputs (matching
        how ``LoadModel`` handles missing ``model_id``). Mock mode
        ignores inputs entirely.

    Expected output:
        ``result["image"]`` is a ``MockImage`` — mock mode does not
        require or validate the vae/latent inputs.
    """
    import worker.nodes.decode

    importlib.reload(worker.nodes.decode)
    from worker.nodes.decode import MockImage, VaeDecode

    node = VaeDecode(mock_context)
    result = node.execute()

    assert "image" in result
    assert isinstance(result["image"], MockImage)


# ---------------------------------------------------------------------------
# Real-path test (non-mock mode)
# ---------------------------------------------------------------------------


class _MockVaeConfig:
    """Minimal VAE config for the real decode path test.

    Mirrors the config attributes that ``VaeDecode.execute()`` reads
    from a real ``AutoencoderKL`` instance.

    Attributes:
        scaling_factor: The VAE's latent scaling factor (default 0.18215
            for SD-style VAEs).
        shift_factor: The VAE's latent shift factor (defaults to 0.0
            when the model predates the shift parameter).
    """

    def __init__(self, scaling_factor: float = 0.18215, shift_factor: float = 0.0) -> None:
        """Initialise the mock VAE config.

        Args:
            scaling_factor: The latent scaling factor. Defaults to
                0.18215 (the diffusers AutoencoderKL default).
            shift_factor: The latent shift factor. Defaults to 0.0.
        """
        self.scaling_factor = scaling_factor
        self.shift_factor = shift_factor


class _MockVaeWithDecode:
    """Mock VAE with a real ``decode()`` method for the real-path test.

    The ``decode()`` method returns a tuple containing a real torch
    tensor, which ``VaeDecode.execute()`` passes to
    ``VaeImageProcessor.postprocess()``.

    Attributes:
        config: A ``_MockVaeConfig`` instance carrying
            ``scaling_factor`` and ``shift_factor``.
    """

    def __init__(self, scaling_factor: float = 0.18215, shift_factor: float = 0.0) -> None:
        """Initialise a mock VAE with a real decode method.

        Args:
            scaling_factor: Passed through to ``_MockVaeConfig``.
            shift_factor: Passed through to ``_MockVaeConfig``.
        """
        self.config = _MockVaeConfig(scaling_factor, shift_factor)

    def decode(self, latents: Any, return_dict: bool = True) -> tuple:
        """Decode latent tensor to a raw image tensor.

        Returns a plain tuple (since ``return_dict=False`` is always
        passed by ``VaeDecode.execute()``) containing a real torch
        tensor in the ``[-1, 1]`` range.

        Args:
            latents: The latent tensor to decode.
            return_dict: Unused — the real method always returns a
                plain tuple to match the ``return_dict=False`` call
                in ``VaeDecode.execute()``.

        Returns:
            A tuple with one element: a ``torch.Tensor`` in the
            ``[-1, 1]`` range (typical VAE decoder output).
        """
        import torch

        # Produce a small random tensor in [-1, 1] — the exact values
        # don't matter for the test; only the shape and type matter.
        # The postprocess step will handle any valid tensor.
        return (torch.rand(1, 3, 64, 64, dtype=torch.float32),)


def test_vaedeode_real_path_returns_pil_image() -> None:
    """Verify ``execute()`` returns a ``PIL.Image.Image`` in real mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK`` is unset (cleared by this test) so that
        the real decode code path is exercised.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Clear ``ANVILML_WORKER_MOCK``, instantiate ``VaeDecode`` with
        a ``mock_context``, call ``execute()`` with a ``MockVaeWithDecode``
        and a real torch tensor as ``latent``, and assert the returned
        image is a ``PIL.Image.Image``.

    Expected output:
        ``result["image"]`` is a ``PIL.Image.Image`` instance.
    """
    # Capture the pre-existing env value and restore unconditionally
    # after the test, per the env isolation convention (§11.3).
    # The conftest sets ANVILML_WORKER_MOCK=1, so we pop it to
    # exercise the real-mode branch.
    original = os.environ.pop("ANVILML_WORKER_MOCK", None)
    try:
        import worker.nodes.decode

        importlib.reload(worker.nodes.decode)
        from worker.nodes.decode import VaeDecode

        import torch

        vae = _MockVaeWithDecode()
        latent = torch.randn(1, 4, 64, 64, dtype=torch.float32)

        node = VaeDecode(mock_context)
        result = node.execute(vae=vae, latent=latent)

        assert "image" in result
        # Verify the image is a real PIL Image, not a MockImage sentinel.
        from PIL import Image

        assert isinstance(result["image"], Image.Image)
        # Also verify it is NOT a MockImage — the sentinel must be
        # absent from the real-mode output.
        assert not isinstance(result["image"], worker.nodes.decode.MockImage)
    finally:
        # Restore the env var unconditionally so no other test sees
        # a modified environment.
        if original is not None:
            os.environ["ANVILML_WORKER_MOCK"] = original
