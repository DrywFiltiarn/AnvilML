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
from worker.nodes.arch.diffusion import get_module_by_name


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
        ``mod.__name__ == "worker.nodes.arch.diffusion.zit"`` — the zit arch
        module is returned for a ZiT model.
    """
    model = _make_model("zit")
    mod = get_module(model)

    assert mod is not None, "get_module() should return a module for zit model"
    assert mod.__name__ == "worker.nodes.arch.diffusion.zit"


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


# ---------------------------------------------------------------------------
# Tests: get_module_by_name
# ---------------------------------------------------------------------------


def test_get_module_by_name_returns_zit_for_zit() -> None:
    """Verify ``get_module_by_name("zit")`` returns the zit arch module.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``get_module_by_name("zit")`` and assert the returned
        module is not ``None`` and its ``__name__`` identifies the
        zit arch module.

    Expected output:
        ``mod.__name__ == "worker.nodes.arch.diffusion.zit"`` — the
        zit arch module is returned for the "zit" architecture string.
    """
    mod = get_module_by_name("zit")

    assert mod is not None, (
        "get_module_by_name('zit') should return the zit module"
    )
    assert mod.__name__ == "worker.nodes.arch.diffusion.zit"


def test_get_module_by_name_returns_none_for_unknown_arch() -> None:
    """Verify ``get_module_by_name("unknown")`` returns ``None``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``get_module_by_name("unknown")`` (no arch module handles
        this architecture) and assert the result is ``None``.

    Expected output:
        ``result is None`` — no loaded arch module claims an unknown
        architecture string.
    """
    result = get_module_by_name("unknown")

    assert result is None, (
        "get_module_by_name('unknown') should return None"
    )


def test_get_module_by_name_shim_pattern() -> None:
    """Verify the shim object pattern satisfies ``can_handle()``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``get_module_by_name("zit")`` and confirm it returns the
        same module as ``get_module(_make_model("zit"))``, proving
        the shim class (which carries only ``arch = "zit"``) is
        functionally equivalent to a full model object for dispatch
        purposes.  The shim is a bare class with no model-like
        attributes — it relies on ``can_handle()`` reading only the
        ``arch`` attribute via ``getattr()``.

    Expected output:
        Both calls return the same module, confirming the shim pattern
        works correctly without constructing a real model object.
    """
    shim_result = get_module_by_name("zit")
    model_result = get_module(_make_model("zit"))

    assert shim_result is not None, (
        "get_module_by_name('zit') should return a module"
    )
    assert shim_result is model_result, (
        "shim and model object should resolve to the same module"
    )
