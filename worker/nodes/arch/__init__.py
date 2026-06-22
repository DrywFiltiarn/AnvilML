"""Architecture dispatch registry for the AnvilML worker.

This package re-exports ``can_handle()`` and ``get_module()`` from
``worker.nodes.arch.diffusion``, which contains the actual
``pkgutil.iter_modules()`` auto-import scanning logic and dispatch
functions for diffusion architecture modules.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

from worker.nodes.arch.diffusion import can_handle, get_module

__all__ = ["can_handle", "get_module"]
