"""LoadModel node — loads a diffusion model from a safetensors file.

This module defines the ``LoadModel`` node, which accepts a ``model_id``
STRING input and outputs a ``MODEL`` slot containing either a real loaded
pipeline model object (in non-mock mode) or a lightweight ``MockModel``
sentinel (in mock mode).

The ``torch``, ``diffusers``, and ``safetensors`` packages must never be
imported at the top level of this module. Importing them here would cause
the worker to fail on systems without GPU hardware or these libraries.
Instead, any real-mode loading code must import these packages lazily
inside the non-mock code path, which is unreachable when
``ANVILML_WORKER_MOCK=1``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
from typing import Any

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

__all__ = ["LoadModel", "MockModel"]


class MockModel:
    """Sentinel model object for mock mode.

    Carries only the ``arch`` attribute so that downstream nodes
    (Sampler, etc.) can inspect the model architecture without
    needing a real diffusers pipeline object.

    Args:
        arch: The model architecture identifier (e.g. ``"zit"``).
    """

    def __init__(self, arch: str) -> None:
        """Initialise a mock model sentinel.

        Args:
            arch: The model architecture identifier.
        """
        self.arch = arch


@register
class LoadModel(BaseNode):
    """Load a diffusion model from a safetensors file.

    Accepts a ``model_id`` string input and returns a ``MODEL``
    slot containing either a real loaded pipeline component
    (in non-mock mode) or a ``MockModel`` sentinel (in mock mode).

    Attributes:
        NODE_TYPE: The type string used by the scheduler to route
            jobs to this node.
        CATEGORY: The UI category for this node type.
        DISPLAY_NAME: Human-readable name shown in UI.
        DESCRIPTION: Brief description of node behaviour.
        INPUT_SLOTS: One required ``STRING`` slot named ``"model_id"``.
        OUTPUT_SLOTS: One ``MODEL`` slot named ``"model"``.
    """

    NODE_TYPE = "LoadModel"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load Model"
    DESCRIPTION = "Load a diffusion model (UNet or DiT) from a safetensors file"
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]

    def execute(self, **inputs: Any) -> dict[str, Any]:
        """Execute the LoadModel node.

        Reads the ``model_id`` input, checks mock mode, and either
        returns a ``MockModel`` sentinel or loads a real model via
        safetensors + pipeline_cache.

        Args:
            **inputs: Must contain ``"model_id"`` — the identifier
                of the model to load.

        Returns:
            Dict with key ``"model"`` containing either a ``MockModel``
            (mock mode) or a loaded pipeline model object (real mode).

        Raises:
            NotImplementedError: If called in non-mock mode. The real
                safetensors loading path is stubbed until P18-D1.
        """
        # Read the model_id input. In mock mode this is a
        # placeholder string; in real mode it references a
        # model directory or file path registered in the model store.
        model_id = inputs.get("model_id", "")

        # Check mock mode by inspecting the environment variable.
        # This must be a runtime check (not a module-level import)
        # so that CI tests running with ANVILML_WORKER_MOCK=1
        # never touch torch/diffusers/safetensors at import time.
        if os.environ.get("ANVILML_WORKER_MOCK") == "1":
            # In mock mode, return a lightweight sentinel object
            # instead of loading a real pipeline. This keeps tests
            # fast and avoids requiring GPU hardware or torch.
            # The arch="zit" matches the Phase 018 baseline model.
            return {"model": MockModel(arch="zit")}

        # Real mode: load actual safetensors weights.
        # This path is stubbed — the real implementation will use
        # safetensors.safe_open() to read weight tensors, detect
        # architecture from metadata, and load via
        # pipeline_cache.get_or_load(). The pipeline_cache module
        # is implemented in task P18-D1.
        # TODO(P18-A1): Implement real safetensors loading path.
        raise NotImplementedError(
            "Real LoadModel path not yet implemented — "
            "use ANVILML_WORKER_MOCK=1 for testing"
        )
