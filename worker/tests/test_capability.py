"""Tests for worker.capability — real torch-level capability probe.

All tests are marked ``@pytest.mark.real_mode`` because they require
a real torch import and actual forward passes. They do not run under
``ANVILML_WORKER_MOCK=1``.

``pytest.importorskip("torch")`` below is required, not decorative: the
``@pytest.mark.real_mode`` marker only filters which *test items* execute,
after collection. Collection itself still imports this module — and this
module imports ``worker.capability``, which imports torch unconditionally
at module scope (by design; see its own docstring). Without the guard
below, collection fails with ImportError in any environment lacking
torch (e.g. the mock-mode CI job), instead of cleanly skipping. Any
future test module for a real-mode-only, torch-importing unit must use
the same guard — see ``ANVILML_DESIGN.md §18.3``'s pytest marker
convention.
"""

import pytest

torch = pytest.importorskip("torch")

import worker.capability as capability


@pytest.mark.real_mode
class TestProbeDtypes:
    """Tests for individual dtype probes on CPU hardware."""

    def test_fp32_cpu_returns_true(self) -> None:
        """fp32 probe on CPU returns True.

        Verifies that the basic probe works and fp32 is universally
        supported on torch CPU builds. This is the sanity check that
        the probe infrastructure itself is functional.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp32"] is True, (
            "fp32 should be supported on all torch CPU builds"
        )

    def test_fp16_cpu_returns_true(self) -> None:
        """fp16 probe on CPU returns True.

        CPU supports fp16/bf16 on modern torch builds. This confirms
        that the probe correctly identifies fp16 as available.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp16"] is True, (
            "fp16 should be supported on torch 2.x CPU builds"
        )

    def test_bf16_cpu_returns_true(self) -> None:
        """bf16 probe on CPU returns True.

        CPU supports bfloat16 on modern torch builds. This confirms
        that the probe correctly identifies bf16 as available.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["bf16"] is True, (
            "bf16 should be supported on torch 2.x CPU builds"
        )

    def test_fp8_cpu_returns_false(self) -> None:
        """fp8 probe on CPU returns False.

        This is the critical correctness test — but not for the reason it
        might look like. As of current torch builds,
        _probe_dtype(torch.float8_e4m3fn) itself actually SUCCEEDS on CPU
        (see test_float8_tensor_construction_and_forward_succeed_on_cpu
        below) — CPU torch now executes a float8 Linear forward pass via
        internal upcast-compute-downcast emulation, not a raised exception.
        So this False comes from probe_capabilities()'s explicit
        `device_type == "cpu"` exclusion, not from _probe_dtype() catching
        a failure.

        That exclusion is deliberately NOT the per-SKU device-table
        guessing this module's docstring and ANVILML_DESIGN.md §6.6 warn
        against — it's a categorical fact about CPUs as an architecture
        class (no general-purpose CPU has dedicated fp8 execution units),
        confirmed by the emulation this test's sibling proves is what
        CPU's apparent "success" actually is. GPU is not excluded — some
        GPU SKUs genuinely have dedicated fp8 hardware and some don't,
        which real probing (not this CPU exclusion) correctly
        distinguishes per-SKU.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp8"] is False, (
            "fp8 must be False on CPU — no general-purpose CPU has "
            "dedicated fp8 execution units, even though the underlying "
            "torch op itself no longer raises (see "
            "test_float8_tensor_construction_and_forward_succeed_on_cpu)"
        )

    def test_fp4_cpu_returns_false(self) -> None:
        """fp4 probe on CPU returns False.

        Torch 2.x does not expose a native fp4 dtype; the probe attempts
        torch.float8_e4m3fn as the closest available format on GPU. On
        CPU, probe_capabilities() excludes fp4 the same way and for the
        same reason as fp8 — see test_fp8_cpu_returns_false's docstring.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp4"] is False, (
            "fp4 must be False on CPU — no native torch.float4 dtype, "
            "and the float8_e4m3fn fallback is excluded on CPU for the "
            "same categorical reason fp8 is"
        )

    def test_float8_tensor_construction_and_forward_succeed_on_cpu(self) -> None:
        """A float8 Linear forward pass genuinely succeeds on CPU.

        This is the evidence behind probe_capabilities()'s explicit CPU
        exclusion for fp8/fp4 (see test_fp8_cpu_returns_false's docstring)
        — not a bug report. Constructing in float32 and casting to
        float8_e4m3fn via .to(dtype) (rather than constructing directly
        in dtype, which fails on kaiming_uniform_'s RNG kernel — a
        separate, unrelated limitation) lets the actual forward pass run,
        and on current torch builds that forward pass does not raise on
        CPU. Confirmed identically across two independent real
        environments (native ROCm-torch's CPU fallback on Windows, and a
        separate pure-CPU torch build under WSL with zero ROCm
        involvement) — this is a property of the torch version itself,
        not an artifact of either environment.

        Because torch does not expose "ran via internal upcast emulation"
        vs. "ran on genuine dedicated hardware" as a distinguishable
        result at the Python level for this op, _probe_dtype()'s
        try/except structurally cannot tell them apart — which is exactly
        why probe_capabilities() excludes CPU explicitly rather than
        trusting this probe's result for fp8/fp4 specifically.

        Preconditions: torch is importable (enforced by module-level
        importorskip).
        Expected output: No exception raised during construction, cast,
        or the forward pass itself.
        """
        # Directly reproduces _probe_dtype()'s construction approach for
        # torch.float8_e4m3fn, independent of probe_capabilities()'s own
        # CPU exclusion, to isolate and prove the underlying fact that
        # exclusion depends on.
        layer = torch.nn.Linear(4, 4, dtype=torch.float32).to(torch.float8_e4m3fn)
        x = torch.randn(1, 4, dtype=torch.float32).to(torch.float8_e4m3fn)
        assert layer.weight.dtype == torch.float8_e4m3fn, (
            "cast must actually produce float8_e4m3fn layer weights"
        )
        assert x.dtype == torch.float8_e4m3fn, (
            "cast must actually produce a float8_e4m3fn input tensor"
        )

        out = layer(x)
        assert out.dtype == torch.float8_e4m3fn, (
            "forward pass must actually succeed and return float8_e4m3fn "
            "output on CPU — confirming _probe_dtype() alone cannot "
            "distinguish this from genuine hardware fp8 support, which "
            "is why probe_capabilities() excludes CPU explicitly instead"
        )


@pytest.mark.real_mode
class TestProbeFlashAttention:
    """Tests for the flash attention probe on CPU hardware."""

    def test_flash_attention_cpu_returns_true(self) -> None:
        """Flash attention probe on CPU returns True.

        ``torch.nn.functional.scaled_dot_product_attention`` works on
        CPU — it falls back to standard math attention rather than
        raising. The probe correctly returns True because the function
        executes successfully (even though acceleration is not available).

        Note: on CPU this means the probe returns True for
        flash_attention — the function works, just not with flash
        attention acceleration. On CUDA/ROCm, the same probe would
        return True if flash attention acceleration is available,
        or True (via math fallback) if it is not.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["flash_attention"] is True, (
            "SDPA works on CPU (falls back to math); probe returns True"
        )


@pytest.mark.real_mode
class TestProbeStructure:
    """Tests for the overall structure and resilience of probe_capabilities."""

    def test_returns_dict_with_exactly_six_bool_keys(self) -> None:
        """Return dict has exactly 6 keys matching InferenceCaps field names.

        Calls ``probe_capabilities("cpu", 0)`` and asserts the result is a
        dict with exactly 6 keys matching the ``InferenceCaps`` struct
        field names (``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
        ``flash_attention``), and all values are ``bool`` type.
        """
        result = capability.probe_capabilities("cpu", 0)

        expected_keys = {
            "fp32",
            "fp16",
            "bf16",
            "fp8",
            "fp4",
            "flash_attention",
        }
        assert set(result.keys()) == expected_keys, (
            f"Expected keys {expected_keys}, got {set(result.keys())}"
        )
        assert isinstance(result, dict), "Result must be a dict"

        for key, value in result.items():
            assert isinstance(value, bool), (
                f"Value for key {key!r} must be bool, got {type(value).__name__}"
            )

    def test_never_raises_for_cpu(self) -> None:
        """probe_capabilities(\"cpu\", 0) never raises any exception.

        The probe must be resilient on CPU — no matter what dtypes are
        available or unavailable, the function must return a dict and
        never propagate an exception to the caller.
        """
        # This test passes if no exception is raised.
        result = capability.probe_capabilities("cpu", 0)
        assert isinstance(result, dict), (
            "probe_capabilities must return a dict, never raise"
        )

    def test_device_selection_cpu(self) -> None:
        """CPU device is correctly selected (device_index ignored).

        Verifies that when ``device_type="cpu"``, the function does not
        raise and returns a valid result. The device_index parameter is
        ignored for CPU devices (no GPU to select), but the function
        must not fail.
        """
        # Call with device_index=0 — CPU ignores it, should not raise.
        result = capability.probe_capabilities("cpu", 0)
        assert isinstance(result, dict), (
            "CPU device selection must not raise"
        )
        assert len(result) == 6, (
            "CPU probe must return exactly 6 capability keys"
        )

    def test_device_selection_rocm_does_not_raise(self) -> None:
        """device_type="rocm" is translated to a valid torch.device string.

        Regression test: torch.device() only recognizes "cuda" as a valid
        backend string — "rocm" is not in its accepted set, confirmed by
        the exact RuntimeError it previously raised: "Expected one of cpu,
        cuda, ipu, xpu, ... at start of device string: rocm". ROCm-built
        PyTorch exposes AMD GPUs through the same cuda API/device
        namespace via HIP's compatibility layer, so probe_capabilities()
        must translate "rocm" -> "cuda" locally when constructing the
        torch.device object.

        This runs correctly even on CPU-only CI torch: torch.device()
        object construction never touches actual hardware (that only
        happens on later tensor operations, and the constructed `device`
        object here is used for logging only, never passed into the dtype
        probes below it) — so this test verifies the string-construction
        fix specifically, independent of whether real ROCm/CUDA hardware
        is present.

        Preconditions: torch is importable (enforced by module-level
        importorskip).
        Expected output: No exception; a valid 6-key capability dict.
        """
        result = capability.probe_capabilities("rocm", 0)
        assert isinstance(result, dict), (
            "rocm device selection must not raise "
            "(previously: RuntimeError, 'rocm' is not a valid torch "
            "device-type string)"
        )
        assert len(result) == 6, (
            "rocm probe must return exactly 6 capability keys"
        )

    def test_fp8_probe_still_called_for_non_cpu_device_type(self, monkeypatch) -> None:
        """The CPU exclusion for fp8/fp4 does not leak into non-CPU paths.

        Monkeypatches _probe_dtype() to a spy that always returns True and
        records every dtype it was called with, then calls
        probe_capabilities() with device_type="rocm". Asserts
        torch.float8_e4m3fn was actually probed (called at least twice —
        once for fp8, once for fp4's fallback) and that the resulting
        dict reports fp8=True, fp4=True — proving GPU device types still
        go through the real probe rather than being short-circuited the
        way CPU now deliberately is.

        Preconditions: torch is importable (enforced by module-level
        importorskip). No real GPU required — _probe_dtype() itself is
        replaced, so this tests probe_capabilities()'s control flow only.
        Expected output: fp8/fp4 both True; float8_e4m3fn actually probed.
        """
        calls = []

        def spy_probe_dtype(dtype):
            calls.append(dtype)
            return True

        monkeypatch.setattr(capability, "_probe_dtype", spy_probe_dtype)

        result = capability.probe_capabilities("rocm", 0)

        assert result["fp8"] is True, (
            "non-CPU device types must use the real probe result, not "
            "the CPU-only exclusion"
        )
        assert result["fp4"] is True, (
            "non-CPU device types must use the real probe result, not "
            "the CPU-only exclusion"
        )
        assert calls.count(torch.float8_e4m3fn) >= 2, (
            f"expected float8_e4m3fn to be probed for both fp8 and fp4 "
            f"on a non-CPU device_type, got calls={calls}"
        )
