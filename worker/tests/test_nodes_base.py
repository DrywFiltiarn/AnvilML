"""Tests for :mod:`worker.nodes.base`."""

from __future__ import annotations

import threading
from unittest import mock

import pytest

from worker.nodes.base import (
    NODE_REGISTRY,
    BaseNode,
    NodeContext,
    register,
)


@pytest.fixture(autouse=True)
def _clear_registry() -> None:
    """Ensure NODE_REGISTRY is empty before each test."""
    NODE_REGISTRY.clear()


class TestRegisterPopulatesRegistry:
    """Tests for the ``@register`` decorator."""

    def test_register_populates_registry(self) -> None:
        """@register adds a class to NODE_REGISTRY keyed by NODE_TYPE."""

        class DummyNode(BaseNode):
            NODE_TYPE = "dummy_test_node"

            def execute(self, **inputs: object) -> dict[str, object]:
                return {}

        registered = register(DummyNode)

        assert "dummy_test_node" in NODE_REGISTRY
        assert NODE_REGISTRY["dummy_test_node"] is DummyNode
        # Decorator returns the original class unchanged.
        assert registered is DummyNode


class TestMissingExecuteRaisesTypeError:
    """Tests for abstract method enforcement on BaseNode."""

    def test_missing_execute_raises_typeerror(self) -> None:
        """Subclass without execute cannot be instantiated."""

        class IncompleteNode(BaseNode):
            NODE_TYPE = "incomplete"

        with pytest.raises(TypeError):
            IncompleteNode(ctx=mock.MagicMock(spec=NodeContext))
