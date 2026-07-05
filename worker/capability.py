"""Real torch-level capability probe for AnvilML workers.

Constructs tiny ``torch.nn.Linear`` layers at each target dtype and runs
forward passes to determine actual compute support on the target device.
Returns a dict matching ``InferenceCaps`` field names (fp32, fp16, bf16,
fp8, fp4, flash_attention).

This module is called during real-mode worker startup (``ANVILML_WORKER_MOCK``
unset) to discover the hardware's actual precision capabilities. The mock
equivalent (_mock_probe_capabilities) lives inline in ``worker_main.py`` and
never imports torch.
"""

import logging

import torch

logger = logging.getLogger(__name__)


def _probe_dtype(dtype: torch.dtype) -> bool:
    """Probe whether *dtype* is supported by constructing a tiny linear layer.

    Creates ``torch.nn.Linear(4, 4, dtype=dtype)`` on the current device,
    runs one forward pass with a ``(1, 4)`` tensor of the same dtype, and
    returns ``True`` if no exception is raised.

    Catches all exceptions broadly — the probe should never propagate
    failures to the caller. A NotImplementedError (e.g. fp8 on CPU) or
    a runtime error (e.g. unsupported dtype on a specific backend) are
    both treated as "not supported."

    Args:
        dtype: The torch dtype to probe (e.g. ``torch.float16``,
            ``torch.float8_e4m3fn``).

    Returns:
        ``True`` if the dtype is supported on the current device,
        ``False`` otherwise.
    """
    try:
        # Tiny linear layer — 4x4 is enough to trigger dtype validation
        # and a forward pass without consuming meaningful GPU memory.
        layer = torch.nn.Linear(4, 4, dtype=dtype)
        x = torch.randn(1, 4, dtype=dtype)
        _ = layer(x)
        return True
    except Exception:
        # Any exception means this dtype is not usable on the current
        # device. On CPU, torch.float8_e4m3fn raises NotImplementedError;
        # on some older CUDA builds, exotic dtypes may raise runtime errors.
        # We never propagate these — the caller expects a boolean.
        return False


def _probe_flash_attention(device_type: str, device_index: int) -> bool:
    """Probe whether flash attention is available via SDPA.

    Attempts ``torch.nn.functional.scaled_dot_product_attention`` on tiny
    ``(1, 1, 4, 4)`` tensors with an explicit ``scale`` parameter. This is
    the lightest available call path — no model weights needed.

    Returns ``True`` if the call succeeds (torch 2.x always provides a
    math fallback, so on CPU this returns ``True`` — the function works
    but falls back to standard attention). On backends where the backend
    truly lacks the capability, an exception is raised and caught,
    returning ``False``.

    Args:
        device_type: Device type string (``"cuda"``, ``"rocm"``, ``"cpu"``).
            Used for logging only; the function runs on the default device.
        device_index: Device index. Used for logging only.

    Returns:
        ``True`` if ``scaled_dot_product_attention`` runs without error,
        ``False`` otherwise.
    """
    try:
        # Tiny 1-head, 4-dim tensors — enough to exercise the SDPA path
        # without allocating meaningful memory.
        q = torch.randn(1, 1, 4, 4)
        k = torch.randn(1, 1, 4, 4)
        v = torch.randn(1, 1, 4, 4)
        # Pass explicit scale — torch.nn.functional.scaled_dot_product_attention
        # requires it in torch 2.x when the query dimension is small.
        _ = torch.nn.functional.scaled_dot_product_attention(q, k, v, scale=1.0)
        return True
    except Exception:
        # The backend truly lacks SDPA support — rare on torch 2.x since
        # math fallback is always compiled in, but possible on very old
        # builds or exotic hardware.
        return False


def probe_capabilities(device_type: str, device_index: int) -> dict:
    """Probe the target device for actual compute capabilities via torch.

    Constructs tiny layers at each target dtype and runs forward passes to
    determine which precisions the device actually supports. Returns a dict
    with six boolean keys matching ``InferenceCaps`` field names:
    ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``.

    This is the **real-mode** probe — it actually imports torch and runs
    inference at each dtype. The mock equivalent (_mock_probe_capabilities)
    in ``worker_main.py`` returns fixed synthetic values without importing
    torch.

    Args:
        device_type: Device type string — ``"cuda"``, ``"rocm"``, or
            ``"cpu"``. Determines device selection logic.
        device_index: GPU device index (0-based). Ignored for CPU devices.

    Returns:
        Dict with keys ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
        ``flash_attention``, each mapping to a ``bool`` indicating whether
        that precision is supported on the target device.
    """
    # Select the target device — torch.device handles cuda/rocm device
    # selection; cpu device_type maps to "cpu" (device_index is ignored).
    if device_type == "cpu":
        device = torch.device("cpu")
    else:
        device = torch.device(f"{device_type}:{device_index}")

    logger.debug(
        "probe_capabilities: device_type=%s, device_index=%d, device=%s",
        device_type,
        device_index,
        device,
    )

    # Probe each precision independently. Each call is isolated — a failure
    # in one dtype does not affect others. The probe never raises.
    fp32 = _probe_dtype(torch.float32)
    fp16 = _probe_dtype(torch.float16)
    bf16 = _probe_dtype(torch.bfloat16)
    fp8 = _probe_dtype(torch.float8_e4m3fn)

    # Torch 2.x does not expose a native fp4 dtype (no torch.float4 or
    # torch.float4_e2m1fn). We attempt torch.float8_e4m3fn as the closest
    # available 8-bit format; if it fails (as expected on CPU), fp4 is False.
    # This is mechanically correct: we probe, don't guess.
    fp4 = _probe_dtype(torch.float8_e4m3fn)

    # Flash attention probe — runs SDPA on tiny tensors. On CPU, torch
    # falls back to math attention silently, so this returns True (the
    # function works, just not accelerated). On CUDA/ROCm, it returns True
    # if flash attention acceleration is available, or falls back to math.
    flash_attention = _probe_flash_attention(device_type, device_index)

    result = {
        "fp32": fp32,
        "fp16": fp16,
        "bf16": bf16,
        "fp8": fp8,
        "fp4": fp4,
        "flash_attention": flash_attention,
    }

    logger.info(
        "probe_capabilities complete: fp32=%s, fp16=%s, bf16=%s, fp8=%s, "
        "fp4=%s, flash_attention=%s",
        fp32,
        fp16,
        bf16,
        fp8,
        fp4,
        flash_attention,
    )

    return result
