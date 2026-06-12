"""Node package for the AnvilML Python worker.

Imports every ``.py`` module in this package so that node classes
self-register with ``NODE_REGISTRY`` at startup.
"""

from __future__ import annotations

import importlib
import os
import pkgutil

# Always import the base module so NODE_REGISTRY and BaseNode are available.
from . import base  # noqa: F401  # type: ignore[reportUnusedImport]

# Auto-discover and import all other node modules in this package directory.
_package_dir = os.path.dirname(__file__)
for _finder, _mod_name, _is_pkg in pkgutil.iter_modules([_package_dir]):
    if _mod_name == "base":
        continue  # Already imported above.
    try:
        importlib.import_module(f".{_mod_name}", package=__name__)
    except ImportError:
        # A node module may have hard dependencies (e.g. torch, diffusers)
        # that are unavailable in certain environments. Silently skip.
        pass
