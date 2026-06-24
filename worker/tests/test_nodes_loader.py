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


def test_loadvae_registered_in_registry() -> None:
    """Verify ``LoadVae`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.loader`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``loader`` module, assert that
        ``"LoadVae"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"LoadVae"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "LoadVae"``.
    """
    # Re-import the loader module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadVae

    assert "LoadVae" in NODE_REGISTRY
    assert NODE_REGISTRY["LoadVae"] is LoadVae
    assert LoadVae.NODE_TYPE == "LoadVae"


def test_loadvae_execute_returns_mock_vae() -> None:
    """Verify ``execute()`` returns a ``MockVae`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``LoadVae`` with a ``mock_context``, call
        ``execute(model_id="test-vae")``, and assert the returned
        dict contains a ``MockVae`` instance.

    Expected output:
        ``result["vae"]`` is a ``MockVae`` instance.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadVae, MockVae

    node = LoadVae(mock_context)
    result = node.execute(model_id="test-vae")

    assert "vae" in result
    assert isinstance(result["vae"], MockVae)


def test_loadvae_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``LoadVae``.

    Preconditions:
        ``LoadVae`` class is accessible via direct import from
        ``worker.nodes.loader``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type.

    Expected output:
        ``NODE_TYPE == "LoadVae"``, ``CATEGORY == "Loaders"``,
        ``DISPLAY_NAME == "Load VAE"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has one
        ``SlotSpec("model_id", "STRING")``, and ``OUTPUT_SLOTS``
        has one ``SlotSpec("vae", "VAE")``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadVae

    assert LoadVae.NODE_TYPE == "LoadVae"
    assert LoadVae.CATEGORY == "Loaders"
    assert LoadVae.DISPLAY_NAME == "Load VAE"
    assert isinstance(LoadVae.DESCRIPTION, str)
    assert len(LoadVae.DESCRIPTION) > 0

    # Verify INPUT_SLOTS.
    assert len(LoadVae.INPUT_SLOTS) == 1
    input_spec = LoadVae.INPUT_SLOTS[0]
    assert isinstance(input_spec, SlotSpec)
    assert input_spec.name == "model_id"
    assert input_spec.slot_type == "STRING"
    assert input_spec.optional is False

    # Verify OUTPUT_SLOTS.
    assert len(LoadVae.OUTPUT_SLOTS) == 1
    output_spec = LoadVae.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "vae"
    assert output_spec.slot_type == "VAE"
    assert output_spec.optional is False


def test_loadclip_registered_in_registry() -> None:
    """Verify ``LoadClip`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.loader`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``loader`` module, assert that
        ``"LoadClip"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"LoadClip"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "LoadClip"``.
    """
    # Re-import the loader module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadClip

    assert "LoadClip" in NODE_REGISTRY
    assert NODE_REGISTRY["LoadClip"] is LoadClip
    assert LoadClip.NODE_TYPE == "LoadClip"


def test_loadclip_execute_returns_mock_clip_default_type() -> None:
    """Verify ``execute()`` returns a ``MockClip`` with ``clip_type="qwen3"`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``LoadClip`` with a ``mock_context``, call
        ``execute(model_id="test-model")`` without providing ``clip_type``,
        and assert the returned dict contains a ``MockClip`` with
        ``clip_type == "qwen3"`` (the default).

    Expected output:
        ``result["clip"]`` is a ``MockClip`` instance with
        ``result["clip"].clip_type == "qwen3"``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadClip, MockClip

    node = LoadClip(mock_context)
    result = node.execute(model_id="test-model")

    assert "clip" in result
    assert isinstance(result["clip"], MockClip)
    assert result["clip"].clip_type == "qwen3"


def test_loadclip_execute_returns_mock_clip_explicit_type() -> None:
    """Verify ``execute()`` returns a ``MockClip`` with the explicit ``clip_type`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``LoadClip`` with a ``mock_context``, call
        ``execute(model_id="test-model", clip_type="clip_l")``, and
        assert the returned dict contains a ``MockClip`` with
        ``clip_type == "clip_l"``.

    Expected output:
        ``result["clip"]`` is a ``MockClip`` instance with
        ``result["clip"].clip_type == "clip_l"``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadClip, MockClip

    node = LoadClip(mock_context)
    result = node.execute(model_id="test-model", clip_type="clip_l")

    assert "clip" in result
    assert isinstance(result["clip"], MockClip)
    assert result["clip"].clip_type == "clip_l"


def test_loadclip_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``LoadClip``.

    Preconditions:
        ``LoadClip`` class is accessible via direct import from
        ``worker.nodes.loader``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type.

    Expected output:
        ``NODE_TYPE == "LoadClip"``, ``CATEGORY == "Loaders"``,
        ``DISPLAY_NAME == "Load CLIP"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has two specs
        (``model_id`` STRING required, ``clip_type`` STRING optional),
        and ``OUTPUT_SLOTS`` has one ``SlotSpec("clip", "CLIP")``.
    """
    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import LoadClip

    assert LoadClip.NODE_TYPE == "LoadClip"
    assert LoadClip.CATEGORY == "Loaders"
    assert LoadClip.DISPLAY_NAME == "Load CLIP"
    assert isinstance(LoadClip.DESCRIPTION, str)
    assert len(LoadClip.DESCRIPTION) > 0

    # Verify INPUT_SLOTS — two specs: model_id (required), clip_type (optional).
    assert len(LoadClip.INPUT_SLOTS) == 2
    model_id_spec = LoadClip.INPUT_SLOTS[0]
    assert isinstance(model_id_spec, SlotSpec)
    assert model_id_spec.name == "model_id"
    assert model_id_spec.slot_type == "STRING"
    assert model_id_spec.optional is False

    clip_type_spec = LoadClip.INPUT_SLOTS[1]
    assert isinstance(clip_type_spec, SlotSpec)
    assert clip_type_spec.name == "clip_type"
    assert clip_type_spec.slot_type == "STRING"
    assert clip_type_spec.optional is True

    # Verify OUTPUT_SLOTS.
    assert len(LoadClip.OUTPUT_SLOTS) == 1
    output_spec = LoadClip.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "clip"
    assert output_spec.slot_type == "CLIP"
    assert output_spec.optional is False


def test_loadmodel_hf_directory_accepts_device_param() -> None:
    """Verify ``_load_model_from_hf_directory`` accepts a ``device`` parameter.

    This test confirms the function signature includes a third positional
    ``device`` argument (default ``"cpu"``) so that downstream callers
    can pass ``self.ctx.device`` for GPU placement.

    Preconditions:
        ``torch``, ``diffusers``, and ``safetensors`` are installed
        (real mode). When these are absent the test is skipped via
        ``pytest.importorskip``.

    Tests:
        Import the function, inspect its signature via
        ``inspect.signature``, and assert that ``"device"`` is a
        parameter name with a default value of ``"cpu"``.

    Expected output:
        The ``device`` parameter exists with default ``"cpu"``.
    """
    # Real loading requires torch/diffusers/safetensors which are not
    # installed in the CI mock-mode venv. Skip when absent.
    torch = pytest.importorskip("torch")
    del torch  # we only need the import to succeed, not the object

    import inspect

    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import _load_model_from_hf_directory

    sig = inspect.signature(_load_model_from_hf_directory)
    params = sig.parameters

    assert "device" in params, (
        "_load_model_from_hf_directory must accept a 'device' parameter "
        "for GPU placement"
    )
    assert params["device"].default == "cpu", (
        "device parameter must default to 'cpu' for backward compatibility"
    )
