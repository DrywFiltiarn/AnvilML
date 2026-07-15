"""Base node ABC and registration decorator."""

from __future__ import annotations
from abc import ABC, abstractmethod
from typing import Any
from dataclasses import dataclass
import uuid

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


class NodeContext:
    """Runtime context passed to every node's execute() method.

    Attributes:
        job_id: The raw msgpack-decoded bytes of the currently executing
            job's UUID — NOT a string. Rust's `Uuid` serializes as raw
            16 bytes over msgpack (a non-human-readable wire format), so
            this is exactly what arrives over IPC. Passing this directly
            to `ctx.emit(...)` payloads (or anything else that must
            round-trip back to Rust) is correct and required. For a
            human-readable form — log messages, cache-key labels, or
            anything else meant to be read by a person — use
            `job_id_str` instead; `str(job_id)`/f-string interpolation
            of the raw bytes renders unreadably (e.g. `b'\\x1d9...'`).
        device: The torch device string (e.g. "cuda:0", "cpu"). Unused in mock mode.
        caps: The worker's own InferenceCaps dict from capability.probe_capabilities()
            (or the mock equivalent). Arch modules read dtype decisions from this —
            never from a Rust-side hint — per §6.6/§11.5.
        cancel_flag: threading.Event; set when the job is cancelled.
        emit: Callable for emitting WorkerEvent dicts back to the supervisor.
        pipeline_cache: The shared LRU model/pipeline cache.
        mock: bool — True if ANVILML_WORKER_MOCK=1. Nodes branch on this exactly
            once, at the top of execute(), never deeper inside arch dispatch.
    """
    def __init__(self, job_id, device, caps, cancel_flag, emit, pipeline_cache, mock):
        self.job_id = job_id
        self.device = device
        self.caps = caps
        self.cancel_flag = cancel_flag
        self.emit = emit
        self.pipeline_cache = pipeline_cache
        self.mock = mock

    @property
    def job_id_str(self) -> str:
        """Readable UUID string form of job_id, for logs/labels only.

        See the `job_id` attribute docstring above — `job_id` itself must
        stay raw bytes everywhere it's used as data (e.g. ctx.emit(...)
        payloads); this property exists so nodes never need to
        reimplement `uuid.UUID(bytes=...)` themselves, and never
        accidentally interpolate the raw bytes into a log message or
        cache-key label.
        """
        return str(uuid.UUID(bytes=self.job_id))


class BaseNode(ABC):
    """Abstract base class for all node types.

    Subclasses must implement execute(). Direct instantiation is
    prevented by Python's ABC machinery.
    """

    @abstractmethod
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute this node's computation.

        Subclasses override this method to perform inference or
        data transformation. The base class provides no default
        implementation — a subclass missing this method cannot be
        instantiated.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Named input values keyed by slot name,
                matching the node's INPUT_SLOTS.

        Returns:
            Dict of output values keyed by slot name,
            matching the node's OUTPUT_SLOTS.
        """
        ...
