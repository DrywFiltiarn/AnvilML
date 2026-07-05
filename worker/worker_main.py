"""Entry point for the Python worker process.

This module provides the mock-mode capability probe used during worker startup
when ``ANVILML_WORKER_MOCK=1`` is set. The real-mode startup path (torch
import, real hardware probe in ``capability.py``) is the default branch;
mock mode is the explicit alternate that never imports torch.

The mock probe returns fixed synthetic capability values that represent
what a GPU-capable device would report — they are device-agnostic defaults,
not a CPU simulation.
"""


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
