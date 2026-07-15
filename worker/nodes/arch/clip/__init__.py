"""CLIP architecture family module dispatcher.

This module provides the shared dispatch mechanism for the CLIP
architecture family (Qwen3, etc.). Each concrete CLIP
arch module defines its own ``can_handle(key)`` method; the dispatcher
scans the registered modules and returns the first match.

With zero registered modules (concrete arch modules are wired in later
phases), ``get_module`` returns ``None`` silently — never raises.

Design: ANVILML_DESIGN.md §10.4 — ``get_module`` is the ONE shared
dispatcher per family, never reimplemented per module.
"""

from __future__ import annotations

from types import ModuleType
from typing import Any

from worker.nodes.arch.clip import qwen3

_REGISTERED_MODULES: list[ModuleType] = []
_REGISTERED_MODULES.append(qwen3)


def get_module(key: Any) -> ModuleType | None:
    """Return the first registered CLIP module that handles *key*.

    Iterates over ``_REGISTERED_MODULES`` in registration order and calls
    ``module.can_handle(key)`` on each. Returns the first module whose
    ``can_handle`` returns ``True``.

    If the registry is empty (no concrete arch modules have been wired in
    yet) or no registered module handles the key, returns ``None``.

    Args:
        key: The module key to look up — typically a clip_type string
            such as ``"qwen3"`` identifying a text-encoder variant.

    Returns:
        The first matching ``ModuleType``, or ``None`` if no match.
    """
    for module in _REGISTERED_MODULES:
        # can_handle is defined by each concrete CLIP arch module;
        # it returns True when the module's text-encoder kind matches
        # the clip_type key.
        if module.can_handle(key):
            return module

    return None
