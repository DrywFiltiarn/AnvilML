"""Base node ABC and registration decorator."""

from __future__ import annotations
from abc import ABC, abstractmethod
from typing import Any
from dataclasses import dataclass

NODE_REGISTRY: dict[str, type["BaseNode"]] = {}


@dataclass
class SlotSpec:
    """Declares one input or output slot on a node."""
    name: str
    slot_type: str          # Must match a SlotType value (e.g. "MODEL", "CLIP")
    optional: bool = False
