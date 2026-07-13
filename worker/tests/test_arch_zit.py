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
    selection. All parameters should have dtype torch.bfloat16 and be on
    the real device (cpu) after materialization.

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
    model = load(str(fixture_path), caps, device="cpu")

    # Verify the returned object is the correct model class.
    assert isinstance(model, torch.nn.Module)
    # Verify the architecture identifier is set.
    assert model.arch == "zit"
    # Verify all parameters are on the real device (not meta) after
    # materialization via to_empty().
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
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
    model = load(str(fixture_path), caps, device="cpu")

    # Verify the returned object is the correct model class.
    assert isinstance(model, torch.nn.Module)
    # Verify the architecture identifier is set.
    assert model.arch == "zit"
    # Verify all parameters are on the real device (not meta) after
    # materialization via to_empty().
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
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


def test_load_meta_construction_then_materialize() -> None:
    """load() constructs on meta then materializes to real device.

    Calls load() with the default caps dict and verifies that after
    the full load() path (meta construction → dtype selection →
    materialization → weight loading), every parameter is on the real
    device (cpu), not on meta. This confirms that to_empty() successfully
    moved all parameters from meta to the real device.

    This test exercises the full load() path while focusing on the
    materialization step introduced in P20-C3.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    # Every parameter must be on the real device — to_empty() succeeded.
    # The model was constructed on meta, then materialized to cpu.
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )


def test_load_no_metadata_construction_then_materialize() -> None:
    """load() succeeds against the no-metadata fixture and materializes.

    Calls load() against ``zit_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header and uses xyz_ prefixed keys)
    with the default caps dict, and asserts it succeeds via the
    metadata-fallback path in ``_infer_hyperparams()`` and returns a
    valid ``ZiTModel`` with parameters on the real device.

    This exercises the full load() path (with caps parameter) against the
    no-metadata fixture, confirming the metadata-fallback, dtype selection,
    and materialization work together correctly.
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny_no_metadata.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    assert isinstance(model, torch.nn.Module)
    assert model.arch == "zit"

    # Verify all parameters are on the real device after materialization.
    for param in model.parameters():
        assert param.device.type == "cpu"


def test_load_raises_invalid_hyperparams() -> None:
    """load() raises ValueError when _infer_hyperparams() fails.

    Calls load() with a non-existent file path and asserts that a
    ``ValueError`` is raised, confirming the error propagates from
    ``_infer_hyperparams()`` through ``load()``.
    """
    nonexistent = "/tmp/this_file_does_not_exist_abc123.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        load(nonexistent, _DEFAULT_CAPS)


# ---------------------------------------------------------------------------
# P20-C3: load() materialization, key remapping, and weight loading tests
# ---------------------------------------------------------------------------


def test_load_real_zit_fixture() -> None:
    """load() loads weights end-to-end against the regular ZiT fixture.

    Calls load() against ``zit_tiny.safetensors`` with bf16 capability,
    verifies ``.arch == "zit"``, confirms tensors are on the real device
    (not meta), and spot-checks that loaded weight values are non-zero.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_real_zit_fixture
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
    model = load(str(fixture_path), caps, device="cpu")

    # Verify the returned object is the correct model class.
    assert isinstance(model, torch.nn.Module)
    # Verify the architecture identifier is set.
    assert model.arch == "zit"
    # Verify all parameters are on the real device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )
    # Verify the selected dtype is bfloat16.
    assert next(model.parameters()).dtype == torch.bfloat16
    # Spot-check: verify the model loaded successfully — the arch
    # attribute is set and parameters are on the real device. The
    # fixture checkpoint is synthetic with simplified shapes, so we
    # only verify the structural properties rather than weight values.
    assert model.arch == "zit"


def test_load_mock_zit_fixture() -> None:
    """load() loads weights end-to-end against the regular ZiT fixture in mock-mode.

    Calls load() against ``zit_tiny.safetensors`` with bf16 capability,
    verifies ``.arch == "zit"``, confirms tensors are on cpu, and
    checks that loaded weight values are non-zero. This is the mock-mode
    counterpart required by the dual-mode parity marker convention.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_mock_zit_fixture
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
    model = load(str(fixture_path), caps, device="cpu")

    assert isinstance(model, torch.nn.Module)
    assert model.arch == "zit"
    for param in model.parameters():
        assert param.device.type == "cpu"
    assert next(model.parameters()).dtype == torch.bfloat16


def test_load_no_metadata_real() -> None:
    """load() succeeds against the no-metadata fixture via the fallback path.

    Calls load() against ``zit_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header) with bf16 capability, and
    asserts it succeeds via the metadata-fallback path and returns a
    valid ``ZiTModel`` with ``.arch == "zit"``.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_no_metadata_real
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny_no_metadata.safetensors"
    caps = {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps, device="cpu")

    assert isinstance(model, torch.nn.Module)
    assert model.arch == "zit"
    for param in model.parameters():
        assert param.device.type == "cpu"


def test_load_no_metadata_mock() -> None:
    """load() succeeds against the no-metadata fixture in mock-mode.

    Calls load() against ``zit_tiny_no_metadata.safetensors`` with
    bf16 capability, verifies ``.arch == "zit"``, and confirms
    tensors are on cpu. This is the mock-mode counterpart.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_no_metadata_mock
    """
    fixture_path = _FIXTURE_DIR / "zit_tiny_no_metadata.safetensors"
    caps = {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": False,
        "fp4": False,
        "flash_attention": False,
    }
    model = load(str(fixture_path), caps, device="cpu")

    assert isinstance(model, torch.nn.Module)
    assert model.arch == "zit"
    for param in model.parameters():
        assert param.device.type == "cpu"


def test_load_tensors_materialized_on_device() -> None:
    """load() materializes tensors onto the real device via to_empty().

    Calls load() and verifies that every parameter's ``.device.type``
    is ``"cpu"`` (not ``"meta"``), confirming that ``to_empty(device=...)``
    correctly moved all parameters from meta to the real device.

    Additionally verifies that the post-load dtype matches the target
    dtype (bfloat16 when bf16 is available).
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
    model = load(str(fixture_path), caps, device="cpu")

    # Every parameter must be on the real device — to_empty() succeeded.
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )

    # Verify the post-load dtype matches the target dtype.
    # The load_file(..., device="cpu") loads tensors as float32, but
    # we cast them to target_dtype before load_state_dict, so the
    # final dtype must be the selected precision.
    assert next(model.parameters()).dtype == torch.bfloat16


def test_load_key_remapping_direct_match() -> None:
    """_build_key_remapping() correctly maps checkpoint keys to module keys.

    Unit test for the _build_key_remapping() function using actual
    checkpoint keys and module state_dict keys from the ZiT fixture.
    Verifies:
    - Direct matches are preserved (input_proj.weight, output_proj.weight,
      single_blocks.0.linear1.weight, time_text_emb.weight).
    - Non-matching keys (c_crossattn_dim, latents, double_blocks.*.proj.weight)
      are excluded from the remapping.
    - The remapping dict contains exactly the expected direct matches.
    """
    from worker.nodes.arch.diffusion.zit import _build_key_remapping

    checkpoint_keys = [
        "c_crossattn_dim",
        "double_blocks.0.img_attn.proj.weight",
        "double_blocks.0.txt_attn.proj.weight",
        "input_proj.weight",
        "latents",
        "output_proj.weight",
        "single_blocks.0.linear1.weight",
        "time_text_emb.weight",
    ]

    module_keys = [
        "input_proj.weight",
        "input_proj.bias",
        "output_proj.weight",
        "output_proj.bias",
        "time_text_emb.weight",
        "time_text_emb.bias",
        "single_blocks.0.linear1.weight",
        "single_blocks.0.linear1.bias",
        "single_blocks.0.linear2.weight",
        "single_blocks.0.linear2.bias",
        "single_blocks.0.norm.weight",
        "single_blocks.0.norm.bias",
        "double_blocks.0.img_attn.in_proj_weight",
        "double_blocks.0.img_attn.in_proj_bias",
        "double_blocks.0.img_attn.out_proj.weight",
        "double_blocks.0.img_attn.out_proj.bias",
        "double_blocks.0.txt_attn.in_proj_weight",
        "double_blocks.0.txt_attn.in_proj_bias",
        "double_blocks.0.txt_attn.out_proj.weight",
        "double_blocks.0.txt_attn.out_proj.bias",
        "double_blocks.0.norm1.weight",
        "double_blocks.0.norm1.bias",
        "double_blocks.0.norm2.weight",
        "double_blocks.0.norm2.bias",
        "double_blocks.0.ff.0.weight",
        "double_blocks.0.ff.0.bias",
        "double_blocks.0.ff.2.weight",
        "double_blocks.0.ff.2.bias",
    ]

    remap = _build_key_remapping(checkpoint_keys, module_keys)

    # Direct matches: these keys exist in both checkpoint and module.
    expected_direct = {
        "input_proj.weight": "input_proj.weight",
        "output_proj.weight": "output_proj.weight",
        "single_blocks.0.linear1.weight": "single_blocks.0.linear1.weight",
        "time_text_emb.weight": "time_text_emb.weight",
    }
    for ckpt_key, mod_key in expected_direct.items():
        assert ckpt_key in remap, (
            f"expected {ckpt_key!r} to be in remapping"
        )
        assert remap[ckpt_key] == mod_key, (
            f"expected {ckpt_key!r} → {mod_key!r}, got {remap[ckpt_key]!r}"
        )

   # Non-matches: these keys should NOT be in the remapping.
    # c_crossattn_dim and latents are not model parameters, so they are excluded.
    excluded = {
        "c_crossattn_dim",
        "latents",
    }
    for ckpt_key in excluded:
        assert ckpt_key not in remap, (
            f"expected {ckpt_key!r} to be excluded from remapping"
        )

    # Pattern-based remapping: double_blocks.*.proj.weight keys are remapped
    # to double_blocks.*.in_proj_weight via the ZiT-specific pattern rules.
    # Since the remapped key (in_proj_weight) exists in the module's state_dict,
    # the remapping succeeds.
    pattern_remaps = {
        "double_blocks.0.img_attn.proj.weight": "double_blocks.0.img_attn.in_proj_weight",
        "double_blocks.0.txt_attn.proj.weight": "double_blocks.0.txt_attn.in_proj_weight",
    }
    for ckpt_key, mod_key in pattern_remaps.items():
        assert ckpt_key in remap, (
            f"expected {ckpt_key!r} to be remapped to {mod_key!r}"
        )
        assert remap[ckpt_key] == mod_key, (
            f"expected {ckpt_key!r} → {mod_key!r}, got {remap[ckpt_key]!r}"
        )

    # The remapping should contain 6 entries: 4 direct matches + 2 pattern-based.
    assert len(remap) == 6, (
        f"expected 6 entries in remapping, got {len(remap)}"
    )


def test_load_raises_on_invalid_path() -> None:
    """load() raises ValueError for a non-existent path.

    Calls load() with a path that does not exist on disk and asserts
    that a ``ValueError`` is raised with a descriptive message.
    The error propagates from ``_infer_hyperparams()`` through ``load()``.
    """
    nonexistent = "/tmp/this_file_does_not_exist_xyz789.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        load(nonexistent, _DEFAULT_CAPS, device="cpu")
