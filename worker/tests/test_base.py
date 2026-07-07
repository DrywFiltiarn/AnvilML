"""Tests for worker.nodes.base — SlotSpec dataclass, NODE_REGISTRY dict, and @register decorator."""

import threading
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
    """NODE_REGISTRY contains only known auto-registered nodes after import.

    Verifies that the module-level NODE_REGISTRY global contains at
    least the PassThrough node (auto-registered by _import_nodes())
    when the base module is first imported — confirming that the
    auto-import mechanism works. This is the precondition for all
    subsequent registration tests.
    """
    # PassThrough is registered via auto-import at package load time.
    assert "PassThrough" in base.NODE_REGISTRY


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


def test_node_context_assigns_all_attrs() -> None:
    """NodeContext constructor assigns all 7 attributes to matching self attributes.

    Constructs a NodeContext with concrete values for every parameter and
    asserts each attribute is stored correctly — job_id, device, caps,
    cancel_flag, emit, pipeline_cache, and mock.
    """
    import threading

    cancel = threading.Event()
    ctx = base.NodeContext(
        job_id="test-job",
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=cancel,
        emit=lambda e: None,
        pipeline_cache={},
        mock=True,
    )
    assert ctx.job_id == "test-job"
    assert ctx.device == "cpu"
    assert ctx.caps == {"bf16": True, "fp8": False}
    assert ctx.cancel_flag is cancel
    assert ctx.emit is not None
    assert ctx.pipeline_cache == {}
    assert ctx.mock is True


def test_node_context_mock_true() -> None:
    """NodeContext constructs cleanly with mock=True.

    Constructs a NodeContext with mock=True and asserts the mock attribute
    is True, confirming the flag is stored without transformation.
    """
    ctx = base.NodeContext(
        job_id="test-job",
        device="cpu",
        caps={},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache={},
        mock=True,
    )
    assert ctx.mock is True


def test_node_context_mock_false() -> None:
    """NodeContext constructs cleanly with mock=False.

    Constructs a NodeContext with mock=False and asserts the mock attribute
    is False, confirming the flag is stored without transformation.
    """
    ctx = base.NodeContext(
        job_id="test-job",
        device="cpu",
        caps={},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache={},
        mock=False,
    )
    assert ctx.mock is False


def test_node_context_caps_accepts_arbitrary_dict() -> None:
    """caps accepts any arbitrary dict without validation.

    Constructs a NodeContext with a non-standard dict (arbitrary keys and
    mixed-value types) and asserts it is stored unchanged, confirming that
    NodeContext imposes no validation on the caps payload.
    """
    ctx = base.NodeContext(
        job_id="test-job",
        device="cpu",
        caps={"some_key": "some_value", "numeric": 42},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache={},
        mock=False,
    )
    assert ctx.caps == {"some_key": "some_value", "numeric": 42}


def test_base_node_cannot_be_instantiated() -> None:
    """BaseNode cannot be instantiated directly — ABC machinery enforces this.

    Asserts that calling BaseNode() raises TypeError, confirming that
    Python's ABC machinery prevents direct instantiation of the abstract
    base class. This is enforced by the abstract method decorator, not
    by custom code.
    """
    try:
        base.BaseNode()
    except TypeError:
        pass
    else:
        assert False, "BaseNode() should have raised TypeError"


def test_concrete_subclass_instantiates() -> None:
    """A concrete subclass implementing execute() instantiates without error.

    Defines a minimal subclass that provides execute() returning an empty
    dict, and asserts that instantiation succeeds. This confirms that the
    abstract method requirement is satisfied by providing execute().
    """
    class ConcreteNode(base.BaseNode):
        def execute(self, ctx, **inputs) -> dict:
            return {}

    node = ConcreteNode()
    assert isinstance(node, base.BaseNode)


def test_execute_calls_subclass_impl() -> None:
    """Calling execute() on a concrete subclass invokes the subclass's implementation.

    Defines a concrete subclass that sets self.called = True in execute(),
    instantiates it, calls execute(), and asserts the flag is True. This
    proves the subclass's implementation runs rather than a base no-op.

    This guards against a future regression where a base no-op is
    accidentally called instead of the subclass's overridden method.
    """
    class FlagNode(base.BaseNode):
        def __init__(self) -> None:
            self.called = False

        def execute(self, ctx, **inputs) -> dict:
            self.called = True
            return {}

    node = FlagNode()
    node.execute(None)
    assert node.called is True
