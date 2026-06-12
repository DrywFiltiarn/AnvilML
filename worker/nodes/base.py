"""Base node infrastructure for the AnvilML Python worker.

Defines the ``NodeContext`` dataclass, the abstract ``BaseNode`` class with
class-level slot declarations, a module-level ``NODE_REGISTRY`` dictionary,
and a ``@register`` decorator that auto-populates the registry keyed by
``NODE_TYPE``.
"""

from __future__ import annotations

import threading
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, ClassVar


@dataclass
class NodeContext:
    """Execution context passed to every node.

    Attributes:
        pipeline_cache:  LRU cache for loaded pipeline objects.
        device_str:      Device string (e.g. ``"cuda:0"``, ``"cpu"``).
        emit_fn:         Callback to emit IPC events to the Rust supervisor.
        cancel_flag:     Event set when the current job is cancelled.
        job_id:          Unique identifier for the current job.
    """

    pipeline_cache: Any
    device_str: str
    emit_fn: Any
    cancel_flag: threading.Event
    job_id: str


class BaseNode(ABC):
    """Abstract base class for all worker nodes.

    Subclasses declare their slot topology via class variables and
    implement ``execute`` to perform the actual computation.
    """

    NODE_TYPE: ClassVar[str] = ""
    INPUT_SLOTS: ClassVar[list[str]] = []
    OUTPUT_SLOTS: ClassVar[list[str]] = []

    def __init__(self, ctx: NodeContext) -> None:
        """Initialise the node with the given execution context.

        Args:
            ctx: The node execution context.
        """
        self.ctx = ctx

    @abstractmethod
    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute this node with the given slot inputs.

        Args:
            **inputs: Slot name -> value mapping.

        Returns:
            Dict of slot name -> output value.
        """
        ...


NODE_REGISTRY: dict[str, type[BaseNode]] = {}


def register(cls: type[BaseNode]) -> type[BaseNode]:
    """Class decorator that registers a node type in ``NODE_REGISTRY``.

    The registry key is the class's ``NODE_TYPE`` class variable.

    Args:
        cls: The node class to register.

    Returns:
        The original class (unchanged).
    """
    NODE_REGISTRY[cls.NODE_TYPE] = cls
    return cls
