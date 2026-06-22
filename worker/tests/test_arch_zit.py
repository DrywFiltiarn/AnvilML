"""Unit tests for the ZiT architecture dispatch module.

Tests cover ``can_handle()`` dispatching for ZiT and non-ZiT models,
the mock ``sample()`` path returning ``MockLatent`` with the correct
seed, and import isolation (no torch import at module load time).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import os
import sys
from typing import Any

import pytest

from worker.nodes.arch.zit import (
    MockLatent,
    VAE_SCALE_FACTOR,
    can_handle,
    compute_latent_shape,
    sample,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_model(arch: str | None = "zit") -> Any:
    """Build a minimal model object with an ``arch`` attribute.

    Uses a simple namespace object with the ``arch`` attribute set
    to the given value. If ``arch`` is ``None``, the model object
    is constructed without an ``arch`` attribute to test the
    missing-attribute case.

    Args:
        arch: The architecture string to set, or ``None`` to omit
            the attribute entirely.

    Returns:
        A namespace object with an ``arch`` attribute (or none).
    """
    if arch is None:
        # Create a model object without an arch attribute.
        # This tests the getattr fallback in can_handle().
        return type("Model", (), {})()
    return type("Model", (), {"arch": arch})()


# ---------------------------------------------------------------------------
# Tests: VAE_SCALE_FACTOR
# ---------------------------------------------------------------------------


def test_vae_scale_factor_value() -> None:
    """Verify ``VAE_SCALE_FACTOR`` module constant equals ``8``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        reading a module-level constant, but consistent with the test file
        convention).

    Tests:
        Import ``VAE_SCALE_FACTOR`` from the module under test and
        assert it equals ``8``.

    Expected output:
        ``VAE_SCALE_FACTOR == 8`` — the Z-Image-Turbo VAE spatial
        compression factor matches the published config.
    """
    assert VAE_SCALE_FACTOR == 8


# ---------------------------------------------------------------------------
# Tests: can_handle
# ---------------------------------------------------------------------------


def test_can_handle_zit() -> None:
    """Verify ``can_handle()`` returns ``True`` for a ZiT model.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model object with ``arch == "zit"``, pass it to
        ``can_handle()``, and assert the result is ``True``.

    Expected output:
        ``can_handle(model) == True`` — the ZiT arch module claims
        this model.
    """
    model = _make_model("zit")
    assert can_handle(model) is True


def test_can_handle_non_zit() -> None:
    """Verify ``can_handle()`` returns ``False`` for non-ZiT models.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model with ``arch == "flux"`` and pass it to
        ``can_handle()``, assert ``False``. Then construct a model
        without an ``arch`` attribute and assert ``False`` again.

    Expected output:
        ``can_handle(flux_model) == False`` and
        ``can_handle(no_arch_model) == False`` — the ZiT arch module
        does not claim these models.
    """
    # Test with a non-ZiT architecture string.
    flux_model = _make_model("flux")
    assert can_handle(flux_model) is False

    # Test with a model that has no arch attribute at all.
    no_arch_model = _make_model(None)
    assert can_handle(no_arch_model) is False


# ---------------------------------------------------------------------------
# Tests: sample (mock mode)
# ---------------------------------------------------------------------------


def test_sample_mock_returns_mock_latent_and_seed() -> None:
    """Verify ``sample()`` returns ``(MockLatent(), seed)`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.

    Tests:
        Call ``sample()`` with ``seed=42`` and all other arguments
        as ``None`` or empty, and assert the returned tuple contains
        a ``MockLatent`` sentinel and the correct seed value.

    Expected output:
        ``result[0]`` is a ``MockLatent`` instance and
        ``result[1] == 42``.
    """
    result = sample(
        model=None,
        conditioning=None,
        latent=None,
        steps=4,
        cfg=7.0,
        seed=42,
        device="cpu",
        cancel_flag=[False],
        emit_progress=lambda step, total: None,
    )

    assert isinstance(result[0], MockLatent)
    assert result[1] == 42


def test_sample_mock_preserves_seed_value() -> None:
    """Verify ``sample()`` returns the exact seed passed in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.

    Tests:
        Call ``sample()`` with several different seed values (0, 1,
        2**32 - 1) and assert each one is returned unchanged.

    Expected output:
        The seed value in the result tuple matches the input exactly
        for each test case.
    """
    for test_seed in (0, 1, 2**32 - 1, 12345):
        result = sample(
            model=None,
            conditioning=None,
            latent=None,
            steps=4,
            cfg=7.0,
            seed=test_seed,
            device="cpu",
            cancel_flag=[False],
            emit_progress=lambda step, total: None,
        )

        assert result[1] == test_seed


def test_sample_real_path_raises_not_implemented() -> None:
    """Verify ``sample()`` raises ``NotImplementedError`` in real mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK`` is temporarily set to ``"0"`` by this
        test, overriding the autouse fixture.

    Tests:
        Call ``sample()`` with ``ANVILML_WORKER_MOCK=0`` and assert
        a ``NotImplementedError`` is raised with the expected message.

    Expected output:
        ``NotImplementedError`` with message containing
        "Real ZiT sampling path not yet implemented".
    """
    # Capture the pre-existing value and force real mode.
    original = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        with pytest.raises(NotImplementedError, match="Real ZiT sampling path"):
            sample(
                model=None,
                conditioning=None,
                latent=None,
                steps=4,
                cfg=7.0,
                seed=42,
                device="cpu",
                cancel_flag=[False],
                emit_progress=lambda step, total: None,
            )
    finally:
        # Restore the original value unconditionally.
        if original is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original


# ---------------------------------------------------------------------------
# Tests: import isolation
# ---------------------------------------------------------------------------


def test_sample_mock_no_torch_import() -> None:
    """Verify the module imports cleanly without torch in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture.

    Tests:
        Remove ``torch`` from ``sys.modules`` (if present) and
        re-import the ``worker.nodes.arch.zit`` module. Assert that
        no ``ImportError`` is raised and that ``torch`` is not in
        ``sys.modules`` after import — proving no top-level import
        of torch occurs.

    Expected output:
        Module imports successfully and ``"torch"`` is absent from
        ``sys.modules``, confirming mock-mode import isolation.
    """
    # Remove torch from sys.modules to simulate an environment
    # where torch is not installed. This ensures the import
    # succeeds even without the package available.
    torch_was_present = "torch" in sys.modules
    torch_saved = sys.modules.pop("torch", None)

    # Also remove the module from sys.modules cache so we get a
    # fresh import that exercises the full module body.
    sys.modules.pop("worker.nodes.arch.zit", None)

    # Also remove the parent arch package from cache.
    sys.modules.pop("worker.nodes.arch", None)

    try:
        # Import must succeed — if torch were imported at module level,
        # this would raise ImportError since we just removed it.
        import worker.nodes.arch.zit as zit_mod

        importlib.reload(zit_mod)

        # Verify torch is still absent from sys.modules after import.
        assert "torch" not in sys.modules, (
            "torch was imported at module level — "
            "this breaks mock-mode isolation"
        )

        # Verify the module's public API is intact.
        assert callable(zit_mod.can_handle)
        assert callable(zit_mod.sample)
        assert zit_mod.MockLatent is not None
    finally:
        # Restore torch if it was present before.
        if torch_was_present and torch_saved is not None:
            sys.modules["torch"] = torch_saved
        # Restore cached modules for other tests.
        sys.modules.pop("worker.nodes.arch.zit", None)
        sys.modules.pop("worker.nodes.arch", None)


# ---------------------------------------------------------------------------
# Tests: compute_latent_shape
# ---------------------------------------------------------------------------


def test_compute_latent_shape_known_dims() -> None:
    """Verify ``compute_latent_shape()`` produces the canonical ZiT latent shape.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        a pure arithmetic function, but consistent with the test file
        convention).

    Tests:
        Call ``compute_latent_shape(1, 1024, 1024, 4)`` and assert
        the result equals ``(1, 4, 128, 128)``. This is the canonical
        ZiT case: 1024×1024 image → 128×128 latent (8× spatial
        compression), batch 1, 4 channels (standard SD-style).

    Expected output:
        ``compute_latent_shape(1, 1024, 1024, 4) == (1, 4, 128, 128)``.
    """
    result = compute_latent_shape(1, 1024, 1024, 4)
    assert result == (1, 4, 128, 128)


def test_compute_latent_shape_non_divisible() -> None:
    """Verify ``compute_latent_shape()`` silently floors non-divisible dimensions.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``compute_latent_shape(2, 1025, 1026, 4)`` and assert
        the result equals ``(2, 4, 128, 128)``. The floor division
        ``1025 // 16 == 64`` and ``1026 // 16 == 64``, so
        ``h == w == 128`` — this verifies that non-divisible
        dimensions silently floor rather than raise, matching
        ``ZImagePipeline.prepare_latents``'s integer-division behaviour.

    Expected output:
        ``compute_latent_shape(2, 1025, 1026, 4) == (2, 4, 128, 128)``.
    """
    result = compute_latent_shape(2, 1025, 1026, 4)
    assert result == (2, 4, 128, 128)
