"""Shared infrastructure for the AnvilML real-path node verification harness.

This is NOT a pytest suite. The existing worker/tests/ suite is mock-only
by design (conftest.py forces ANVILML_WORKER_MOCK=1 for every test via an
autouse fixture), so it structurally cannot exercise the real safetensors /
torch / diffusers code paths. These scripts hook directly into the worker's
node classes and arch dispatch modules the same way worker_main.py does,
run them against real model files, and print the resulting object's shape /
dtype / type so a human can eyeball whether the real path produced something
sane.

Each script in this directory is runnable standalone:

    ANVILML_MODELS_DIR=/path/to/models python3 01_loaders.py
    ANVILML_MODELS_DIR=/path/to/models python3 02_clip_encode.py
    ...

Run them in order (01 -> 05) because later scripts assume earlier ones
proved their inputs are constructible. Each script is also independently
runnable if you already know the upstream objects work.

Environment variables:
    ANVILML_MODELS_DIR   Directory containing diffusion/, text_encoders/,
                          vae/ subdirectories with real .safetensors files.
                          Defaults to "./models" (matches docs/RUNNABLE_PROOF.md
                          Phase 006 convention).
    ANVILML_ZIT_MODEL    Filename (relative to $ANVILML_MODELS_DIR/diffusion/)
                          of the ZiT FP8 diffusion transformer. Required.
    ANVILML_ZIT_VAE      Filename (relative to $ANVILML_MODELS_DIR/vae/) of
                          the ZiT-compatible VAE. Required.
    ANVILML_ZIT_CLIP     Filename (relative to $ANVILML_MODELS_DIR/text_encoders/)
                          of the Qwen3 4B text encoder safetensors. Required.
    ANVILML_DEVICE       Device string, e.g. "cuda:0" or "cpu". Defaults to
                          "cuda:0".

This harness must be run WITHOUT ANVILML_WORKER_MOCK set (or with it set to
"0"), since the entire point is to exercise the real code paths. _harness_common
asserts this on import and aborts loudly if mock mode is active, because a
silent fallback to mock sentinels would defeat the purpose and produce a
false "PASS".
"""

from __future__ import annotations

import os
import sys
import time
import traceback
from pathlib import Path
from typing import Any, Callable


# ---------------------------------------------------------------------------
# Hard guard: this harness is meaningless in mock mode.
# ---------------------------------------------------------------------------

if os.environ.get("ANVILML_WORKER_MOCK") == "1":
    print(
        "FATAL: ANVILML_WORKER_MOCK=1 is set. This harness exercises REAL\n"
        "       safetensors/torch/diffusers code paths and will silently\n"
        "       return mock sentinels if mock mode is active, producing a\n"
        "       meaningless PASS. Unset ANVILML_WORKER_MOCK and re-run.",
        file=sys.stderr,
    )
    sys.exit(2)

# Make worker.* importable when this script is run from this directory
# rather than the repo root.
_REPO_ROOT_CANDIDATES = [
    Path.cwd(),
    Path(__file__).resolve().parent,
    Path(__file__).resolve().parent.parent,
    Path(__file__).resolve().parent.parent.parent,
]
for _candidate in _REPO_ROOT_CANDIDATES:
    if (_candidate / "worker" / "nodes").is_dir():
        sys.path.insert(0, str(_candidate))
        break
else:
    print(
        "FATAL: could not locate the AnvilML repo root (a directory\n"
        "       containing worker/nodes/) from any of:\n"
        + "\n".join(f"         {c}" for c in _REPO_ROOT_CANDIDATES)
        + "\n       Run this script from inside the AnvilML repo, or set\n"
        "       PYTHONPATH to the repo root yourself.",
        file=sys.stderr,
    )
    sys.exit(2)


# ---------------------------------------------------------------------------
# Model path resolution
# ---------------------------------------------------------------------------


def models_dir() -> Path:
    """Return the configured models directory, defaulting to ./models."""
    return Path(os.environ.get("ANVILML_MODELS_DIR", "./models")).resolve()


def require_env(name: str) -> str:
    """Fetch a required env var or abort with a clear message.

    Args:
        name: Environment variable name.

    Returns:
        The variable's value.
    """
    value = os.environ.get(name)
    if not value:
        print(
            f"FATAL: required environment variable {name} is not set.\n"
            f"       See _harness_common.py module docstring for the full list.",
            file=sys.stderr,
        )
        sys.exit(2)
    return value


def zit_model_path() -> str:
    """Resolve the absolute path to the ZiT diffusion transformer file."""
    p = models_dir() / "diffusion" / require_env("ANVILML_ZIT_MODEL")
    _assert_exists(p)
    return str(p)


def zit_vae_path() -> str:
    """Resolve the absolute path to the ZiT-compatible VAE file."""
    p = models_dir() / "vae" / require_env("ANVILML_ZIT_VAE")
    _assert_exists(p)
    return str(p)


def zit_clip_path() -> str:
    """Resolve the absolute path to the Qwen3 4B text encoder file."""
    p = models_dir() / "text_encoders" / require_env("ANVILML_ZIT_CLIP")
    _assert_exists(p)
    return str(p)


def device() -> str:
    """Return the configured device string, defaulting to cuda:0."""
    return os.environ.get("ANVILML_DEVICE", "cuda:0")


def _assert_exists(p: Path) -> None:
    if not p.is_file():
        print(
            f"FATAL: model file does not exist: {p}\n"
            f"       Check ANVILML_MODELS_DIR and the filename env vars.",
            file=sys.stderr,
        )
        sys.exit(2)


# ---------------------------------------------------------------------------
# Minimal NodeContext construction matching worker_main.py's real wiring
# ---------------------------------------------------------------------------


def make_real_context(job_id: str = "harness-job") -> Any:
    """Build a NodeContext identical in shape to the one worker_main.py builds.

    Mirrors worker/worker_main.py's Execute handler exactly: cancel_flag is
    a list[bool] (NOT a threading.Event — this is deliberate, see
    KNOWN_ISSUES.md), pipeline_cache is a real PipelineCache instance, and
    emit just prints instead of going over the IPC socket.

    Returns:
        A worker.nodes.base.NodeContext instance.
    """
    from worker.nodes.base import NodeContext
    from worker.pipeline_cache import PipelineCache

    cancel_flag = [False]  # list[bool], matches worker_main.py:48 exactly

    def emit(event: dict[str, Any]) -> None:
        print(f"    [emit] {event}")

    return NodeContext(
        job_id=job_id,
        device=device(),
        cancel_flag=cancel_flag,
        emit=emit,
        pipeline_cache=PipelineCache(),
    )


# ---------------------------------------------------------------------------
# Reporting helpers
# ---------------------------------------------------------------------------

_PASS = []
_FAIL = []


def describe(obj: Any) -> str:
    """Best-effort one-line description of a node output value.

    Handles torch.Tensor (shape/dtype/device), PIL.Image, and falls back
    to type name + repr for anything else (sentinels, wrapper objects).
    """
    type_name = type(obj).__name__
    module_name = type(obj).__module__

    # torch.Tensor — the case we care most about for shape verification.
    if module_name == "torch" or "torch.Tensor" in str(type(obj)):
        try:
            return (
                f"{type_name} shape={tuple(obj.shape)} "
                f"dtype={obj.dtype} device={obj.device}"
            )
        except Exception:
            pass

    # PIL.Image
    if module_name.startswith("PIL"):
        try:
            return f"{type_name} size={obj.size} mode={obj.mode}"
        except Exception:
            pass

    # Generic object with a handful of useful attributes.
    interesting_attrs = ("shape", "dtype", "arch", "in_channels", "width",
                          "height", "batch_size")
    found = []
    for attr in interesting_attrs:
        if hasattr(obj, attr):
            try:
                found.append(f"{attr}={getattr(obj, attr)!r}")
            except Exception:
                pass
    suffix = f" ({', '.join(found)})" if found else ""
    return f"{type_name}{suffix}"


def step(name: str, fn: Callable[[], Any]) -> Any:
    """Run one harness step, print PASS/FAIL, and return the result.

    On exception, prints the full traceback (not just the message) because
    these scripts exist specifically to surface real wiring bugs — a
    truncated error is the opposite of useful here.

    Args:
        name: Human-readable step name for reporting.
        fn: Zero-argument callable to execute.

    Returns:
        The return value of fn() on success.

    Raises:
        SystemExit: never — failures are caught and recorded, not raised,
            so that one node's failure doesn't prevent the harness from
            reporting on the rest.
    """
    print(f"\n--- {name} ---")
    start = time.monotonic()
    try:
        result = fn()
        elapsed = time.monotonic() - start
        print(f"    -> {describe(result)}")
        print(f"PASS  {name}  ({elapsed:.2f}s)")
        _PASS.append(name)
        return result
    except Exception:
        elapsed = time.monotonic() - start
        print(f"FAIL  {name}  ({elapsed:.2f}s)", file=sys.stderr)
        traceback.print_exc()
        _FAIL.append(name)
        return None


def summary_and_exit() -> None:
    """Print a final PASS/FAIL summary and exit with the appropriate code."""
    print("\n" + "=" * 70)
    print(f"SUMMARY: {len(_PASS)} passed, {len(_FAIL)} failed")
    for n in _PASS:
        print(f"  PASS  {n}")
    for n in _FAIL:
        print(f"  FAIL  {n}")
    print("=" * 70)
    sys.exit(1 if _FAIL else 0)
