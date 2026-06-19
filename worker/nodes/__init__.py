"""Python-side node registry for the AnvilML worker.

This package provides the dynamic node registration infrastructure.
On first import, ``_ensure_imported()`` scans the ``worker/nodes``
directory for sibling ``.py`` files (excluding ``base.py`` itself)
and imports each one, allowing concrete node classes to register
themselves via the ``@register`` decorator.

The ``NODE_REGISTRY`` dict is the single source of truth for
available node types — it is populated by ``@register`` calls
at import time and consumed by ``worker_main._build_node_types_list()``
to build the ``node_types`` field of the ``Ready`` IPC event.
"""

from __future__ import annotations

import importlib
import logging
import pkgutil

# Define NODE_REGISTRY BEFORE importing from base.py to break
# the circular import. base.py needs NODE_REGISTRY for the
# @register decorator, so it must exist before base.py is loaded.
# Global registry mapping node type strings to their class objects.
# Populated at import time by the @register decorator on each
# concrete node module. The worker's Ready event reads this dict
# to advertise available node types to the Rust supervisor.
NODE_REGISTRY: dict[str, type] = {}

from worker.nodes.base import (
    BaseNode,
    NodeContext,
    SlotSpec,
    register,
)

__all__ = [
    "BaseNode",
    "NodeContext",
    "NODE_REGISTRY",
    "SlotSpec",
    "register",
]

# Module-level flag for idempotency — ensures _ensure_imported()
# runs exactly once even if the module is re-imported.
_imported: bool = False

logger = logging.getLogger(__name__)


def _ensure_imported() -> None:
    """Import all sibling node modules exactly once.

    Uses ``pkgutil.iter_modules()`` to enumerate ``.py`` files in
    this package's directory, then imports each one via
    ``importlib.import_module()``.  Import errors are caught and
    logged as warnings — a missing or broken node module should not
    prevent the worker from starting; it simply won't be registered.

    The ``base.py`` module is excluded because it defines the
    infrastructure (BaseNode, register, etc.) rather than a
    concrete node type.  It is imported directly above, not via
    the auto-import loop.
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
        # Skip base.py — it defines the registration infrastructure,
        # not a concrete node type. Importing it here would create a
        # circular dependency (base.py imports NODE_REGISTRY from here).
        if mod_info.name == "base":
            continue

        # Skip non-.py modules (e.g. compiled extensions or packages).
        if mod_info.ispkg:
            continue

        # Build the full dotted module name so importlib can resolve it.
        # The "worker.nodes." prefix is required — bare module names
        # like "load_model" would cause ModuleNotFoundError.
        full_name = f"worker.nodes.{mod_info.name}"

        try:
            importlib.import_module(full_name)
        except ImportError as exc:
            # Log a warning but continue — a broken node module should
            # not prevent the worker from starting. The node simply won't
            # be registered, and the Rust supervisor will not dispatch
            # jobs to that node type.
            logger.warning(
                "Failed to import node module %s: %s",
                full_name,
                exc,
            )


# Trigger auto-import at module load time so that any code that
# imports `worker.nodes` gets a populated NODE_REGISTRY without
# needing to call _ensure_imported() explicitly.
_ensure_imported()
