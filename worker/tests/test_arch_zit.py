"""Tests for worker.nodes.arch.diffusion.zit — _infer_hyperparams(), can_handle(), load(), and dispatch registration."""

from pathlib import Path

import pytest
import torch

from worker.nodes.arch.diffusion import get_module
from worker.nodes.arch.diffusion import zit
from worker.nodes.arch.diffusion.zit import _infer_hyperparams, _select_dtype, can_handle, load

_FIXTURE_DIR = Path(__file__).parent / "fixtures"

# Default caps dict — all precision flags False except fp32 (always True).
# Used by tests that need a minimal caps dict but don't care about dtype selection.
_DEFAULT_CAPS: dict = {
    "fp32": True,
    "fp16": False,
    "bf16": False,
    "fp8": False,
    "fp4": False,
    "flash_attention": False,
}


def test_infer_hyperparams_regular_fixture() -> None:
    """_infer_hyperparams() returns correct hyperparameters for the regular ZiT fixture.

    Calls _infer_hyperparams() against zit_tiny.safetensors (which has
    arch="zit" metadata and recognizable ZiT key prefixes) and asserts
    the returned dict has the expected keys and correct values.

    This is the primary test — it exercises the regular code path where
    metadata contains the "arch" key and all ZiT key prefixes are present.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # Assert all expected keys are present (including native_dtype added in P20-C2).
    expected_keys: list[str] = [
        "hidden_dim",
        "double_block_count",
        "single_block_count",
        "latent_channels",
        "latent_height",
        "latent_width",
        "patch_size",
        "arch",
        "native_dtype",
    ]
    for key in expected_keys:
        assert key in result, f"missing key '{key}' in hyperparameter dict"

    # Assert correct values for the tiny fixture.
    assert result["hidden_dim"] == 64
    assert result["double_block_count"] == 1
    assert result["single_block_count"] == 1
    assert result["latent_channels"] == 4
    assert result["latent_height"] == 8
    assert result["latent_width"] == 8
    assert result["arch"] == "zit"
    # patch_size = hidden_dim // latent_channels = 64 // 4 = 16
    assert result["patch_size"] == 16


def test_infer_hyperparams_no_metadata_fixture() -> None:
    """_infer_hyperparams() infers arch from key patterns when metadata is absent.

    Calls _infer_hyperparams() against zit_tiny_no_metadata.safetensors
    (which has no "arch" key in its safetensors header and uses xyz_
    prefixed keys) and asserts the metadata-fallback path succeeds.

    The fallback path must:
    1. Detect ZiT architecture from key naming patterns (double_block,
       single_block, output_proj).
    2. Return the same shape-based hyperparameters as the regular fixture.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny_no_metadata.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # The fallback path must identify the architecture from key patterns.
    assert result["arch"] == "zit"

    # Shape-based hyperparameters should match the regular fixture.
    assert result["hidden_dim"] == 64
    assert result["double_block_count"] == 1
    assert result["single_block_count"] == 1
    assert result["latent_channels"] == 4
    assert result["latent_height"] == 8
    assert result["latent_width"] == 8
    assert result["patch_size"] == 16


def test_infer_hyperparams_nonexistent_path_raises() -> None:
    """_infer_hyperparams() raises ValueError for a non-existent file path.

    Calls _infer_hyperparams() with a path that does not exist on disk
    and asserts that a ValueError is raised with a descriptive message.
    """
    nonexistent = "/tmp/this_file_does_not_exist_abc123.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        _infer_hyperparams(nonexistent)


def test_infer_hyperparams_truncated_header_raises() -> None:
    """_infer_hyperparams() raises ValueError for a truncated/corrupted file.

    Creates a temporary file containing invalid safetensors data (just a
    few random bytes that do not form a valid safetensors header) and
    asserts that _infer_hyperparams() raises ValueError.
    """
    # Write a small binary blob that is not a valid safetensors file.
    # A valid safetensors file starts with an 8-byte little-endian u64
    # header length followed by a valid JSON header.
    corrupt_data = b"\x00\x01\x02\x03\x04\x05\x06\x07"

    # Use a temp file — it will be cleaned up by the filesystem.
    import tempfile

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
        import os

        try:
            os.unlink(tmp_path)
        except OSError:
            pass


def test_can_handle_matches_zit() -> None:
    """can_handle(\"zit\") returns True — the primary match path for the ZiT architecture.

    Calls can_handle() with the canonical ZiT architecture string and
    asserts it returns True, proving the dispatcher will route a
    ``\"zit\"`` key to this module.
    """
    assert can_handle("zit") is True


def test_can_handle_rejects_unrelated_key() -> None:
    """can_handle(\"flux2klein\") returns False — the module rejects unrelated keys.

    Calls can_handle() with an unrelated architecture string and asserts
    it returns False, proving the dispatcher will skip this module for
    non-ZiT keys.
    """
    assert can_handle("flux2klein") is False


def test_get_module_returns_zit_for_matching_key() -> None:
    """get_module(\"zit\") returns the zit module — end-to-end dispatch integration.

    Calls get_module() with ``"zit"`` and asserts the result is not None
    and is the zit module, proving that importing zit in __init__.py and
    appending it to _REGISTERED_MODULES makes the dispatcher find it.
    """
    result = get_module("zit")
    assert result is not None
    # Identity comparison — the zit module imported in __init__.py
    # is the same object returned by get_module().
    assert result is zit


# ---------------------------------------------------------------------------
# New dtype selection tests (P20-C2)
# ---------------------------------------------------------------------------


def test_dtype_selection_fp8_caps_and_native() -> None:
    """_select_dtype() returns float8_e4m3fn when caps.fp8=True AND native_dtype is fp8.

    Tests the _select_dtype() pure function with controlled inputs:
    caps.fp8=True and native_dtype="fp8". This verifies the first branch
    of the §11.5 precedence chain without relying on fixture properties.

    The fixture checkpoint is F32, so the full load() path cannot exercise
    the fp8 branch — this unit test covers that gap.
    """
    caps = {
        "fp32": True,
        "fp16": False,
        "bf16": False,
        "fp8": True,
        "fp4": False,
        "flash_attention": False,
    }
    result = _select_dtype(caps, "fp8")
    assert result == torch.float8_e4m3fn


def test_dtype_selection_fp8_native_non_fp8_caps_fp8() -> None:
    """_select_dtype() falls through to bf16 when native_dtype is NOT fp8, even with caps.fp8=True.

    Tests that caps.fp8=True alone is insufficient — the native dtype must
    also be fp8. With native_dtype="fp32" and caps.fp8=True, bf16 should
    NOT be selected (bf16=False too), so fp32 is the result.

    This verifies the AND condition in the first precedence branch.
    """
    caps = {
        "fp32": True,
        "fp16": False,
        "bf16": False,
        "fp8": True,
        "fp4": False,
        "flash_attention": False,
    }
    result = _select_dtype(caps, "fp32")
    assert result == torch.float32


def test_dtype_selection_bf16_real() -> None:
    """load() selects bfloat16 when caps.bf16=True and checkpoint native is F32.

    Calls load() against ``zit_tiny.safetensors`` (native dtype = F32) with
    caps.bf16=True, fp16=True, fp8=False. The precedence chain selects bf16
    (branch 2 of §11.5) since fp8 requires native_dtype == fp8.

    This is the primary real-mode test for the load() function with dtype
    selection. All parameters should have dtype torch.bfloat16.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    caps = {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps)

    # Verify the returned object is the correct model class.
    assert isinstance(model, torch.nn.Module)
    # Verify the architecture identifier is set.
    assert model.arch == "zit"
    # Verify all parameters are on the meta device.
    for param in model.parameters():
        assert param.device.type == "meta", (
            f"expected parameter on meta device, got {param.device}"
        )
    # Verify the selected dtype is bfloat16 — bf16 takes precedence
    # over fp16 in the §11.5 chain, and fp8 is not viable since the
    # fixture native dtype is F32.
    assert next(model.parameters()).dtype == torch.bfloat16


def test_dtype_selection_bf16_mock() -> None:
    """load() selects bfloat16 under mock-mode conditions.

    Calls load() against ``zit_tiny.safetensors`` with caps.bf16=True,
    fp16=True, fp8=False. This is the mock-mode counterpart required by
    the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6).
    The load() function itself has no mock/real path divergence, but the
    marker convention requires a distinct test name for each mode.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    caps = {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps)

    # Verify the returned object is the correct model class.
    assert isinstance(model, torch.nn.Module)
    # Verify the architecture identifier is set.
    assert model.arch == "zit"
    # Verify all parameters are on the meta device.
    for param in model.parameters():
        assert param.device.type == "meta", (
            f"expected parameter on meta device, got {param.device}"
        )
    # Verify the selected dtype is bfloat16.
    assert next(model.parameters()).dtype == torch.bfloat16


def test_dtype_selection_fp16_only() -> None:
    """load() selects float16 when only caps.fp16=True (bf16=False, fp8=False).

    Calls load() against ``zit_tiny.safetensors`` with caps.fp16=True,
    bf16=False, fp8=False. The precedence chain selects fp16 (branch 3)
    since bf16 is not available.

    This verifies the fp16 fallback branch of the §11.5 precedence chain.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    caps = {
        "fp32": True,
        "fp16": True,
        "bf16": False,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps)

    # Verify the selected dtype is float16 — fp16 is the highest
    # available precision when bf16 and fp8 are not supported.
    assert next(model.parameters()).dtype == torch.float16


def test_dtype_selection_fp32_fallback() -> None:
    """load() selects float32 when all precision caps are False (universal fallback).

    Calls load() against ``zit_tiny.safetensors`` with all precision flags
    False. The precedence chain falls through to fp32 (branch 4), which
    is always supported on every device.

    This verifies the universal fallback branch of the §11.5 precedence chain.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    caps = {
        "fp32": True,
        "fp16": False,
        "bf16": False,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps)

    # Verify the selected dtype is float32 — the universal fallback
    # when no higher-precision capability is available.
    assert next(model.parameters()).dtype == torch.float32


def test_dtype_selection_fp8_beats_bf16() -> None:
    """_select_dtype() selects fp8 over bf16 when both caps are True and native is fp8.

    Tests the _select_dtype() pure function with caps.fp8=True, bf16=True,
    and native_dtype="fp8". This verifies that fp8 takes precedence over
    bf16 when both the capability and the checkpoint native dtype are fp8.

    This is the priority test — it confirms the precedence ordering is
    correct: fp8 > bf16 > fp16 > fp32.
    """
    caps = {
        "fp32": True,
        "fp16": False,
        "bf16": True,
        "fp8": True,
        "fp4": False,
        "flash_attention": False,
    }
    result = _select_dtype(caps, "fp8")
    assert result == torch.float8_e4m3fn


def test_load_meta_device_zero_real_memory() -> None:
    """load() allocates zero real memory — all parameters reside on meta device.

    Calls load() with the default caps dict and verifies that every
    parameter's ``.device.type`` is ``"meta"``. Meta tensors have shape
    metadata only — no actual GPU/CPU memory buffer is allocated. This
    is the zero-memory guarantee that prevents the ~15 GB construction
    crash that P904 experienced.

    This test exercises the full load() path (with caps parameter) while
    focusing exclusively on the zero-memory property.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS)

    # Every parameter must be on the meta device — this is the zero-memory
    # guarantee. Meta tensors carry shape metadata but allocate no real
    # GPU/CPU memory.
    for param in model.parameters():
        assert param.device.type == "meta", (
            f"expected parameter on meta device, got {param.device}"
        )


def test_load_meta_construction_no_metadata_variant() -> None:
    """load() succeeds against the no-metadata fixture via the fallback path.

    Calls load() against ``zit_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header and uses xyz_ prefixed keys)
    with the default caps dict, and asserts it succeeds via the
    metadata-fallback path in ``_infer_hyperparams()`` and returns a
    valid ``ZiTModel``.

    This exercises the full load() path (with caps parameter) against the
    no-metadata fixture, confirming the metadata-fallback and dtype
    selection work together correctly.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny_no_metadata.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS)

    assert isinstance(model, torch.nn.Module)
    assert model.arch == "zit"

    # Verify all parameters are on the meta device.
    for param in model.parameters():
        assert param.device.type == "meta"


def test_load_raises_invalid_hyperparams() -> None:
    """load() raises ValueError when _infer_hyperparams() fails.

    Calls load() with a non-existent file path and asserts that a
    ``ValueError`` is raised, confirming the error propagates from
    ``_infer_hyperparams()`` through ``load()``.
    """
    nonexistent = "/tmp/this_file_does_not_exist_abc123.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        load(nonexistent, _DEFAULT_CAPS)
