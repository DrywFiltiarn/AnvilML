"""Architecture dispatch registry for the AnvilML worker.

This package provides dynamic architecture module loading and model-architecture
dispatch. On first import, ``_ensure_imported()`` scans the ``worker/nodes/arch``
directory for sibling ``.py`` files and imports each one, allowing architecture-
specific modules to register their ``can_handle()`` functions.

The ``can_handle()`` dispatcher iterates through all loaded arch modules'
``can_handle()`` functions and returns ``True`` if any match, enabling the
Sampler node to select the correct sampling path for a given model.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import logging
import pkgutil
from typing import Any

__all__ = ["can_handle"]

# Module-level flag for idempotency — ensures _ensure_imported()
# runs exactly once even if the module is re-imported.
_imported: bool = False

logger = logging.getLogger(__name__)


def _ensure_imported() -> None:
    """Import all sibling architecture modules exactly once.

    Uses ``pkgutil.iter_modules()`` to enumerate ``.py`` files in
    this package's directory, then imports each one via
    ``importlib.import_module()``. Import errors are caught and
    logged as warnings — a missing or broken arch module should not
    prevent the worker from starting.

    Importing these modules is what populates the arch namespace with
    ``can_handle()`` functions that the dispatcher later calls.
    """
    global _imported

    # Idempotency guard: if this function has already run, skip.
    # This prevents duplicate imports if the module is re-imported
    # or if multiple code paths trigger the auto-import.
    if _imported:
        return

    _imported = True

    # Scan the package directory for sibling .py modules.
    # pkgutil.iter_modules returns ModuleInfo namedtuples with
    # .name (module name without .py extension) and .ispkg flag.
    for mod_info in pkgutil.iter_modules(__path__):
        # Skip non-.py modules (e.g. compiled extensions or packages).
        if mod_info.ispkg:
            continue

        # Build the full dotted module name so importlib can resolve it.
        # The "worker.nodes.arch." prefix is required — bare module names
        # like "zit" would cause ModuleNotFoundError.
        full_name = f"worker.nodes.arch.{mod_info.name}"

        try:
            importlib.import_module(full_name)
        except ImportError as exc:
            # Log a warning but continue — a broken arch module should
            # not prevent the worker from starting. The architecture
            # simply won't be available for model dispatch.
            logger.warning(
                "Failed to import arch module %s: %s",
                full_name,
                exc,
            )


# Trigger auto-import at module load time so that any code that
# imports `worker.nodes.arch` gets access to the loaded arch modules
# without needing to call _ensure_imported() explicitly.
_ensure_imported()


def can_handle(model_obj: Any) -> bool:
    """Check whether any loaded architecture module claims this model.

    Iterates through all loaded arch modules' ``can_handle()`` functions
    and returns ``True`` if any one of them returns ``True`` for the
    given model object. If no arch modules are loaded, returns ``False``.

    Args:
        model_obj: A model descriptor object carrying attributes like
            ``arch`` (architecture type string), ``model_id``, etc.

    Returns:
        ``True`` if at least one loaded arch module's ``can_handle()``
        function returns ``True`` for the model; ``False`` otherwise.
    """
    # Iterate through all modules in this package's namespace.
    # Each loaded arch module should expose a ``can_handle()`` function
    # that returns True if it can handle the given model object.
    for mod_info in pkgutil.iter_modules(__path__):
        if mod_info.ispkg:
            continue

        full_name = f"worker.nodes.arch.{mod_info.name}"

        try:
            mod = importlib.import_module(full_name)
        except ImportError:
            # Module failed to import earlier; skip it.
            # The warning was already logged by _ensure_imported().
            continue

        # Check if this module has a can_handle function and call it.
        # getattr returns None if the attribute doesn't exist, so we
        # safely skip modules that don't export can_handle().
        handler = getattr(mod, "can_handle", None)
        if handler is not None:
            try:
                if handler(model_obj):
                    return True
            except Exception:
                # A can_handle() implementation that raises an exception
                # is a bug in that module; skip it rather than failing
                # the entire dispatch check.
                continue

    return False
