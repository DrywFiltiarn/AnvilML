"""Unit tests for the T5-XXL CLIP architecture dispatch module.

Tests cover ``can_handle()`` dispatching for T5 and non-T5
clip types, the mock ``load()`` path returning ``RealClip`` with
``MockTokenizer`` and ``MockTextEncoder``, and import isolation
(no torch import at module load time).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import sys
from typing import Any

import pytest

from worker.nodes.arch.clip.t5 import (
    can_handle,
    load,
)
from worker.nodes.loader import (
    MockTokenizer,
    MockTextEncoder,
    RealClip,
)


# ---------------------------------------------------------------------------
# Tests: can_handle
# ---------------------------------------------------------------------------


def test_can_handle_t5() -> None:
    """Verify ``can_handle("t5")`` returns ``True``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        a pure string comparison, but consistent with the test file
        convention).

    Tests:
        Call ``can_handle("t5")`` and assert the result is ``True``.

    Expected output:
        ``can_handle("t5") == True`` — the T5 arch module claims
        this clip type.
    """
    assert can_handle("t5") is True


def test_can_handle_non_t5() -> None:
    """Verify ``can_handle()`` returns ``False`` for non-T5 clip types.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``can_handle("qwen3")`` and ``can_handle("clip_l")``, assert
        both return ``False``.

    Expected output:
        ``can_handle("qwen3") == False`` and
        ``can_handle("clip_l") == False`` — the T5 arch module does not
        claim these clip types.
    """
    assert can_handle("qwen3") is False
    assert can_handle("clip_l") is False


# ---------------------------------------------------------------------------
# Tests: load (mock mode)
# ---------------------------------------------------------------------------


def test_load_mock_returns_realclip() -> None:
    """Verify ``load()`` returns ``RealClip`` with sentinel objects in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.

    Tests:
        Call ``load("/fake/path", None)`` and assert the result is a
        ``RealClip`` instance whose ``.tokenizer`` is a ``MockTokenizer``
        and ``.text_encoder`` is a ``MockTextEncoder``.

    Expected output:
        ``isinstance(result, RealClip)`` is ``True``,
        ``isinstance(result.tokenizer, MockTokenizer)`` is ``True``,
        and ``isinstance(result.text_encoder, MockTextEncoder)`` is ``True``.
    """
    result = load("/fake/path", None)

    assert isinstance(result, RealClip)
    assert isinstance(result.tokenizer, MockTokenizer)
    assert isinstance(result.text_encoder, MockTextEncoder)


# ---------------------------------------------------------------------------
# Tests: import isolation
# ---------------------------------------------------------------------------


def test_load_mock_no_torch_import() -> None:
    """Verify the module imports cleanly without torch in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture.

    Tests:
        Remove ``torch`` from ``sys.modules`` (if present), remove the
        module from cache, and re-import the ``t5`` module. Assert
        that no ``ImportError`` is raised and that ``torch`` is not in
        ``sys.modules`` after import — proving no top-level import of
        torch occurs.

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
    sys.modules.pop("worker.nodes.arch.clip.t5", None)

    # Also remove the parent arch clip package from cache.
    sys.modules.pop("worker.nodes.arch.clip", None)

    try:
        # Import must succeed — if torch were imported at module level,
        # this would raise ImportError since we just removed it.
        import worker.nodes.arch.clip.t5 as t5_mod

        importlib.reload(t5_mod)

        # Verify torch is still absent from sys.modules after import.
        assert "torch" not in sys.modules, (
            "torch was imported at module level — "
            "this breaks mock-mode isolation"
        )

        # Verify the module's public API is intact.
        assert callable(t5_mod.can_handle)
        assert callable(t5_mod.load)
    finally:
        # Restore torch if it was present before.
        if torch_was_present and torch_saved is not None:
            sys.modules["torch"] = torch_saved
        # Restore cached modules for other tests.
        sys.modules.pop("worker.nodes.arch.clip.t5", None)
        sys.modules.pop("worker.nodes.arch.clip", None)
