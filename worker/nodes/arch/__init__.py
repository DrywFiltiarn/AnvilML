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
from types import ModuleType
from typing import Any

__all__ = ["can_handle", "get_module"]

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


def get_module(model_obj: Any) -> ModuleType | None:
    """Return the first loaded arch module whose ``can_handle()`` matches.

    Iterates through all modules in this package's namespace,
    imports each one, and checks if it exposes a ``can_handle()``
    function that returns ``True`` for the given model object.
    Returns the matching module on first match, or ``None`` if
    no module matches.

    Args:
        model_obj: A model descriptor object carrying attributes
            like ``arch`` (architecture type string).

    Returns:
        The matching architecture module, or ``None`` if no
        loaded arch module claims this model.
    """
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

        handler = getattr(mod, "can_handle", None)
        if handler is not None:
            try:
                if handler(model_obj):
                    return mod  # Found the matching module
            except Exception:
                # A can_handle() that raises is a bug in that
                # module; skip it rather than failing dispatch.
                continue

    return None


def can_handle(model_obj: Any) -> bool:
    """Check whether any loaded architecture module claims this model.

    Delegates to ``get_module()`` — returns ``True`` if a matching
    module is found, ``False`` otherwise. This avoids duplicating
    the module iteration logic that both functions need.

    Args:
        model_obj: A model descriptor object carrying attributes
            like ``arch`` (architecture type string), ``model_id``, etc.

    Returns:
        ``True`` if at least one loaded arch module's ``can_handle()``
        function returns ``True`` for the model; ``False`` otherwise.
    """
    return get_module(model_obj) is not None
