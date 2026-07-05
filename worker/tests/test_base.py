"""Tests for worker.nodes.base — SlotSpec dataclass, NODE_REGISTRY dict, and @register decorator."""

from worker.nodes import base


# Module-level helper: a class with all 6 required attributes, so tests don't
# repeat them. Individual tests that need to omit one attribute subclass this
# and delete the attribute via `delattr` before passing to @register.
class _FullySpecifiedNode:
    """A node class with every required attribute populated.

    Used as the base class for success and missing-attr tests — concrete
    subclasses only modify what the individual test needs to change.
    """
    NODE_TYPE = "test.node"
    CATEGORY = "test"
    DISPLAY_NAME = "Test Node"
    DESCRIPTION = "A test node with all required attributes."
    INPUT_SLOTS = []
    OUTPUT_SLOTS = []


def test_node_registry_starts_empty() -> None:
    """NODE_REGISTRY is an empty dict immediately after import.

    Verifies that the module-level NODE_REGISTRY global starts empty
    when the base module is first imported — no nodes have been
    registered yet. This is the precondition for all subsequent
    registration tests.
    """
    assert base.NODE_REGISTRY == {}


def test_slotspec_optional_defaults_to_false() -> None:
    """SlotSpec(name, slot_type) has optional=False by default.

    Constructs a SlotSpec with only the required fields and asserts
    that the optional attribute defaults to False, confirming the
    dataclass default is applied correctly.
    """
    spec = base.SlotSpec(name="x", slot_type="MODEL")
    assert spec.optional is False
    assert spec.name == "x"
    assert spec.slot_type == "MODEL"


def test_slotspec_accepts_explicit_optional_true() -> None:
    """SlotSpec(name, slot_type, optional=True) sets optional to True.

    Constructs a SlotSpec with optional=True explicitly and asserts
    the attribute is True, confirming the optional parameter is
    accepted and stored correctly.
    """
    spec = base.SlotSpec(name="y", slot_type="IMAGE", optional=True)
    assert spec.optional is True
    assert spec.name == "y"
    assert spec.slot_type == "IMAGE"


def test_register_success() -> None:
    """@register on a fully-specified class inserts it into NODE_REGISTRY.

    Defines a class with all 6 required attributes, decorates it with
    @register, and asserts it appears in NODE_REGISTRY under its
    NODE_TYPE key. Then removes the entry to avoid polluting subsequent
    tests.
    """
    class TestNode(_FullySpecifiedNode):
        pass

    decorated = base.register(TestNode)
    assert "test.node" in base.NODE_REGISTRY
    assert base.NODE_REGISTRY["test.node"] is TestNode
    # Clean up: remove the entry so subsequent tests see a clean registry.
    del base.NODE_REGISTRY["test.node"]


def test_register_missing_NODE_TYPE() -> None:
    """Missing NODE_TYPE raises TypeError naming NODE_TYPE.

    Dynamically creates a class without NODE_TYPE (using type()) and
    asserts @register raises TypeError with "NODE_TYPE" in the message.
    """
    BadNode = type("BadNode", (), {
        "CATEGORY": "test",
        "DISPLAY_NAME": "Test Node",
        "DESCRIPTION": "A test node with all required attributes.",
        "INPUT_SLOTS": [],
        "OUTPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "NODE_TYPE" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_missing_CATEGORY() -> None:
    """Missing CATEGORY raises TypeError naming CATEGORY.

    Dynamically creates a class without CATEGORY (using type()) and
    asserts @register raises TypeError with "CATEGORY" in the message.
    """
    BadNode = type("BadNode", (), {
        "NODE_TYPE": "test.node",
        "DISPLAY_NAME": "Test Node",
        "DESCRIPTION": "A test node with all required attributes.",
        "INPUT_SLOTS": [],
        "OUTPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "CATEGORY" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_missing_DISPLAY_NAME() -> None:
    """Missing DISPLAY_NAME raises TypeError naming DISPLAY_NAME.

    Dynamically creates a class without DISPLAY_NAME (using type()) and
    asserts @register raises TypeError with "DISPLAY_NAME" in the message.
    """
    BadNode = type("BadNode", (), {
        "NODE_TYPE": "test.node",
        "CATEGORY": "test",
        "DESCRIPTION": "A test node with all required attributes.",
        "INPUT_SLOTS": [],
        "OUTPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "DISPLAY_NAME" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_missing_DESCRIPTION() -> None:
    """Missing DESCRIPTION raises TypeError naming DESCRIPTION.

    Dynamically creates a class without DESCRIPTION (using type()) and
    asserts @register raises TypeError with "DESCRIPTION" in the message.
    """
    BadNode = type("BadNode", (), {
        "NODE_TYPE": "test.node",
        "CATEGORY": "test",
        "DISPLAY_NAME": "Test Node",
        "INPUT_SLOTS": [],
        "OUTPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "DESCRIPTION" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_missing_INPUT_SLOTS() -> None:
    """Missing INPUT_SLOTS raises TypeError naming INPUT_SLOTS.

    Dynamically creates a class without INPUT_SLOTS (using type()) and
    asserts @register raises TypeError with "INPUT_SLOTS" in the message.
    """
    BadNode = type("BadNode", (), {
        "NODE_TYPE": "test.node",
        "CATEGORY": "test",
        "DISPLAY_NAME": "Test Node",
        "DESCRIPTION": "A test node with all required attributes.",
        "OUTPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "INPUT_SLOTS" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_missing_OUTPUT_SLOTS() -> None:
    """Missing OUTPUT_SLOTS raises TypeError naming OUTPUT_SLOTS.

    Dynamically creates a class without OUTPUT_SLOTS (using type()) and
    asserts @register raises TypeError with "OUTPUT_SLOTS" in the message.
    """
    BadNode = type("BadNode", (), {
        "NODE_TYPE": "test.node",
        "CATEGORY": "test",
        "DISPLAY_NAME": "Test Node",
        "DESCRIPTION": "A test node with all required attributes.",
        "INPUT_SLOTS": [],
    })
    try:
        base.register(BadNode)
    except TypeError as exc:
        assert "OUTPUT_SLOTS" in str(exc)
    else:
        assert False, "register() should have raised TypeError"


def test_register_returns_class_identity() -> None:
    """@register returns the exact same class object (identity preserved).

    Decorates a fully-specified class and asserts the return value is
    the original class via `is` comparison — confirming that register()
    does not wrap or proxy the class, preserving MRO and method resolution.
    """
    class TestNode(_FullySpecifiedNode):
        pass

    result = base.register(TestNode)
    assert result is TestNode
    # Clean up: remove the entry so subsequent tests see a clean registry.
    del base.NODE_REGISTRY["test.node"]
