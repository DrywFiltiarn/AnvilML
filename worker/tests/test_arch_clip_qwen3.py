"""Unit tests for the Qwen3 CLIP architecture dispatch module.

Tests cover ``can_handle()`` dispatching for Qwen3 and non-Qwen3
clip types, the mock ``load()`` path returning ``RealClip`` with
``MockTokenizer`` and ``MockTextEncoder``, and import isolation
(no torch import at module load time).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import subprocess
import sys
from typing import Any

import pytest

from worker.nodes.arch.clip.qwen3 import (
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


def test_can_handle_qwen3() -> None:
    """Verify ``can_handle("qwen3")`` returns ``True``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        a pure string comparison, but consistent with the test file
        convention).

    Tests:
        Call ``can_handle("qwen3")`` and assert the result is ``True``.

    Expected output:
        ``can_handle("qwen3") == True`` — the Qwen3 arch module claims
        this clip type.
    """
    assert can_handle("qwen3") is True


def test_can_handle_non_qwen3() -> None:
    """Verify ``can_handle()`` returns ``False`` for non-Qwen3 clip types.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``can_handle("clip_l")`` and ``can_handle("t5")``, assert
        both return ``False``.

    Expected output:
        ``can_handle("clip_l") == False`` and
        ``can_handle("t5") == False`` — the Qwen3 arch module does not
        claim these clip types.
    """
    assert can_handle("clip_l") is False
    assert can_handle("t5") is False


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
        Spawn a fresh child Python process (where ``torch`` has never
        been loaded at all) and import the ``qwen3`` module in it.
        Assert the child exits 0 and reports that ``torch`` is still
        absent from its own ``sys.modules`` after the import — proving
        no top-level import of torch occurs.

        This runs in a subprocess rather than popping ``torch`` out of
        the live test process's ``sys.modules`` and calling
        ``importlib.reload()`` on the project module — that combination
        is unsafe against an already-natively-initialized ``torch``
        (OpenMP/MKL thread pools, C-extension static state) and can
        fault at the native level, outside any Python exception. A
        confirmed real incident crashed a project owner's WSL2 VM
        running exactly that pattern.

    Expected output:
        The child process exits 0 and prints ``False`` for
        ``"torch" in sys.modules``, confirming mock-mode import
        isolation without touching the parent process's own torch
        state.
    """
    script = (
        "import worker.nodes.arch.clip.qwen3; "
        "import sys; "
        "print('torch' in sys.modules)"
    )
    result = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert result.returncode == 0, (
        f"child process failed to import the module: {result.stderr}"
    )
    assert result.stdout.strip() == "False", (
        "torch was imported at module level — "
        "this breaks mock-mode isolation"
    )
