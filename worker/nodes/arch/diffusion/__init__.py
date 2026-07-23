"""Diffusion architecture family module dispatcher.

This module provides the shared dispatch mechanism for the diffusion
architecture family (ZiT, Flux2Klein, etc.). Each concrete diffusion
arch module defines its own ``can_handle(key)`` method; the dispatcher
scans the registered modules and returns the first match.

Currently registered modules: ``flux2klein`` and ``zit``.
With zero registered modules (concrete arch modules are wired in later
phases), ``get_module`` returns ``None`` silently — never raises.

Design: ANVILML_DESIGN.md §10.4 — ``get_module`` is the ONE shared
dispatcher per family, never reimplemented per module.
"""

from __future__ import annotations

from types import ModuleType
from typing import Any

from safetensors import safe_open

from worker.nodes.arch.diffusion import flux2klein
from worker.nodes.arch.diffusion import zit

_REGISTERED_MODULES: list[ModuleType] = []
_REGISTERED_MODULES.append(flux2klein)
_REGISTERED_MODULES.append(zit)


def detect_arch(path: str) -> str:
    """Determine the dispatch key for a diffusion checkpoint at *path*.

    This is the piece of ANVILML_DESIGN.md §10.4's contract that was never
    implemented anywhere: "key is an arch string read from safetensors
    metadata or a path-derived fallback." Prior to this function, LoadModel
    passed a hardcoded ``"zit"`` literal to ``get_module()`` instead of
    deriving it from the checkpoint — harmless while only one diffusion
    module existed, a genuine dispatch defect once a second one
    (``flux2klein``) was registered (P25-F1's Runnable Proof).

    Primary source: the ``"arch"`` field in the safetensors header
    metadata — the canonical, checkpoint-author-declared identifier. Every
    fixture in this project embeds it (see each family's
    ``build_*_fixture.py``). Reads the header only; no tensor data is
    loaded, and ``framework="np"`` keeps this torch-free, consistent with
    every arch module's own ``_infer_hyperparams()``.

    Fallback: if the metadata ``"arch"`` key is absent (the no-metadata
    regression fixtures), tries each registered module's own
    ``_infer_hyperparams(path)`` in registration order. Each module already
    implements a key-pattern fallback for exactly this case (its own
    no-metadata fixture test); reusing it here avoids duplicating
    architecture-specific heuristics in the dispatcher, and generalizes to
    any future registered module with zero changes to this function.

    Args:
        path: Filesystem path to a diffusion checkpoint file.

    Returns:
        The architecture string (e.g. ``"zit"``, ``"flux2klein"``) to pass
        to ``get_module()``.

    Raises:
        ValueError: If the file cannot be opened/parsed, or no registered
            module recognizes it via metadata or the fallback.
    """
    try:
        with safe_open(path, framework="np") as f:
            metadata = f.metadata()
    except (FileNotFoundError, OSError) as exc:
        raise ValueError(f"cannot open checkpoint at {path}: {exc}") from exc
    except Exception as exc:
        raise ValueError(
            f"cannot parse safetensors header at {path}: {exc}"
        ) from exc

    arch = metadata.get("arch") if metadata else None
    if arch:
        return arch

    # Fallback: no arch metadata — ask each registered module whether it
    # recognizes this checkpoint's key patterns via its own
    # _infer_hyperparams(), which already implements this exact fallback.
    for module in _REGISTERED_MODULES:
        infer = getattr(module, "_infer_hyperparams", None)
        if infer is None:
            continue
        try:
            hyperparams = infer(path)
        except ValueError:
            continue
        detected = hyperparams.get("arch")
        if detected:
            return detected

    raise ValueError(
        f"cannot determine architecture for checkpoint at {path}: no "
        "'arch' metadata key and no registered diffusion module "
        "recognized its key patterns"
    )


def get_module(key: Any) -> ModuleType | None:
    """Return the first registered diffusion module that handles *key*.

    Iterates over ``_REGISTERED_MODULES`` in registration order and calls
    ``module.can_handle(key)`` on each. Returns the first module whose
    ``can_handle`` returns ``True``.

    If the registry is empty (no concrete arch modules have been wired in
    yet) or no registered module handles the key, returns ``None``.

    Args:
        key: The module key to look up — typically a string identifier
            such as ``"zit"`` or ``"flux2klein"``.

    Returns:
        The first matching ``ModuleType``, or ``None`` if no match.
    """
    for module in _REGISTERED_MODULES:
        # can_handle is defined by each concrete diffusion arch module;
        # it returns True when the module's model kind matches the key.
        if module.can_handle(key):
            return module

    return None
