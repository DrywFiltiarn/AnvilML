"""Node registration infrastructure for the AnvilML Python worker.

This module defines the base types and registration machinery for the
dynamic node registry. Concrete node implementations (LoadModel,
Sampler, etc.) are added in later phases and imported into this
package via the auto-import mechanism in ``__init__.py``.

The ``@register`` decorator validates that a node class exposes all
required metadata attributes, then records it in ``NODE_REGISTRY``
so that the worker's ``Ready`` event can enumerate available node
types at startup.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Callable

from worker.nodes import NODE_REGISTRY

__all__ = [
    "BaseNode",
    "NodeContext",
    "SlotSpec",
    "register",
]


# ---------------------------------------------------------------------------
# Registry decorator
# ---------------------------------------------------------------------------


def register(cls: type) -> type:
    """Decorate *cls* as a registered node type.

    Validates that the class exposes all six required metadata
    attributes (``NODE_TYPE``, ``CATEGORY``, ``DISPLAY_NAME``,
    ``DESCRIPTION``, ``INPUT_SLOTS``, ``OUTPUT_SLOTS``).  If any
    attribute is missing a ``TypeError`` is raised before the class
    is added to the registry, preventing partially-defined nodes from
    polluting the registry at runtime.

    The validated class is then stored in ``NODE_REGISTRY`` keyed by
    its ``NODE_TYPE`` string and returned unchanged so the decorator
    can be used as ``@register`` on a class definition.

    Args:
        cls: The node class to register. Must have all six metadata
            attributes.

    Returns:
        The same class, now stored in ``NODE_REGISTRY``.

    Raises:
        TypeError: If any of the six required attributes is missing.
    """
    # Validate all six required attributes exist on the class.
    # Missing attributes indicate an incomplete node definition —
    # catching this at decoration time (import time) rather than at
    # runtime prevents confusing errors when the supervisor tries
    # to dispatch jobs to an undefined node type.
    required = ("NODE_TYPE", "CATEGORY", "DISPLAY_NAME",
                "DESCRIPTION", "INPUT_SLOTS", "OUTPUT_SLOTS")
    missing = [attr for attr in required if not hasattr(cls, attr)]
    if missing:
        raise TypeError(
            f"Node class {cls.__name__!r} is missing attributes: "
            f"{', '.join(repr(a) for a in missing)}"
        )

    # Store in the global registry keyed by NODE_TYPE.
    # This is the single source of truth that the worker's Ready
    # event and the Rust scheduler both query for available node types.
    NODE_REGISTRY[cls.NODE_TYPE] = cls
    return cls


# ---------------------------------------------------------------------------
# Slot specification
# ---------------------------------------------------------------------------


@dataclass
class SlotSpec:
    """Declare one input or output slot on a node.

    Each slot specifies its name, the type of data it carries
    (e.g. ``"MODEL"``, ``"LATENT"``, ``"IMAGE"``), and whether
    the slot is optional.

    Args:
        name: Human-readable slot name (e.g. ``"model"``, ``"samples"``).
        slot_type: The data type string. Must match a ``SlotType`` enum
            value defined on the Rust side (e.g. ``"MODEL"``, ``"CLIP"``).
        optional: Whether the slot may be absent in a job graph.
            Defaults to ``False`` (required).
    """

    name: str
    slot_type: str
    optional: bool = False


# ---------------------------------------------------------------------------
# Runtime context
# ---------------------------------------------------------------------------


class NodeContext:
    """Runtime context passed to every node instance.

    Provides access to the current job identity, the target device,
    a cancellation flag, an ``emit`` callable for sending events
    back to the Rust supervisor, and a pipeline cache for
    cross-node data sharing within the same job.

    Args:
        job_id: Unique identifier for the current job.
        device: Target device string (e.g. ``"cuda:0"``, ``"cpu"``).
        cancel_flag: A mutable flag that nodes should check during
            long-running operations to support graceful cancellation.
        emit: Callable that accepts a dict and sends a ``WorkerEvent``
            back to the Rust supervisor via the IPC channel.
        pipeline_cache: A dict-like object for nodes to share data
            within the same job execution.
    """

    def __init__(
        self,
        job_id: str,
        device: str,
        cancel_flag: Any,
        emit: Callable[..., None],
        pipeline_cache: dict[str, Any],
    ) -> None:
        """Initialise the node context.

        Args:
            job_id: Unique identifier for the current job.
            device: Target device string (e.g. ``"cuda:0"``, ``"cpu"``).
            cancel_flag: A mutable flag that nodes should check during
                long-running operations to support graceful cancellation.
            emit: Callable that accepts a dict and sends a ``WorkerEvent``
                back to the Rust supervisor via the IPC channel.
            pipeline_cache: A dict-like object for nodes to share data
                within the same job execution.
        """
        self.job_id = job_id
        self.device = device
        self.cancel_flag = cancel_flag
        self.emit = emit
        self.pipeline_cache = pipeline_cache


# ---------------------------------------------------------------------------
# Abstract base node
# ---------------------------------------------------------------------------


class BaseNode(ABC):
    """Abstract base class for all worker node implementations.

    Subclasses must define six class-level metadata attributes
    (``NODE_TYPE``, ``CATEGORY``, ``DISPLAY_NAME``, ``DESCRIPTION``,
    ``INPUT_SLOTS``, ``OUTPUT_SLOTS``) and implement the
    ``execute()`` method.  The ``@register`` decorator enforces
    the attribute requirements and records the class in
    ``NODE_REGISTRY``.

    The constructor stores a ``NodeContext`` instance that provides
    runtime access to the job, device, cancellation, and IPC channel.

    Subclasses:
        Concrete node types such as ``LoadModel``, ``Sampler``,
        ``VAEDecode``, etc. (added in later phases).
    """

    # Metadata attributes — required for registration.
    # Subclasses must override these with meaningful values.
    NODE_TYPE: str = ""
    CATEGORY: str = ""
    DISPLAY_NAME: str = ""
    DESCRIPTION: str = ""
    INPUT_SLOTS: list[SlotSpec] = []
    OUTPUT_SLOTS: list[SlotSpec] = []

    def __init__(self, ctx: NodeContext) -> None:
        """Store the runtime context for this node instance.

        Args:
            ctx: The ``NodeContext`` providing job, device, cancel,
                emit, and pipeline cache access.
        """
        self.ctx = ctx

    @abstractmethod
    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the node's computation.

        Subclasses must implement this method.  It receives keyword
        arguments corresponding to the node's input slot names and
        must return a dict mapping output slot names to values.

        Args:
            **inputs: Keyword arguments keyed by input slot name,
                with values provided by upstream nodes in the graph.

        Returns:
            Dict mapping output slot names to computed values.
        """
        ...
