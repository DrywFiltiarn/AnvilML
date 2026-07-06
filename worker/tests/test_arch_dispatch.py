"""Tests for worker.nodes.arch.diffusion — get_module dispatcher."""

from types import ModuleType
from unittest.mock import Mock

from worker.nodes.arch import diffusion


def test_get_module_returns_none_when_empty() -> None:
    """get_module returns None when _REGISTERED_MODULES is empty.

    With zero registered modules, get_module("zit") must return None
    without raising — the empty registry is the default state before
    concrete arch modules are wired in later phases.
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
