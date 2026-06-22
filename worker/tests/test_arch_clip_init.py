"""Unit tests for the CLIP architecture dispatch module.

Tests cover ``get_module()`` and ``can_handle()`` dispatching using
a temporary ``_test_dummy`` sibling module that is installed into
the real package directory during test execution.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Iterator

import pytest

from worker.nodes.arch.clip import can_handle, get_module

# Path to the real clip package directory on disk.
_CLIP_PKG_DIR = Path(__file__).parent.parent / "nodes" / "arch" / "clip"
# Path to the dummy module in the real package directory.
_DUMMY_MOD = _CLIP_PKG_DIR / "_test_dummy.py"

# The dummy module source content, written inline so the fixture
# can re-create it for each test without depending on a source file.
_DUMMY_CONTENT = '''"""Temporary dummy CLIP architecture module for testing the dispatcher.

This module is a test artifact only — it will be removed before commit.
It exists solely to prove that ``get_module()`` and ``can_handle()``
iterate over sibling modules and correctly match a ``can_handle()`` handler.

.. versionadded:: 0.1.0
"""

from __future__ import annotations


def can_handle(clip_type: str) -> bool:
    """Return ``True`` when *clip_type* matches the dummy identifier.

    Args:
        clip_type: The clip type string to check.

    Returns:
        ``True`` if ``clip_type == "_dummy"``, ``False`` otherwise.
    """
    return clip_type == "_dummy"
'''


@pytest.fixture(autouse=True)
def _install_test_dummy() -> Iterator[None]:
    """Install the ``_test_dummy`` module into the real clip package.

    Writes ``_test_dummy.py`` into the real ``clip/`` directory before
    each test, then removes it after the test completes. This ensures
    ``get_module()`` and ``can_handle()`` can discover the dummy module
    via ``pkgutil.iter_modules(__path__)``.

    Also clears the ``worker.nodes.arch.clip`` module cache so that
    ``get_module()`` re-discovers the newly installed module.
    """
    # Write the dummy module source into the real package directory.
    _DUMMY_MOD.write_text(_DUMMY_CONTENT)

    # Clear cached clip modules so get_module() re-discovers them.
    modules_to_clear = [
        k for k in sys.modules if k.startswith("worker.nodes.arch.clip")
    ]
    for mod_name in modules_to_clear:
        sys.modules.pop(mod_name, None)

    try:
        yield
    finally:
        # Remove the dummy module from the real package directory.
        if _DUMMY_MOD.exists():
            _DUMMY_MOD.unlink()

        # Restore the module cache.
        for mod_name in modules_to_clear:
            sys.modules.pop(mod_name, None)


# ---------------------------------------------------------------------------
# Tests: get_module
# ---------------------------------------------------------------------------


def test_get_module_returns_dummy_for_dummy_clip_type() -> None:
    """Verify ``get_module()`` returns the ``_test_dummy`` module for ``"_dummy"``.

    Preconditions:
        The ``_test_dummy`` module is installed into the real clip
        package directory by the ``_install_test_dummy`` fixture,
        and ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py``
        autouse fixture.

    Tests:
        Call ``get_module("_dummy")`` and assert the returned module's
        ``__name__`` is ``"worker.nodes.arch.clip._test_dummy"``.
        This proves ``get_module()`` iterates modules, imports them,
        calls ``can_handle()``, and returns the correct module.

    Expected output:
        ``result.__name__ == "worker.nodes.arch.clip._test_dummy"``
        — the dispatcher found and returned the dummy module.
    """
    result = get_module("_dummy")
    assert result is not None
    assert result.__name__ == "worker.nodes.arch.clip._test_dummy"


def test_get_module_returns_none_for_unknown_clip_type() -> None:
    """Verify ``get_module()`` returns ``None`` for a nonexistent clip type.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture. The ``_test_dummy`` module is present in the clip
        package namespace during test execution (installed by the
        ``_install_test_dummy`` fixture).

    Tests:
        Call ``get_module("nonexistent")`` and assert ``None`` is
        returned. This proves the function correctly returns ``None``
        when no module's ``can_handle()`` matches.

    Expected output:
        ``get_module("nonexistent") is None`` — no clip arch module
        claims this unknown clip type.
    """
    result = get_module("nonexistent")
    assert result is None


# ---------------------------------------------------------------------------
# Tests: can_handle
# ---------------------------------------------------------------------------


def test_can_handle_returns_bools_correctly() -> None:
    """Verify ``can_handle()`` returns correct boolean values.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture. The ``_test_dummy`` module is present in the clip
        package namespace during test execution (installed by the
        ``_install_test_dummy`` fixture).

    Tests:
        Call ``can_handle("_dummy")`` (expect ``True``) and
        ``can_handle("nonexistent")`` (expect ``False``).
        This proves the delegation to ``get_module()`` works correctly.

    Expected output:
        ``can_handle("_dummy") == True`` and
        ``can_handle("nonexistent") == False`` — the dispatcher
        correctly identifies known and unknown clip types.
    """
    assert can_handle("_dummy") is True
    assert can_handle("nonexistent") is False
