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

    The zit and flux2klein modules are registered at import time
    (P20-B2 and P25-B2), so tests that assert "empty registry"
    behaviour must temporarily remove them.
    """
    # Save both registered modules so we can restore them after the test.
    _modules = list(diffusion._REGISTERED_MODULES)
    diffusion._REGISTERED_MODULES.clear()
    yield
    # Restore both modules so other tests see the correct initial state.
    diffusion._REGISTERED_MODULES.extend(_modules)


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


def test_vae_get_module_returns_zit_vae_when_registered() -> None:
    """vae.get_module returns the zit_vae module after registration.

    With zit_vae registered in _REGISTERED_MODULES, vae.get_module("zit_vae")
    must return the zit_vae module — proving the dispatch registration
    (P23-B2) wired the module correctly.
    """
    result = vae.get_module("zit_vae")
    assert result is not None
    assert result.__name__ == "worker.nodes.arch.vae.zit_vae"


def test_vae_get_module_does_not_raise_for_various_key_types() -> None:
    """vae.get_module does not raise for str, None, or arbitrary object keys.

    With zit_vae registered, vae.get_module("zit_vae") returns the module,
    while unrecognised keys (None, arbitrary object) return None without
    raising — the dispatch loop should never throw, even for edge-case
    keys that no module would ever match.
    """
    # String key that matches — returns the registered module.
    result = vae.get_module("zit_vae")
    assert result is not None

    # None key — some callers may pass None as a fallback.
    assert vae.get_module(None) is None

    # Arbitrary object key — must not raise.
    obj = object()
    assert vae.get_module(obj) is None


def test_vae_get_module_skips_module_with_can_handle_false() -> None:
    """vae dispatcher skips a module whose can_handle returns False.

    Registers a test double whose can_handle(key) returns False for the key
    "flux2_vae", then calls vae.get_module("flux2_vae") and asserts it
    returns None — proving the dispatcher continues scanning zit_vae
    (which also returns False for "flux2_vae") and eventually returns None
    when no module matches.
    """
    # Create a module-like object with a can_handle that always returns False.
    fake_module = Mock(spec=ModuleType)
    fake_module.can_handle = Mock(return_value=False)

    # Register the fake module.
    vae._REGISTERED_MODULES.append(fake_module)
    try:
        result = vae.get_module("flux2_vae")
        # Both zit_vae.can_handle and fake_module.can_handle are called.
        # zit_vae.can_handle("flux2_vae") returns False, then the fake
        # module's can_handle is called.
        assert fake_module.can_handle.called
        # No module matches "flux2_vae", so result is None.
        assert result is None
    finally:
        # Always clean up: remove the fake module so subsequent tests see
        # the original registry state.
        vae._REGISTERED_MODULES.remove(fake_module)
