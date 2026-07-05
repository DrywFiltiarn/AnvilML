"""Entry point for the Python worker process.

This module provides the mock-mode capability probe used during worker startup
when ``ANVILML_WORKER_MOCK=1`` is set. The real-mode startup path (torch
import, real hardware probe in ``capability.py``) is the default branch;
mock mode is the explicit alternate that never imports torch.

The mock probe returns fixed synthetic capability values that represent
what a GPU-capable device would report — they are device-agnostic defaults,
not a CPU simulation.
"""

import logging
import os
import sys

logger = logging.getLogger(__name__)


def _real_startup_sequence() -> dict:
    """Run the real-mode startup sequence: IPC connect → torch import → device select → probe.

    This implements the real-mode startup path per ``ANVILML_DESIGN.md §14.2``.
    It reads environment variables injected by the Rust supervisor, connects
    to the IPC ROUTER socket, imports torch, selects the target device, and
    runs the real torch-level capability probe.

    The torch import happens inside this function (not at module level) so
    that importing ``worker_main`` does not transitively pull in torch —
    mock-mode tests and CI jobs can import this module without torch installed.

    Args:
        None — all configuration comes from environment variables:
            ANVILML_IPC_PORT, ANVILML_WORKER_ID,
            ANVILML_DEVICE_TYPE, ANVILML_DEVICE_INDEX.

    Returns:
        Dict with keys ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
        ``flash_attention``, each mapping to a ``bool`` indicating whether
        that precision is supported on the target device.

    Raises:
        KeyError: If any required environment variable is not set.
        RuntimeError: If IPC connection fails.
    """
    # Read IPC connection parameters from environment — these are injected
    # by the Rust supervisor's WorkerEnv at process launch time.
    port = int(os.environ["ANVILML_IPC_PORT"])
    worker_id = os.environ["ANVILML_WORKER_ID"]
    device_type = os.environ["ANVILML_DEVICE_TYPE"]
    device_index = int(os.environ["ANVILML_DEVICE_INDEX"])

    # Connect to the Rust supervisor's ROUTER socket before anything else;
    # all subsequent IPC operations (send_event, recv_message) require this.
    import worker.ipc as ipc

    ipc.connect(port, worker_id)

    # Import torch inside the function body — this ensures the import
    # only happens when real-mode startup actually runs, not at module
    # import time. Mock-mode tests can import worker_main without torch.
    import torch  # noqa: F401 (used for torch.cuda.set_device below)

    # CPU devices have no per-device selection; torch uses "cpu" implicitly.
    # For CUDA/ROCm, we must call set_device to select the target GPU index.
    if device_type != "cpu":
        torch.cuda.set_device(device_index)

    # Run the real torch-level capability probe — this actually constructs
    # tiny layers at each dtype and runs forward passes to determine support.
    import worker.capability as capability

    caps = capability.probe_capabilities(device_type, device_index)

    # Log the probe result at DEBUG level — operators need this for
    # diagnosing precision-capability mismatches on target hardware.
    logger.debug(
        "real_startup: device_type=%s, caps.fp32=%s, caps.fp16=%s, caps.bf16=%s",
        device_type,
        caps["fp32"],
        caps["fp16"],
        caps["bf16"],
    )

    return caps


def _mock_probe_capabilities() -> dict:
    """Return fixed synthetic capability values matching ``InferenceCaps`` fields.

    This is the mock-mode equivalent of ``capability.probe_capabilities()``.
    It returns a dict with six boolean keys matching the ``InferenceCaps``
    struct field names: ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
    ``flash_attention``.

    The function never imports ``torch`` — it is a pure Python function
    returning a static dict literal, enabling mock-mode CI jobs to run
    without any GPU driver or torch installation.

    The values represent what a GPU-capable device would report:

    * ``fp32``, ``fp16``, ``bf16`` are ``True`` — universally supported on
      modern GPUs.
    * ``fp8`` is ``True`` — supported on GPU hardware (Ampere+ / CDNA+).
      While the real probe returns ``fp8=False`` on CPU (because
      ``torch.float8_e4m3fn`` raises on CPU), the mock is a synthetic
      baseline representing GPU capability, not a CPU simulation.
    * ``fp4`` is ``False`` — Torch 2.x has no native ``torch.float4`` dtype,
      so ``fp4`` is universally unsupported on all current torch builds.
    * ``flash_attention`` is ``True`` — on modern torch builds,
      ``scaled_dot_product_attention`` is always available (at minimum via
      the math fallback), so this is correctly ``True`` for the synthetic
      baseline.

    Returns:
        Dict with keys ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
        ``flash_attention``, each mapping to a ``bool``.
    """
    # All capabilities except fp4 are True — this represents a GPU-capable
    # device baseline. fp4 is False because Torch 2.x has no native float4
    # dtype; no torch build supports it.
    return {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": True,
        "fp4": False,
        "flash_attention": True,
    }


if __name__ == "__main__":
    caps = _real_startup_sequence()
    print(caps)
    sys.exit(0)
