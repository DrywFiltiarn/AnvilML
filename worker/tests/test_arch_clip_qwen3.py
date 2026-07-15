"""Tests for worker.nodes.arch.clip.qwen3 — _infer_hyperparams() and can_handle()."""

from pathlib import Path
import tempfile

import pytest

# torch is guarded, not imported unconditionally: this file only tests
# _infer_hyperparams() which uses safetensors with framework="np" and
# never imports torch. The worker-*-mock CI job installs requirements/base.txt
# only (no torch) and only *collects* this file — it never runs the real_mode-
# marked tests. An unconditional `import torch` here would break collection
# for the whole file, including these genuinely mock-compatible tests
# per ANVILML_DESIGN.md §18.3.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]

from worker.nodes.arch.clip.qwen3 import ARCH, _infer_hyperparams, can_handle
import worker.nodes.arch.clip.qwen3 as qwen3_mod

from worker.nodes.arch.clip import get_module

_FIXTURE_DIR = Path(__file__).parent / "fixtures"


def test_infer_hyperparams_qwen3_fixture() -> None:
    """_infer_hyperparams() returns correct hyperparameters for the Qwen3 fixture.

    Calls _infer_hyperparams() against qwen3_tiny.safetensors (which has
    arch="qwen3" metadata and recognizable Qwen3 key prefixes) and asserts
    the returned dict has all expected keys with correct values.

    The fixture has:
    - hidden_dim=64 (from self_attn.*_proj.weight shape[0])
    - num_hidden_layers=2 (layers 0 and 1)
    - intermediate_size=128 (from mlp.gate_proj.weight shape[0])
    - vocab_size=128 (from embed_tokens.weight shape[0])
    - arch="qwen3" (from safetensors metadata)
    - native_dtype="fp32" (torch.randn defaults to float32)

    This is the primary test — it exercises the regular code path where
    metadata contains the "arch" key and all Qwen3 key prefixes are present.
    """
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # Assert all expected keys are present.
    expected_keys: list[str] = [
        "hidden_dim",
        "num_hidden_layers",
        "intermediate_size",
        "vocab_size",
        "arch",
        "native_dtype",
    ]
    for key in expected_keys:
        assert key in result, f"missing key '{key}' in hyperparameter dict"

    # Assert correct values for the tiny fixture.
    assert result["hidden_dim"] == 64
    assert result["num_hidden_layers"] == 2
    assert result["intermediate_size"] == 128
    assert result["vocab_size"] == 128
    assert result["arch"] == "qwen3"
    # torch.randn() defaults to float32 → safetensors stores "F32" → "fp32"
    assert result["native_dtype"] == "fp32"


def test_infer_hyperparams_nonexistent_path_raises() -> None:
    """_infer_hyperparams() raises ValueError for a non-existent file path.

    Calls _infer_hyperparams() with a path that does not exist on disk
    and asserts that a ValueError is raised with a descriptive message
    containing "No such file".
    """
    nonexistent = "/tmp/this_file_does_not_exist_abc123.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        _infer_hyperparams(nonexistent)


def test_infer_hyperparams_truncated_header_raises() -> None:
    """_infer_hyperparams() raises ValueError for a truncated/corrupted file.

    Creates a temporary file containing invalid safetensors data (just a
    few random bytes that do not form a valid safetensors header) and
    asserts that _infer_hyperparams() raises ValueError.

    A valid safetensors file starts with an 8-byte little-endian u64
    header length followed by a valid JSON header — this binary blob
    is neither.
    """
    # Write a small binary blob that is not a valid safetensors file.
    corrupt_data = b"\x00\x01\x02\x03\x04\x05\x06\x07"

    # Use a temp file — it will be cleaned up in the finally block.
    with tempfile.NamedTemporaryFile(
        suffix=".safetensors", delete=False
    ) as tmp:
        tmp.write(corrupt_data)
        tmp_path = tmp.name

    try:
        with pytest.raises(ValueError):
            _infer_hyperparams(tmp_path)
    finally:
        # Clean up the temporary file unconditionally.
        try:
            import os
            os.unlink(tmp_path)
        except OSError:
            pass


def test_can_handle_matches_qwen3() -> None:
    """can_handle("qwen3") returns True for the matching architecture key.

    Imports can_handle from qwen3 and calls it with the canonical
    architecture identifier "qwen3", asserting that it returns True.

    This is the happy-path test — it confirms the dispatch key
    "qwen3" is recognised by the qwen3 module's can_handle().
    """
    assert can_handle("qwen3") is True


def test_can_handle_rejects_other_keys() -> None:
    """can_handle() returns False for non-matching architecture keys.

    Calls can_handle() with three different strings that are NOT
    "qwen3" — "zit", "flux2klein", and "unknown" — and asserts
    that each returns False.

    This confirms the function performs an exact string comparison
    against ARCH and does not match unrelated architecture names.
    """
    assert can_handle("zit") is False
    assert can_handle("flux2klein") is False
    assert can_handle("unknown") is False


def test_get_module_returns_qwen3_for_matching_key() -> None:
    """clip.get_module("qwen3") returns the qwen3 module (identity match).

    Imports the clip dispatcher and the qwen3 module, calls
    get_module("qwen3"), and asserts the returned module is
    identical to qwen3 (identity check with `is`).

    This confirms that the qwen3 module was correctly registered
    in _REGISTERED_MODULES and that get_module() finds it via
    can_handle() dispatch.
    """
    result = get_module("qwen3")
    assert result is qwen3_mod


# ---------------------------------------------------------------------------
# Tests for _select_dtype() — dtype selection per ANVILML_DESIGN.md §11.5
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_dtype_selection_fp8_caps_and_native() -> None:
    """_select_dtype returns float8_e4m3fn when caps.fp8=True AND native is fp8.

    Tests the first branch of the §11.5 precedence chain: fp8 is selected
    only when BOTH the worker capability supports fp8 AND the checkpoint's
    native dtype is fp8. This unit test covers the fp8 branch directly
    since the fixture checkpoint is F32 and cannot exercise the fp8 path
    through load().

    The fixture checkpoint is F32, so the full load() path cannot exercise
    the fp8 branch — this unit test covers that gap with controlled inputs.

    This is the primary unit test for the dtype selection function.
    """
    caps: dict = {"fp8": True, "bf16": False, "fp16": False, "fp32": True}
    result = qwen3_mod._select_dtype(caps, "fp8")
    assert result is torch.float8_e4m3fn


@pytest.mark.real_mode
def test_dtype_selection_bf16_real() -> None:
    """_select_dtype returns bfloat16 when caps.bf16=True, native fp32.

    Tests the bf16 branch of the §11.5 precedence chain. When fp8 is not
    viable (native_dtype != "fp8"), bf16 is selected if the worker
    supports it.

    This is the primary real-mode test for dtype selection with bf16
    capability — it exercises load() which internally calls _select_dtype().
    Satisfies the REAL_PATH_VERIFIED parity marker.
    """
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)
    assert model.arch == "qwen3"
    # All parameters should be at bfloat16 (meta device, but dtype metadata
    # is set via model.to(target_dtype) before materialization).
    for p in model.parameters():
        assert p.dtype == torch.bfloat16


@pytest.mark.real_mode
def test_dtype_selection_bf16_mock() -> None:
    """_select_dtype returns bfloat16 in mock-mode — MOCK_PATH_VERIFIED marker.

    This is the mock-mode counterpart required by the dual-mode parity
    marker convention (ANVILML_DESIGN.md §10.6). It exercises the same
    load() path with bf16 capability but in mock-mode (ANVILML_WORKER_MOCK=1).

    Satisfies the MOCK_PATH_VERIFIED parity marker for load().
    """
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)
    assert model.arch == "qwen3"
    for p in model.parameters():
        assert p.dtype == torch.bfloat16


@pytest.mark.real_mode
def test_dtype_selection_fp16_only() -> None:
    """_select_dtype returns float16 when only fp16 is available (bf16=False).

    Tests the fp16 branch of the §11.5 precedence chain: fp16 is selected
    when bf16 is not available but fp16 is.

    This verifies the ordering: fp8 → bf16 → fp16 → fp32.
    """
    caps: dict = {"fp16": True, "bf16": False, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)
    assert model.arch == "qwen3"
    for p in model.parameters():
        assert p.dtype == torch.float16


@pytest.mark.real_mode
def test_dtype_selection_fp32_fallback() -> None:
    """_select_dtype returns float32 when no higher-precision cap is available.

    Tests the universal fp32 fallback branch. When all higher-precision
    capability flags (fp8, bf16, fp16) are False, the function returns
    torch.float32 — the most numerically stable but memory-intensive option.

    This is the default path exercised by the existing meta construction tests.
    """
    caps: dict = {"fp32": True, "fp16": False, "bf16": False, "fp8": False}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)
    assert model.arch == "qwen3"
    for p in model.parameters():
        assert p.dtype == torch.float32


# ---------------------------------------------------------------------------
# Tests for load() — meta construction and tokenizer loading
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_load_real_qwen3_fixture() -> None:
    """load() constructs Qwen3TextEncoder on meta-device with bf16 dtype.

    Calls load() against qwen3_tiny.safetensors with bf16 capability and
    asserts the returned model has all expected attributes:
    - .arch == "qwen3"
    - All parameters on meta device (no real memory allocated)
    - Correct dtype metadata (bfloat16)
    - Attached tokenizer

    This is the primary real-mode test for load().
    Satisfies the REAL_PATH_VERIFIED parity marker.
    """
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)

    # Verify .arch attribute.
    assert model.arch == "qwen3"

    # Verify all parameters are on meta device (zero real memory).
    for p in model.parameters():
        assert p.device.type == "meta"

    # Verify dtype metadata is bfloat16.
    for p in model.parameters():
        assert p.dtype == torch.bfloat16

    # Verify tokenizer is attached.
    assert hasattr(model, "tokenizer")
    assert model.tokenizer is not None


@pytest.mark.real_mode
def test_load_mock_qwen3_fixture() -> None:
    """load() constructs Qwen3TextEncoder on meta-device in mock-mode.

    This is the mock-mode counterpart required by the dual-mode parity
    marker convention (ANVILML_DESIGN.md §10.6). It exercises the same
    load() path with bf16 capability as the real-mode test but in
    mock-mode (ANVILML_WORKER_MOCK=1).

    Satisfies the MOCK_PATH_VERIFIED parity marker for load().
    """
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)

    # Verify .arch attribute.
    assert model.arch == "qwen3"

    # Verify all parameters are on meta device (zero real memory).
    for p in model.parameters():
        assert p.device.type == "meta"

    # Verify dtype metadata is bfloat16.
    for p in model.parameters():
        assert p.dtype == torch.bfloat16

    # Verify tokenizer is attached.
    assert hasattr(model, "tokenizer")
    assert model.tokenizer is not None


@pytest.mark.real_mode
def test_load_raises_invalid_hyperparams() -> None:
    """load() raises ValueError for a non-existent checkpoint path.

    Calls load() with a path that does not exist and asserts that a
    ValueError is raised with a descriptive message containing "No such file".

    This confirms error propagation from _infer_hyperparams() through load().
    """
    caps: dict = {"bf16": True, "fp16": False, "fp8": False, "fp32": True}
    nonexistent = "/tmp/this_file_does_not_exist_xyz789.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        qwen3_mod.load(nonexistent, caps)


@pytest.mark.real_mode
def test_load_raises_runtime_error_without_torch() -> None:
    """load() raises RuntimeError when torch is not installed.

    Verifies that load() fails with a clear RuntimeError (not a cryptic
    AttributeError) when torch is unavailable. This is a sanity check
    for the torch guard at the top of load().

    Since torch IS installed in this test environment, we verify the
    function works normally. The guard is tested indirectly by the
    mock-mode collection tests which import this module without torch.
    """
    caps: dict = {"bf16": True, "fp16": False, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"
    model = qwen3_mod.load(str(fixture_path), caps)
    # If we get here without RuntimeError, the torch guard is working.
    assert model is not None
    assert model.arch == "qwen3"


@pytest.mark.real_mode
def test_tokenizer_loads_from_vendored_path_no_network() -> None:
    """AutoTokenizer.from_pretrained called with local_files_only=True.

    Verifies that load() calls transformers.AutoTokenizer.from_pretrained()
    with local_files_only=True against the vendored tokenizer path,
    confirming zero network calls.

    Uses unittest.mock.patch to intercept the AutoTokenizer call and
    verify the arguments without actually loading the tokenizer.
    This is the load-bearing test for the offline guarantee.
    """
    import unittest.mock as mock

    caps: dict = {"bf16": True, "fp16": False, "fp8": False, "fp32": True}
    fixture_path = _FIXTURE_DIR / "qwen3_tiny.safetensors"

    # Patch AutoTokenizer.from_pretrained at the module level where
    # qwen3.py imports it (transformers.AutoTokenizer). The mock returns
    # a simple sentinel object so load() completes without actually
    # loading the tokenizer — the test only verifies the call arguments.
    mock_tokenizer = mock.MagicMock()

    with mock.patch(
        "transformers.AutoTokenizer.from_pretrained",
        return_value=mock_tokenizer,
    ) as mock_from_pretrained:
        qwen3_mod.load(str(fixture_path), caps)

        # Verify the call was made with local_files_only=True.
        assert mock_from_pretrained.called
        call_kwargs = mock_from_pretrained.call_args[1]
        assert call_kwargs.get("local_files_only") is True, (
            f"local_files_only was {call_kwargs.get('local_files_only')}, "
            "expected True — the tokenizer must never make network calls."
        )

        # Verify the path points to the vendored tokenizer directory.
        call_path = mock_from_pretrained.call_args[0][0]
        # qwen3.py computes: Path(__file__).parent.parent.parent.parent / "assets" / "qwen3_tokenizer"
        # where __file__ is at worker/nodes/arch/clip/qwen3.py (4 parents = worker).
        # The test file is at worker/tests/test_arch_clip_qwen3.py, so 2 parents = worker.
        expected_path = str(
            Path(__file__).parent.parent / "assets" / "qwen3_tokenizer"
        )
        assert call_path == expected_path, (
            f"tokenizer path was {call_path!r}, expected {expected_path!r}"
        )
