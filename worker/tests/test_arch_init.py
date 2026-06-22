"""Unit tests for the architecture dispatch registry (``arch/__init__.py``).

Tests cover ``get_module()`` returning the correct arch module for known
architectures, ``None`` for unknown architectures, and ``can_handle()``
delegating correctly to ``get_module()`` after the refactoring.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import sys
from typing import Any

import pytest

from worker.nodes.arch import can_handle, get_module


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
        return type("Model", (), {})()
    return type("Model", (), {"arch": arch})()


# ---------------------------------------------------------------------------
# Tests: get_module
# ---------------------------------------------------------------------------


def test_get_module_returns_zit_for_zit_model() -> None:
    """Verify ``get_module()`` returns the zit module for a ZiT model.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model object with ``arch == "zit"``, pass it to
        ``get_module()``, and assert the returned module is the zit
        arch module (checked via ``mod.__name__``).

    Expected output:
        ``mod.__name__ == "worker.nodes.arch.zit"`` — the zit arch
        module is returned for a ZiT model.
    """
    model = _make_model("zit")
    mod = get_module(model)

    assert mod is not None, "get_module() should return a module for zit model"
    assert mod.__name__ == "worker.nodes.arch.zit"


def test_get_module_returns_none_for_unknown_arch() -> None:
    """Verify ``get_module()`` returns ``None`` for an unknown architecture.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model with ``arch == "unknown"`` (no arch module
        handles this), pass it to ``get_module()``, and assert ``None``.

    Expected output:
        ``result is None`` — no arch module claims an unknown architecture.
    """
    model = _make_model("unknown")
    result = get_module(model)

    assert result is None, "get_module() should return None for unknown arch"


# ---------------------------------------------------------------------------
# Tests: can_handle delegation
# ---------------------------------------------------------------------------


def test_can_handle_still_works_after_refactor() -> None:
    """Verify ``can_handle()`` returns correct bools post-refactor.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``can_handle()`` with ``arch="zit"`` (expect ``True``)
        and ``arch="unknown"`` (expect ``False``), verifying the
        delegation to ``get_module()`` works correctly.

    Expected output:
        ``can_handle(zit_model) == True`` and
        ``can_handle(unknown_model) == False`` — identical results
        to the original pre-refactor implementation.
    """
    zit_model = _make_model("zit")
    unknown_model = _make_model("unknown")

    assert can_handle(zit_model) is True, (
        "can_handle() should return True for zit model"
    )
    assert can_handle(unknown_model) is False, (
        "can_handle() should return False for unknown model"
    )
