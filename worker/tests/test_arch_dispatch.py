"""Tests for worker.nodes.arch.diffusion, clip, and vae — get_module dispatcher."""

from types import ModuleType
from unittest.mock import Mock

import pytest

from worker.nodes.arch import clip
from worker.nodes.arch import diffusion
from worker.nodes.arch import vae


@pytest.fixture(autouse=True)
def _clear_diffusion_registry() -> None:
    """Clear the diffusion registry before each test.

    The zit module is registered at import time (P20-B2), so tests that
    assert "empty registry" behaviour must temporarily remove it.
    """
    # Save the zit module so we can restore it after the test.
    _zit = diffusion.zit  # type: ignore[attr-defined]
    diffusion._REGISTERED_MODULES.clear()
    yield
    # Restore zit so other tests see the correct initial state.
    diffusion._REGISTERED_MODULES.append(_zit)


def test_get_module_returns_none_when_empty() -> None:
    """get_module returns None when _REGISTERED_MODULES is empty.

    With zero registered modules (after clearing zit), get_module("zit")
    must return None without raising — the empty registry is the default
    state before concrete arch modules are wired in later phases.
    """
    result = diffusion.get_module("zit")
    assert result is None


def test_get_module_does_not_raise_for_various_key_types() -> None:
    """get_module does not raise for str, None, or arbitrary object keys.

    With an empty registry, get_module must handle any key type without
    raising — the dispatch loop should never throw, even for edge-case
    keys that no module would ever match.
    """
    # String key — the normal case.
    assert diffusion.get_module("zit") is None

    # None key — some callers may pass None as a fallback.
    assert diffusion.get_module(None) is None

    # Arbitrary object key — must not raise.
    obj = object()
    assert diffusion.get_module(obj) is None


def test_get_module_skips_module_with_can_handle_false() -> None:
    """get_module skips a module whose can_handle returns False.

    Registers a test double whose can_handle(key) returns False, then
    calls get_module("zit") and asserts it returns None — proving the
    dispatcher continues scanning rather than returning a non-matching
    module.
    """
    # Create a module-like object with a can_handle that always returns False.
    fake_module = Mock(spec=ModuleType)
    fake_module.can_handle = Mock(return_value=False)

    # Register the fake module.
    diffusion._REGISTERED_MODULES.append(fake_module)
    try:
        result = diffusion.get_module("zit")
        # can_handle must have been called at least once.
        fake_module.can_handle.assert_called_once_with("zit")
        # Since can_handle returned False, the dispatcher should return None.
        assert result is None
    finally:
        # Always clean up: remove the fake module so subsequent tests see
        # an empty registry.
        diffusion._REGISTERED_MODULES.remove(fake_module)


# ---------------------------------------------------------------------------
# CLIP dispatcher tests
# ---------------------------------------------------------------------------


def test_clip_get_module_returns_none_when_empty() -> None:
    """clip.get_module returns None for a key no registered module handles.

    With qwen3 registered, clip.get_module("unknown") must return None
    without raising — the dispatcher returns None when no module's
    can_handle() matches the key.
    """
    result = clip.get_module("unknown")
    assert result is None


def test_clip_get_module_does_not_raise_for_various_key_types() -> None:
    """clip.get_module does not raise for str, None, or arbitrary object keys.

    clip.get_module must handle any key type without raising — the dispatch
    loop should never throw, even for edge-case keys that no module would
    ever match.
    """
    # String key — the normal case.
    assert clip.get_module("unknown") is None

    # None key — some callers may pass None as a fallback.
    assert clip.get_module(None) is None

    # Arbitrary object key — must not raise.
    obj = object()
    assert clip.get_module(obj) is None


def test_clip_get_module_skips_module_with_can_handle_false() -> None:
    """clip dispatcher skips a module whose can_handle returns False.

    Registers a test double whose can_handle(key) returns False, then
    calls clip.get_module("unknown") and asserts it returns None — proving the
    dispatcher continues scanning rather than returning a non-matching
    module. Uses "unknown" because qwen3 already handles "qwen3", so
    the fake module would never be reached with that key.
    """
    # Create a module-like object with a can_handle that always returns False.
    fake_module = Mock(spec=ModuleType)
    fake_module.can_handle = Mock(return_value=False)

    # Register the fake module.
    clip._REGISTERED_MODULES.append(fake_module)
    try:
        result = clip.get_module("unknown")
        # can_handle must have been called at least once.
        fake_module.can_handle.assert_called_once_with("unknown")
        # Since can_handle returned False, the dispatcher should return None.
        assert result is None
    finally:
        # Always clean up: remove the fake module so subsequent tests see
        # the original registry state.
        clip._REGISTERED_MODULES.remove(fake_module)


# ---------------------------------------------------------------------------
# VAE dispatcher tests
# ---------------------------------------------------------------------------


def test_vae_get_module_returns_none_when_empty() -> None:
    """vae.get_module returns None when _REGISTERED_MODULES is empty.

    With zero registered modules, vae.get_module("zit_vae") must return None
    without raising — the empty registry is the default state before
    concrete arch modules are wired in later phases.
    """
    result = vae.get_module("zit_vae")
    assert result is None


def test_vae_get_module_does_not_raise_for_various_key_types() -> None:
    """vae.get_module does not raise for str, None, or arbitrary object keys.

    With an empty registry, vae.get_module must handle any key type without
    raising — the dispatch loop should never throw, even for edge-case
    keys that no module would ever match.
    """
    # String key — the normal case.
    assert vae.get_module("zit_vae") is None

    # None key — some callers may pass None as a fallback.
    assert vae.get_module(None) is None

    # Arbitrary object key — must not raise.
    obj = object()
    assert vae.get_module(obj) is None


def test_vae_get_module_skips_module_with_can_handle_false() -> None:
    """vae dispatcher skips a module whose can_handle returns False.

    Registers a test double whose can_handle(key) returns False, then
    calls vae.get_module("zit_vae") and asserts it returns None — proving the
    dispatcher continues scanning rather than returning a non-matching
    module.
    """
    # Create a module-like object with a can_handle that always returns False.
    fake_module = Mock(spec=ModuleType)
    fake_module.can_handle = Mock(return_value=False)

    # Register the fake module.
    vae._REGISTERED_MODULES.append(fake_module)
    try:
        result = vae.get_module("zit_vae")
        # can_handle must have been called at least once.
        fake_module.can_handle.assert_called_once_with("zit_vae")
        # Since can_handle returned False, the dispatcher should return None.
        assert result is None
    finally:
        # Always clean up: remove the fake module so subsequent tests see
        # an empty registry.
        vae._REGISTERED_MODULES.remove(fake_module)
