"""Node package — auto-imports node modules to trigger @register side effects."""

_imported: bool = False


def _import_nodes() -> None:
    """Import all .py files directly under nodes/ (not recursively into arch/).

    Each imported module's @register decorator side-effect populates
    NODE_REGISTRY. This function is idempotent — calling it a second
    time has no effect.
    """
    global _imported
    if _imported:
        return
    _imported = True

    import os
    import pkgutil
    import importlib.util

    nodes_dir = os.path.dirname(__file__)
    for _importer, mod_name, is_pkg in pkgutil.iter_modules([nodes_dir]):
        # Skip __init__ (this package) and base (loaded as dependency).
        # Skip packages (is_pkg=True) to avoid recursing into arch/.
        if mod_name in ("__init__", "base") or is_pkg:
            continue
        spec = importlib.util.find_spec(f"worker.nodes.{mod_name}")
        if spec is None:
            continue
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)


# Run auto-import at package load time.
_import_nodes()
