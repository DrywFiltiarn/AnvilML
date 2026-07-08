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

        This is the critical correctness test: float8 matmul is not
        implemented for the CPU backend, and the probe must catch the
        resulting exception and return False. This is correct behavior —
        not a bug to "fix" — because fp8 compute is a GPU-only feature on
        current torch CPU builds.

        Note: _probe_dtype() constructs its layer/input in float32 and
        casts to the target dtype (not directly in dtype) specifically so
        this False reflects the CPU backend's genuine lack of fp8 matmul
        support — not an unrelated RNG-construction limitation that
        previously affected every backend equally, GPU included, and
        masked real fp8 hardware support on capable GPUs. See
        _probe_dtype()'s own inline comment for the full explanation.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp8"] is False, (
            "fp8 should not be supported on CPU (matmul not implemented "
            "for this backend/dtype combination is correct, not a bug)"
        )

    def test_fp4_cpu_returns_false(self) -> None:
        """fp4 probe on CPU returns False.

        Torch 2.x does not expose a native fp4 dtype. The probe attempts
        ``torch.float8_e4m3fn`` as the closest available format; on CPU
        this correctly fails (no float8 matmul kernel for this backend),
        so fp4 is correctly reported as False. This confirms the probe
        probes rather than hardcodes. See test_fp8_cpu_returns_false's
        docstring for why this is now the genuine backend limitation, not
        an artifact of how the probe tensors are constructed.
        """
        result = capability.probe_capabilities("cpu", 0)
        assert result["fp4"] is False, (
            "fp4 should not be supported (no native torch.float4 dtype; "
            "probe attempts float8_e4m3fn which has no CPU matmul kernel)"
        )

    def test_float8_tensor_construction_via_cast_does_not_raise(self) -> None:
        """Constructing a float8 tensor via cast-from-float32 succeeds.

        Regression test isolating the actual mechanism _probe_dtype() now
        relies on, independent of the full probe_capabilities() call and
        of any specific backend's matmul support. torch.randn() does not
        implement its RNG kernel for float8_e4m3fn directly on any
        backend (confirmed here on CPU) — but casting a float32 tensor to
        float8 after generation is unaffected by that limitation on any
        backend, since it's a cast, not an RNG kernel invocation. This is
        the specific step that previously caused _probe_dtype() to return
        False universally regardless of real hardware fp8 support,
        including on capable GPUs — see _probe_dtype()'s inline comment.

        This test does not (and cannot, without real GPU hardware) verify
        that fp8 matmul succeeds on a capable GPU — only that the
        construction step which used to fail everywhere now succeeds
        everywhere, letting the actual backend-dependent matmul below it
        be what determines the result.

        Preconditions: torch is importable (enforced by module-level
        importorskip).
        Expected output: No exception raised during construction or cast.
        """
        # Directly reproduces _probe_dtype()'s construction approach for
        # torch.float8_e4m3fn, without depending on the full probe or on
        # matmul actually succeeding afterward.
        x = torch.randn(1, 4, dtype=torch.float32).to(torch.float8_e4m3fn)
        assert x.dtype == torch.float8_e4m3fn, (
            "cast must actually produce a float8_e4m3fn tensor"
        )

        layer = torch.nn.Linear(4, 4, dtype=torch.float32).to(torch.float8_e4m3fn)
        assert layer.weight.dtype == torch.float8_e4m3fn, (
            "cast must actually produce float8_e4m3fn layer weights"
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
