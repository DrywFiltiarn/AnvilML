"""Unit tests for the VaeDecode node and MockImage sentinel.

Tests cover registry registration, mock-mode execution, missing-input
handling, and metadata attribute verification.

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
