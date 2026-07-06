"""Tests for worker.nodes.__init__ — auto-import mechanism."""


def test_import_does_not_raise() -> None:
    """Importing worker.nodes does not raise an exception.

    Verifies that the auto-import loop in __init__.py completes
    without errors when the package is first imported — even though
    no concrete node files exist yet. This is the baseline
    precondition for all subsequent node-system tests.
    """
    import worker.nodes  # noqa: F401


def test_node_registry_empty_after_import() -> None:
    """NODE_REGISTRY is empty immediately after import (no node files exist yet).

    Imports worker.nodes (triggering the auto-import loop), then
    imports worker.nodes.base and asserts that NODE_REGISTRY is
    still empty — confirming that the auto-import loop does not
    register anything when no sibling node modules exist.
    """
    import worker.nodes  # noqa: F401
    from worker.nodes import base

    assert base.NODE_REGISTRY == {}


def test_reimport_is_idempotent() -> None:
    """Re-importing worker.nodes or calling _import_nodes() twice is safe.

    Imports worker.nodes, then imports it again (which triggers
    __init__.py's module-level _import_nodes() a second time).
    Asserts that no exception is raised and NODE_REGISTRY remains
    empty, confirming the _imported flag prevents duplicate execution.
    """
    import worker.nodes  # noqa: F401
    from worker.nodes import base, _import_nodes

    # Call _import_nodes() again directly — should be a no-op.
    _import_nodes()

    assert base.NODE_REGISTRY == {}
