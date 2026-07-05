"""Base node ABC and registration decorator."""

from __future__ import annotations
from abc import ABC, abstractmethod
from typing import Any
from dataclasses import dataclass

NODE_REGISTRY: dict[str, type["BaseNode"]] = {}


def register(cls: type) -> type:
    """Register a node class in NODE_REGISTRY.

    The class must define NODE_TYPE (str), CATEGORY (str), DISPLAY_NAME (str),
    DESCRIPTION (str), INPUT_SLOTS (list[SlotSpec]), and OUTPUT_SLOTS (list[SlotSpec]).

    Raises:
        TypeError: If any required attribute is missing.
    """
    required = ("NODE_TYPE", "CATEGORY", "DISPLAY_NAME", "DESCRIPTION",
                "INPUT_SLOTS", "OUTPUT_SLOTS")
    for attr in required:
        if not hasattr(cls, attr):
            raise TypeError(f"@register: {cls.__name__} missing {attr}")
    NODE_REGISTRY[cls.NODE_TYPE] = cls
    return cls


@dataclass
class SlotSpec:
    """Declares one input or output slot on a node."""
    name: str
    slot_type: str          # Must match a SlotType value (e.g. "MODEL", "CLIP")
    optional: bool = False
