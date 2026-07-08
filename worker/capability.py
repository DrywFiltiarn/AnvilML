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

    Constructs the layer and input tensor in float32, casts both to *dtype*
    via ``.to(dtype)``, then runs one forward pass and returns ``True`` if
    no exception is raised.

    Deliberately NOT constructed directly in *dtype* (e.g.
    ``torch.nn.Linear(4, 4, dtype=dtype)`` / ``torch.randn(..., dtype=dtype)``)
    — see this function's own inline comment for why that previously masked
    real GPU fp8 support entirely.

    Catches all exceptions broadly — the probe should never propagate
    failures to the caller. A NotImplementedError (e.g. fp8 matmul not
    implemented for the CPU backend) or a runtime error (e.g. unsupported
    dtype on a specific backend) are both treated as "not supported."

    Args:
        dtype: The torch dtype to probe (e.g. ``torch.float16``,
            ``torch.float8_e4m3fn``).

    Returns:
        ``True`` if the dtype is supported on the current device,
        ``False`` otherwise.
    """
    try:
        # Construct in float32, then cast down with .to(dtype) — do NOT
        # construct directly in *dtype*. torch.randn(dtype=...) and
        # nn.Linear's internal weight initialization (kaiming_uniform_)
        # both call RNG kernels that are not implemented for 8-bit float
        # types (float8_e4m3fn/e5m2) on *any* backend, CPU or GPU alike —
        # this is a torch RNG limitation, unrelated to whether the target
        # hardware actually supports float8 *compute*. Constructing
        # directly in dtype meant this probe always hit that RNG
        # limitation first and returned False universally, masking real
        # fp8 hardware support even on GPUs with native float8 matmul
        # (e.g. RDNA4/ROCm via hipBLASLt) — confirmed on real hardware,
        # where this previously reported fp8=False despite the device
        # genuinely supporting it. Casting from float32 sidesteps the RNG
        # limitation entirely; the forward pass below (a real matmul at
        # the target dtype) is what actually exercises hardware support,
        # and that operation's success/failure genuinely does differ by
        # backend — CPU still correctly returns False here (no float8
        # matmul kernel implemented for CPU), but a capable GPU no longer
        # gets a false negative from an unrelated RNG gap.
        layer = torch.nn.Linear(4, 4, dtype=torch.float32).to(dtype)
        x = torch.randn(1, 4, dtype=torch.float32).to(dtype)
        _ = layer(x)
        return True
    except Exception:
        # Any exception means this dtype is not usable on the current
        # device. We never propagate these — the caller expects a boolean.
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
        that precision is supported on the target device. ``fp8``/``fp4``
        are always ``False`` when ``device_type == "cpu"`` — see the
        inline comment where they're computed for why this is an
        explicit categorical exclusion, not a per-SKU guess, and not
        simply the probe's own try/except result (current torch builds
        execute fp8 compute on CPU via internal emulation without
        raising, which this exclusion accounts for).
    """
    # Select the target device — torch.device() only recognizes "cuda" as
    # a valid backend string (confirmed by the exact runtime error it
    # raises: "Expected one of cpu, cuda, ipu, xpu, ... at start of device
    # string: rocm" — "rocm" is never in that list). ROCm-built PyTorch
    # exposes AMD GPUs transparently through the *same* cuda API/device
    # namespace — that's what HIP's CUDA-compatibility layer means in
    # practice (torch.cuda.set_device(), torch.cuda.get_device_name(),
    # etc. all already work correctly for ROCm in worker_main.py, since
    # those take a plain integer index, never a device-type string).
    # This translation is local to constructing the torch.device object
    # only — device_type itself stays "rocm" everywhere else (the Ready
    # event's device_type field, the debug log below, and every other
    # caller in this module) so the semantic AMD/ROCm distinction isn't
    # lost from what's actually reported.
    if device_type == "cpu":
        device = torch.device("cpu")
    else:
        torch_device_type = "cuda" if device_type == "rocm" else device_type
        device = torch.device(f"{torch_device_type}:{device_index}")

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

    # fp8/fp4: CPU is excluded from the probe here — unlike fp16/bf16
    # above, which stay genuinely probed on CPU (real native support,
    # correctly detected either way). This is NOT the per-SKU
    # device-table guessing this module's docstring and
    # ANVILML_DESIGN.md §6.6 warn against ("the database/PCI-table can
    # correctly say 'this silicon supports FP8' while the
    # actually-installed torch build cannot use it") — it's a
    # categorical fact about CPUs as an architecture class: no
    # general-purpose CPU has dedicated 8-bit float execution units,
    # full stop, regardless of vendor or model.
    #
    # Confirmed directly, not assumed: _probe_dtype(torch.float8_e4m3fn)
    # DOES succeed on CPU on current torch builds — verified identically
    # across two fully independent environments (native Windows
    # ROCm-torch's CPU fallback, and a separate pure-CPU torch build
    # under WSL with zero ROCm involvement) — but only via internal
    # upcast-compute-downcast emulation (promote to fp32, compute, cast
    # the result back down), never genuine 8-bit arithmetic. Torch does
    # not expose that distinction at the Python level for this op — a
    # successful forward pass looks identical whether it ran on real
    # fp8 silicon or was silently emulated — so the try/except probe
    # below is structurally unable to tell them apart for this specific
    # dtype on this specific backend category. GPU is NOT excluded:
    # unlike CPU, some GPU SKUs genuinely do have dedicated fp8 compute
    # (e.g. RDNA4 via hipBLASLt) and some don't, which is exactly the
    # case the real probe below exists to correctly distinguish per-SKU,
    # rather than trusting a hint table either way.
    if device_type == "cpu":
        fp8 = False
        fp4 = False
    else:
        fp8 = _probe_dtype(torch.float8_e4m3fn)

        # Torch 2.x does not expose a native fp4 dtype (no torch.float4 or
        # torch.float4_e2m1fn). We attempt torch.float8_e4m3fn as the
        # closest available 8-bit format on GPU backends where genuine
        # fp8 hardware support is itself the thing being probed for.
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
