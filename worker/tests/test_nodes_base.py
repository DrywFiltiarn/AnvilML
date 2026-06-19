"""Unit tests for the node registration infrastructure.

Tests cover the NODE_REGISTRY global, the @register decorator,
BaseNode ABC enforcement, and the SlotSpec dataclass.
"""

from __future__ import annotations

from dataclasses import dataclass

import pytest

from worker.nodes import NODE_REGISTRY
from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def registry_clean() -> None:
    """Clear NODE_REGISTRY before each test to ensure isolation.

    The @register decorator modifies NODE_REGISTRY globally.
    This fixture clears it before each test so tests don't
    leak state into one another.
    """
    NODE_REGISTRY.clear()


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


def test_registry_populated_after_import() -> None:
    """Verify that importing ``worker.nodes`` does not raise and
    ``NODE_REGISTRY`` is accessible as a dict.

    At this stage no concrete node modules exist, so the registry
    is empty — this test verifies the import path works without
    errors and the registry is a dict.
    """
    import worker.nodes

    assert isinstance(NODE_REGISTRY, dict)


def test_register_decorator_adds_class() -> None:
    """Verify that the ``@register`` decorator adds a class to
    ``NODE_REGISTRY`` keyed by its ``NODE_TYPE``.

    Creates a concrete test node class with all six required
    attributes, applies ``@register``, and asserts the class
    appears in ``NODE_REGISTRY`` under the correct key.
    """
    # Define a minimal concrete node class with all required
    # metadata attributes. This is the minimum viable node —
    # no execute() implementation needed for registration.
    @register
    class TestNode(BaseNode):
        NODE_TYPE = "TestNode"
        CATEGORY = "test"
        DISPLAY_NAME = "Test Node"
        DESCRIPTION = "A test node for unit testing"
        INPUT_SLOTS = []
        OUTPUT_SLOTS = []

        def execute(self, **inputs: object) -> dict[str, object]:
            return {}

    assert "TestNode" in NODE_REGISTRY
    assert NODE_REGISTRY["TestNode"] is TestNode


def test_base_node_cannot_be_instantiated() -> None:
    """Verify that ``BaseNode()`` raises ``TypeError`` because
    it is an abstract base class (ABC).

    Attempting to instantiate the ABC directly must fail — this
    is the core enforcement mechanism that prevents accidental
    use of the abstract class.
    """
    with pytest.raises(TypeError):
        BaseNode()


def test_slot_spec_dataclass() -> None:
    """Verify that ``SlotSpec`` creates a dataclass instance with
    correct fields and default values.

    Constructs a ``SlotSpec`` with just name and slot_type, then
    asserts the optional field defaults to ``False``.
    """
    spec = SlotSpec("input1", "MODEL")

    assert spec.name == "input1"
    assert spec.slot_type == "MODEL"
    assert spec.optional is False

    # Verify explicit optional=True also works.
    spec_opt = SlotSpec("seed", "Int", optional=True)
    assert spec_opt.optional is True
