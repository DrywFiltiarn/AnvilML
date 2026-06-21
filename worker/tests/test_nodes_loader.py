"""Unit tests for the LoadModel node and MockModel sentinel.

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


def test_loadmodel_registered_in_registry() -> None:
    """Verify ``LoadModel`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.loader`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``loader`` module, assert that
        ``"LoadModel"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"LoadModel"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "LoadModel"``.
    """
    # Re-import the loader module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadModel

    assert "LoadModel" in NODE_REGISTRY
    assert NODE_REGISTRY["LoadModel"] is LoadModel
    assert LoadModel.NODE_TYPE == "LoadModel"


def test_loadmodel_execute_returns_mock_model() -> None:
    """Verify ``execute()`` returns a ``MockModel`` with ``arch="zit"`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``LoadModel`` with a ``mock_context``, call
        ``execute(model_id="test-model")``, and assert the returned
        dict contains a ``MockModel`` with ``arch == "zit"``.

    Expected output:
        ``result["model"]`` is a ``MockModel`` instance with
        ``result["model"].arch == "zit"``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadModel, MockModel

    node = LoadModel(mock_context)
    result = node.execute(model_id="test-model")

    assert "model" in result
    assert isinstance(result["model"], MockModel)
    assert result["model"].arch == "zit"


def test_loadmodel_execute_missing_model_id_defaults_empty() -> None:
    """Verify ``execute()`` handles missing ``model_id`` gracefully in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Call ``execute()`` without providing a ``model_id`` key in
        the inputs dict. The mock code path ignores the model_id
        value entirely.

    Expected output:
        ``result["model"]`` is a ``MockModel(arch="zit")`` — mock
        mode does not require or validate the model_id input.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadModel, MockModel

    node = LoadModel(mock_context)
    result = node.execute()

    assert "model" in result
    assert isinstance(result["model"], MockModel)
    assert result["model"].arch == "zit"


def test_loadmodel_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``LoadModel``.

    Preconditions:
        ``LoadModel`` class is accessible via direct import from
        ``worker.nodes.loader``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type.

    Expected output:
        ``NODE_TYPE == "LoadModel"``, ``CATEGORY == "Loaders"``,
        ``DISPLAY_NAME == "Load Model"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has one
        ``SlotSpec("model_id", "STRING")``, and ``OUTPUT_SLOTS``
        has one ``SlotSpec("model", "MODEL")``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadModel

    assert LoadModel.NODE_TYPE == "LoadModel"
    assert LoadModel.CATEGORY == "Loaders"
    assert LoadModel.DISPLAY_NAME == "Load Model"
    assert isinstance(LoadModel.DESCRIPTION, str)
    assert len(LoadModel.DESCRIPTION) > 0

    # Verify INPUT_SLOTS.
    assert len(LoadModel.INPUT_SLOTS) == 1
    input_spec = LoadModel.INPUT_SLOTS[0]
    assert isinstance(input_spec, SlotSpec)
    assert input_spec.name == "model_id"
    assert input_spec.slot_type == "STRING"
    assert input_spec.optional is False

    # Verify OUTPUT_SLOTS.
    assert len(LoadModel.OUTPUT_SLOTS) == 1
    output_spec = LoadModel.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "model"
    assert output_spec.slot_type == "MODEL"
    assert output_spec.optional is False
