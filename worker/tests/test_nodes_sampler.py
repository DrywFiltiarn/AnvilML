"""Unit tests for the EmptyLatent and Sampler nodes and MockLatent sentinel.

Tests cover registry registration, mock-mode execution, default-value
handling, seed resolution, metadata attributes, and the EMITS_PROGRESS
flag.

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


def test_emptylatent_registered_in_registry() -> None:
    """Verify ``EmptyLatent`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.sampler`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``sampler`` module, assert that
        ``"EmptyLatent"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"EmptyLatent"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "EmptyLatent"``.
    """
    # Re-import the sampler module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import EmptyLatent

    assert "EmptyLatent" in NODE_REGISTRY
    assert NODE_REGISTRY["EmptyLatent"] is EmptyLatent
    assert EmptyLatent.NODE_TYPE == "EmptyLatent"


def test_emptylatent_execute_returns_mock_latent() -> None:
    """Verify ``execute()`` returns a ``MockLatent`` with correct dimensions in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``EmptyLatent`` with a ``mock_context``, call
        ``execute(width=512, height=512, batch_size=4)``, and assert
        the returned dict contains a ``MockLatent`` with the correct
        width, height, and batch_size.

    Expected output:
        ``result["latent"]`` is a ``MockLatent`` instance with
        ``result["latent"].width == 512``,
        ``result["latent"].height == 512``, and
        ``result["latent"].batch_size == 4``.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import EmptyLatent, MockLatent

    node = EmptyLatent(mock_context)
    result = node.execute(width=512, height=512, batch_size=4)

    assert "latent" in result
    assert isinstance(result["latent"], MockLatent)
    assert result["latent"].width == 512
    assert result["latent"].height == 512
    assert result["latent"].batch_size == 4


def test_emptylatent_default_batch_size() -> None:
    """Verify ``execute()`` defaults ``batch_size`` to 1 when omitted.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Call ``execute()`` without providing a ``batch_size`` key in
        the inputs dict. The mock code path should default to 1.

    Expected output:
        ``result["latent"].batch_size == 1`` — the default batch size.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import EmptyLatent, MockLatent

    node = EmptyLatent(mock_context)
    result = node.execute(width=512, height=512)

    assert "latent" in result
    assert isinstance(result["latent"], MockLatent)
    assert result["latent"].batch_size == 1


def test_sampler_registered_in_registry() -> None:
    """Verify ``Sampler`` is registered in ``NODE_REGISTRY`` after importing.

    Preconditions:
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.
        ``worker.nodes.sampler`` is imported (and reloaded) so that
        the ``@register`` decorator runs.

    Tests:
        After re-importing the ``sampler`` module, assert that
        ``"Sampler"`` is a key in ``NODE_REGISTRY`` and that the
        registered class has the correct ``NODE_TYPE``.

    Expected output:
        ``"Sampler"`` present in ``NODE_REGISTRY``, keyed by
        ``NODE_TYPE == "Sampler"``.
    """
    # Re-import the sampler module so @register runs against the
    # now-empty NODE_REGISTRY. Python caches modules in sys.modules,
    # so we must use importlib.reload() to re-execute the module body.
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import Sampler

    assert "Sampler" in NODE_REGISTRY
    assert NODE_REGISTRY["Sampler"] is Sampler
    assert Sampler.NODE_TYPE == "Sampler"


def test_sampler_execute_returns_mock_latent_and_seed() -> None:
    """Verify ``execute()`` returns a ``MockLatent`` with correct seed in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Instantiate ``Sampler`` with a ``mock_context``, call
        ``execute()`` with all required inputs and ``seed=42``, and
        assert the returned dict contains a ``MockLatent`` with the
        correct dimensions and ``seed == 42``.

    Expected output:
        ``result["seed"] == 42`` and ``result["latent"]`` is a
        ``MockLatent(512, 512, 1)``.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import MockLatent, Sampler

    node = Sampler(mock_context)
    result = node.execute(
        model=None,
        conditioning=None,
        latent=MockLatent(512, 512, 1),
        steps=4,
        cfg=7.0,
        seed=42,
    )

    assert "latent" in result
    assert "seed" in result
    assert isinstance(result["latent"], MockLatent)
    assert result["seed"] == 42
    assert result["latent"].width == 512
    assert result["latent"].height == 512
    assert result["latent"].batch_size == 1


def test_sampler_seed_negative_one_resolves_to_random() -> None:
    """Verify ``seed=-1`` resolves to a random integer in ``[0, 2**32-1]``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Call ``execute()`` with ``seed=-1`` and assert the returned
        seed is in the valid range ``[0, 2**32-1]`` (not -1).

    Expected output:
        ``0 <= result["seed"] <= 4294967295`` — the resolved seed
        is a valid non-negative integer within the ComfyUI range.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import MockLatent, Sampler

    node = Sampler(mock_context)
    result = node.execute(
        model=None,
        conditioning=None,
        latent=MockLatent(512, 512, 1),
        steps=4,
        cfg=7.0,
        seed=-1,
    )

    assert "seed" in result
    assert isinstance(result["seed"], int)
    assert 0 <= result["seed"] <= 2**32 - 1
    assert result["seed"] != -1


def test_sampler_emits_progress_flag() -> None:
    """Verify ``Sampler.EMITS_PROGRESS`` is ``True`` so executor emits Progress events.

    Preconditions:
        None — this is a class-level attribute check.

    Tests:
        Assert that the ``EMITS_PROGRESS`` class attribute on
        ``Sampler`` is ``True``.

    Expected output:
        ``Sampler.EMITS_PROGRESS is True`` — the executor's
        progress-emission path will activate for this node.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import Sampler

    assert Sampler.EMITS_PROGRESS is True


def test_sampler_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``Sampler``.

    Preconditions:
        ``Sampler`` class is accessible via direct import from
        ``worker.nodes.sampler``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type. Also verify ``EMITS_PROGRESS`` is
        ``True`` and check the INPUT_SLOTS and OUTPUT_SLOTS structure.

    Expected output:
        ``NODE_TYPE == "Sampler"``, ``CATEGORY == "Sampling"``,
        ``DISPLAY_NAME == "Sampler"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has seven specs (including
        ``clip``), and ``OUTPUT_SLOTS`` has two specs.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import Sampler

    assert Sampler.NODE_TYPE == "Sampler"
    assert Sampler.CATEGORY == "Sampling"
    assert Sampler.DISPLAY_NAME == "Sampler"
    assert isinstance(Sampler.DESCRIPTION, str)
    assert len(Sampler.DESCRIPTION) > 0

    # Verify INPUT_SLOTS — seven specs: model, conditioning, clip,
    # latent, steps, cfg, seed.
    assert len(Sampler.INPUT_SLOTS) == 7

    model_spec = Sampler.INPUT_SLOTS[0]
    assert isinstance(model_spec, SlotSpec)
    assert model_spec.name == "model"
    assert model_spec.slot_type == "MODEL"
    assert model_spec.optional is False

    cond_spec = Sampler.INPUT_SLOTS[1]
    assert isinstance(cond_spec, SlotSpec)
    assert cond_spec.name == "conditioning"
    assert cond_spec.slot_type == "CONDITIONING"
    assert cond_spec.optional is False

    clip_spec = Sampler.INPUT_SLOTS[2]
    assert isinstance(clip_spec, SlotSpec)
    assert clip_spec.name == "clip"
    assert clip_spec.slot_type == "CLIP"
    assert clip_spec.optional is False

    latent_spec = Sampler.INPUT_SLOTS[3]
    assert isinstance(latent_spec, SlotSpec)
    assert latent_spec.name == "latent"
    assert latent_spec.slot_type == "LATENT"
    assert latent_spec.optional is False

    steps_spec = Sampler.INPUT_SLOTS[4]
    assert isinstance(steps_spec, SlotSpec)
    assert steps_spec.name == "steps"
    assert steps_spec.slot_type == "INT"
    assert steps_spec.optional is False

    cfg_spec = Sampler.INPUT_SLOTS[5]
    assert isinstance(cfg_spec, SlotSpec)
    assert cfg_spec.name == "cfg"
    assert cfg_spec.slot_type == "FLOAT"
    assert cfg_spec.optional is False

    seed_spec = Sampler.INPUT_SLOTS[6]
    assert isinstance(seed_spec, SlotSpec)
    assert seed_spec.name == "seed"
    assert seed_spec.slot_type == "INT"
    assert seed_spec.optional is False

    # Verify OUTPUT_SLOTS — two specs: latent, seed.
    assert len(Sampler.OUTPUT_SLOTS) == 2

    output_latent = Sampler.OUTPUT_SLOTS[0]
    assert isinstance(output_latent, SlotSpec)
    assert output_latent.name == "latent"
    assert output_latent.slot_type == "LATENT"
    assert output_latent.optional is False

    output_seed = Sampler.OUTPUT_SLOTS[1]
    assert isinstance(output_seed, SlotSpec)
    assert output_seed.name == "seed"
    assert output_seed.slot_type == "INT"
    assert output_seed.optional is False

    # Verify EMITS_PROGRESS.
    assert Sampler.EMITS_PROGRESS is True


def test_emptylatent_metadata_attributes() -> None:
    """Verify all six required metadata attributes on ``EmptyLatent``.

    Preconditions:
        ``EmptyLatent`` class is accessible via direct import from
        ``worker.nodes.sampler``.

    Tests:
        Assert each of the six required metadata attributes has the
        correct value and type. Also verify the INPUT_SLOTS and
        OUTPUT_SLOTS structure.

    Expected output:
        ``NODE_TYPE == "EmptyLatent"``, ``CATEGORY == "Latents"``,
        ``DISPLAY_NAME == "Empty Latent"``, ``DESCRIPTION`` is a
        non-empty string, ``INPUT_SLOTS`` has four specs (width,
        height, batch_size, model), and ``OUTPUT_SLOTS`` has one spec.
    """
    import worker.nodes.sampler

    importlib.reload(worker.nodes.sampler)
    from worker.nodes.sampler import EmptyLatent

    assert EmptyLatent.NODE_TYPE == "EmptyLatent"
    assert EmptyLatent.CATEGORY == "Latents"
    assert EmptyLatent.DISPLAY_NAME == "Empty Latent"
    assert isinstance(EmptyLatent.DESCRIPTION, str)
    assert len(EmptyLatent.DESCRIPTION) > 0

    # Verify INPUT_SLOTS — four specs: width (required), height
    # (required), batch_size (optional), model (optional).
    assert len(EmptyLatent.INPUT_SLOTS) == 4

    width_spec = EmptyLatent.INPUT_SLOTS[0]
    assert isinstance(width_spec, SlotSpec)
    assert width_spec.name == "width"
    assert width_spec.slot_type == "INT"
    assert width_spec.optional is False

    height_spec = EmptyLatent.INPUT_SLOTS[1]
    assert isinstance(height_spec, SlotSpec)
    assert height_spec.name == "height"
    assert height_spec.slot_type == "INT"
    assert height_spec.optional is False

    batch_spec = EmptyLatent.INPUT_SLOTS[2]
    assert isinstance(batch_spec, SlotSpec)
    assert batch_spec.name == "batch_size"
    assert batch_spec.slot_type == "INT"
    assert batch_spec.optional is True

    # The 4th slot is the optional model input for real-mode
    # architecture dispatch. It is optional so existing job graphs
    # without it still pass registration.
    model_spec = EmptyLatent.INPUT_SLOTS[3]
    assert isinstance(model_spec, SlotSpec)
    assert model_spec.name == "model"
    assert model_spec.slot_type == "MODEL"
    assert model_spec.optional is True

    # Verify OUTPUT_SLOTS.
    assert len(EmptyLatent.OUTPUT_SLOTS) == 1
    output_spec = EmptyLatent.OUTPUT_SLOTS[0]
    assert isinstance(output_spec, SlotSpec)
    assert output_spec.name == "latent"
    assert output_spec.slot_type == "LATENT"
    assert output_spec.optional is False
