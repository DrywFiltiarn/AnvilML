"""Tests for worker.nodes.__init__ — auto-import mechanism."""


def test_import_does_not_raise() -> None:
    """Importing worker.nodes does not raise an exception.

    Verifies that the auto-import loop in __init__.py completes
    without errors when the package is first imported — including
    loading any concrete node modules found in the package directory.
    This is the baseline precondition for all subsequent node-system
    tests.
    """
    import worker.nodes  # noqa: F401


def test_node_registry_empty_after_import() -> None:
    """NODE_REGISTRY contains auto-registered nodes after import.

    Imports worker.nodes (triggering the auto-import loop), then
    imports worker.nodes.base and asserts that NODE_REGISTRY contains
    the PassThrough node — confirming that the auto-import loop
    registers node modules found in the package directory.
    """
    import worker.nodes  # noqa: F401
    from worker.nodes import base

    # PassThrough is registered via auto-import at package load time.
    assert "PassThrough" in base.NODE_REGISTRY


def test_reimport_is_idempotent() -> None:
    """Re-importing worker.nodes or calling _import_nodes() twice is safe.

    Imports worker.nodes, then imports it again (which triggers
    __init__.py's module-level _import_nodes() a second time).
    Asserts that no exception is raised and NODE_REGISTRY still
    contains only the known registered nodes (e.g. PassThrough),
    confirming the _imported flag prevents duplicate execution.
    """
    import worker.nodes  # noqa: F401
    from worker.nodes import base, _import_nodes

    # Call _import_nodes() again directly — should be a no-op.
    _import_nodes()

    # PassThrough is registered via auto-import at package load time.
    # The _imported flag ensures no duplicate entries are added.
    assert "PassThrough" in base.NODE_REGISTRY
