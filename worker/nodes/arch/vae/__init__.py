"""VAE architecture family module dispatcher.

This module provides the shared dispatch mechanism for the VAE
architecture family (ZiT VAE, Flux2 VAE, etc.). Each concrete VAE
arch module defines its own ``can_handle(key)`` method; the dispatcher
scans the registered modules and returns the first match.

With zero registered modules (concrete arch modules are wired in later
phases), ``get_module`` returns ``None`` silently — never raises.

Design: ANVILML_DESIGN.md §10.4 — ``get_module`` is the ONE shared
dispatcher per family, never reimplemented per module.
"""

from __future__ import annotations

from types import ModuleType
from typing import Any

from safetensors import safe_open

from worker.nodes.arch.vae import zit_vae, flux2_vae

_REGISTERED_MODULES: list[ModuleType] = []
_REGISTERED_MODULES.append(zit_vae)
_REGISTERED_MODULES.append(flux2_vae)


def detect_arch(path: str) -> str:
    """Determine the dispatch key for a VAE checkpoint at *path*.

    VAE counterpart of ``arch.diffusion.detect_arch()`` — see that
    function's docstring for the full rationale (ANVILML_DESIGN.md §10.4's
    "metadata or a path-derived fallback" contract, previously unimplemented
    anywhere: LoadVae passed a hardcoded ``"zit_vae"`` literal to
    ``get_module()`` regardless of the checkpoint's actual architecture).

    Source: the ``"arch"`` field in the safetensors header metadata (e.g.
    ``"zit_vae"``, ``"flux2"`` — note flux2_vae.py's own ``ARCH`` constant
    is ``"flux2"``, not ``"flux2_vae"``; this function returns whatever
    string the checkpoint declares, unmodified).

    Fallback: if the metadata ``"arch"`` key is absent (the no-metadata
    regression fixtures), tries each registered module's own
    ``_infer_hyperparams(path)`` in registration order, reusing its
    already-tested key-pattern fallback — the same approach as the
    diffusion family's ``detect_arch()``. This was NOT safe until the
    P900-series retrofit fixed a separate defect: ``zit_vae.py`` and
    ``flux2_vae.py``'s ``_infer_hyperparams_inner()`` used to never raise
    for an unrecognized checkpoint, each silently defaulting to its own
    arch string instead. That made either module's own detection function
    falsely "succeed" on the other's checkpoint, which would have broken
    this exact fallback loop. Both modules now raise ``ValueError`` when
    they don't recognize the checkpoint, matching ``zit.py``/
    ``flux2klein.py``'s existing (correct) behavior.

    Args:
        path: Filesystem path to a VAE checkpoint file.

    Returns:
        The architecture string to pass to ``get_module()``.

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
    # _infer_hyperparams(), which now correctly raises ValueError instead
    # of silently defaulting (see the docstring above).
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
        "'arch' metadata key and no registered VAE module recognized "
        "its key patterns"
    )


def get_module(key: Any) -> ModuleType | None:
    """Return the first registered VAE module that handles *key*.

    Iterates over ``_REGISTERED_MODULES`` in registration order and calls
    ``module.can_handle(key)`` on each. Returns the first module whose
    ``can_handle`` returns ``True``.

    If the registry is empty (no concrete arch modules have been wired in
    yet) or no registered module handles the key, returns ``None``.

    Args:
        key: The module key to look up — typically an arch string
            read from safetensors metadata or a path-derived fallback
            such as ``"zit_vae"`` or ``"flux2_vae"``.

    Returns:
        The first matching ``ModuleType``, or ``None`` if no match.
    """
    for module in _REGISTERED_MODULES:
        # can_handle is defined by each concrete VAE arch module;
        # it returns True when the module's VAE kind matches the key.
        if module.can_handle(key):
            return module

    return None
