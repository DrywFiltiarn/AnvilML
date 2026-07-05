"""Tests for worker.nodes.base — SlotSpec dataclass and NODE_REGISTRY dict."""

from worker.nodes import base


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
