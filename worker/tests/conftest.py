"""Shared pytest fixtures for worker tests.

All tests in this directory run with mock mode enabled so that
no real hardware backend (torch) is required.
"""

from __future__ import annotations

import os

import pytest


@pytest.fixture(autouse=True)
def mock_mode() -> None:
    """Enable mock mode for every test by setting ``ANVILML_WORKER_MOCK=1``.

    Captures the pre-existing value of the environment variable (or
    ``None`` if absent), sets it to ``"1"``, and restores the original
    value unconditionally in a ``finally`` block so that no test
    leaks state into the next.
    """
    original = os.environ.get("ANVILML_WORKER_MOCK")  # None if absent
    os.environ["ANVILML_WORKER_MOCK"] = "1"
    try:
        yield
    finally:
        if original is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original
