"""Architecture dispatch registry for the AnvilML worker CLIP backends.

This package provides dynamic CLIP architecture module loading and model-architecture
dispatch. On first import, ``_ensure_imported()`` scans the ``worker/nodes/arch/clip``
directory for sibling ``.py`` files and imports each one, allowing architecture-
specific modules to register their ``can_handle()`` functions.

The ``can_handle()`` dispatcher iterates through all loaded arch modules'
``can_handle()`` functions and returns ``True`` if any match, enabling the
LoadClip node to select the correct loader for a given clip type string.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import logging
import pkgutil
from types import ModuleType

__all__ = ["can_handle", "get_module"]

# Module-level flag for idempotency — ensures _ensure_imported()
# runs exactly once even if the module is re-imported.
_imported: bool = False

logger = logging.getLogger(__name__)


def _ensure_imported() -> None:
    """Import all sibling CLIP architecture modules exactly once.

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
        # The "worker.nodes.arch.clip." prefix is required — bare
        # module names like "qwen3" would cause ModuleNotFoundError.
        full_name = f"worker.nodes.arch.clip.{mod_info.name}"

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
# imports `worker.nodes.arch.clip` gets access to the loaded arch modules
# without needing to call _ensure_imported() explicitly.
_ensure_imported()


def get_module(clip_type: str) -> ModuleType | None:
    """Return the first loaded clip arch module whose ``can_handle()`` matches.

    Iterates through all modules in this package's namespace,
    imports each one, and checks if it exposes a ``can_handle()``
    function that returns ``True`` for the given clip type string.
    Returns the matching module on first match, or ``None`` if
    no module matches.

    Args:
        clip_type: The clip type string to match against (e.g.
            "qwen3", "clip_l", "t5").

    Returns:
        The matching architecture module, or ``None`` if no
        loaded clip arch module claims this clip_type.
    """
    for mod_info in pkgutil.iter_modules(__path__):
        if mod_info.ispkg:
            continue

        full_name = f"worker.nodes.arch.clip.{mod_info.name}"

        try:
            mod = importlib.import_module(full_name)
        except ImportError:
            # Module failed to import earlier; skip it.
            # The warning was already logged by _ensure_imported().
            continue

        handler = getattr(mod, "can_handle", None)
        if handler is not None:
            try:
                # Pass the clip_type string directly to the handler.
                # Unlike diffusion dispatch (which passes a model object),
                # clip dispatch operates on the type string alone since
                # no loaded model object exists yet at LoadClip dispatch time.
                if handler(clip_type):
                    return mod  # Found the matching module
            except Exception:
                # A can_handle() that raises is a bug in that
                # module; skip it rather than failing dispatch.
                continue

    return None


def can_handle(clip_type: str) -> bool:
    """Check whether any loaded clip architecture module claims this clip_type.

    Delegates to ``get_module()`` — returns ``True`` if a matching
    module is found, ``False`` otherwise. This avoids duplicating
    the module iteration logic that both functions need.

    Args:
        clip_type: The clip type string (e.g. "qwen3", "clip_l", "t5").

    Returns:
        ``True`` if at least one loaded clip arch module's ``can_handle()``
        function returns ``True`` for the clip_type; ``False`` otherwise.
    """
    return get_module(clip_type) is not None
