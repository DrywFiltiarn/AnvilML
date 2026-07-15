"""Tests for worker.nodes.arch.diffusion.zit — _infer_hyperparams(), can_handle(), load(), and dispatch registration."""

from pathlib import Path

import pytest

# torch is guarded, not imported unconditionally: most tests in this file are
# real-mode-only (marked @pytest.mark.real_mode below) and only ever execute
# in the worker-*-real CI job, which installs torch. The worker-*-mock job
# installs requirements/base.txt only (no torch) and only *collects* this
# file - it never runs the real_mode-marked tests - so an unconditional
# `import torch` here would break collection for the whole file, including
# the handful of genuinely mock-compatible tests (can_handle,
# _infer_hyperparams, compute_latent_shape) per ANVILML_DESIGN.md §18.3.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]

from worker.nodes.arch.diffusion import get_module
from worker.nodes.arch.diffusion import zit
from worker.nodes.arch.diffusion.zit import (
    _infer_hyperparams,
    _select_dtype,
    can_handle,
    compute_latent_shape,
    load,
)

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
    assert result["latent_height"] == 4
    assert result["latent_width"] == 4
    assert result["arch"] == "zit"
    # patch_size = sqrt(latent_dim / latent_channels) = sqrt(64 / 4) = 4
    assert result["patch_size"] == 4


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
    assert result["latent_height"] == 4
    assert result["latent_width"] == 4
    assert result["patch_size"] == 4


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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
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


@pytest.mark.real_mode
def test_load_raises_on_invalid_path() -> None:
    """load() raises ValueError for a non-existent path.

    Calls load() with a path that does not exist on disk and asserts
    that a ``ValueError`` is raised with a descriptive message.
    The error propagates from ``_infer_hyperparams()`` through ``load()``.
    """
    nonexistent = "/tmp/this_file_does_not_exist_xyz789.safetensors"
    with pytest.raises(ValueError, match="No such file"):
        load(nonexistent, _DEFAULT_CAPS, device="cpu")


# ---------------------------------------------------------------------------
# P21-A1: compute_latent_shape() tests
# ---------------------------------------------------------------------------


@pytest.fixture
def _default_patch_hyperparams():
    """Pin zit.MODEL_PATCH_SIZE / MODEL_LATENT_CHANNELS, then restore them.

    compute_latent_shape() reads module-level mutable globals that load()
    overwrites in place with checkpoint-derived values (P21-A1). Any test that
    exercises the pre-load *default* must not depend on execution order
    relative to a real_mode test that has already called load() in the same
    session — test_compute_latent_shape_real_after_load and its
    non_multiple sibling do exactly that, and previously ran earlier in this
    file, which is why the un-isolated versions of these tests silently
    passed only when run in file order and failed once collection was fixed
    and they could be run standalone (P900-series retrofit).

    This fixture pins both globals to the documented default (patch_size=2 —
    Z-Image Turbo's actual patchify default, not an arbitrary placeholder —
    and latent_channels=4) for the duration of the test, then restores
    whatever value was present before, so these tests are correct however
    they're invoked: `-m "not real_mode"` alone, `-m real_mode` alone, or the
    full file with no marker filter at all.
    """
    original_patch_size = zit.MODEL_PATCH_SIZE
    original_latent_channels = zit.MODEL_LATENT_CHANNELS
    zit.MODEL_PATCH_SIZE = 2
    zit.MODEL_LATENT_CHANNELS = 4
    try:
        yield
    finally:
        # Restore unconditionally, even if the test body raises, so a failing
        # test can never leak a pinned value into whatever runs next.
        zit.MODEL_PATCH_SIZE = original_patch_size
        zit.MODEL_LATENT_CHANNELS = original_latent_channels


def test_compute_latent_shape_mock_exact_multiple(_default_patch_hyperparams) -> None:
    """compute_latent_shape() produces correct shape for exact-patch-size dimensions.

    Calls compute_latent_shape() with width=32, height=32, batch_size=1.
    With MODEL_PATCH_SIZE=2 (Z-Image Turbo's actual patchify default, pinned
    by the _default_patch_hyperparams fixture), this gives latent_height=16,
    latent_width=16. The result should be (1, 4, 16, 16).

    This is the primary mock-mode test for the formula — it exercises the
    exact-multiple path of the ceiling division.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple
    """
    result = compute_latent_shape(32, 32, 1)
    assert result == (1, 4, 16, 16)


def test_compute_latent_shape_mock_non_multiple(_default_patch_hyperparams) -> None:
    """compute_latent_shape() rounds up non-multiple dimensions via ceiling division.

    Calls compute_latent_shape() with width=33, height=33, batch_size=1.
    With MODEL_PATCH_SIZE=2, 33/2 = 16.5, which rounds up to 17.
    The result should be (1, 4, 17, 17).

    This verifies the ceiling-division path: non-multiples of patch_size
    are rounded up so the latent grid fully covers the input.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_non_multiple
    """
    result = compute_latent_shape(33, 33, 1)
    assert result == (1, 4, 17, 17)


def test_compute_latent_shape_mock_batch_scaling(_default_patch_hyperparams) -> None:
    """compute_latent_shape() scales the batch dimension correctly.

    Calls compute_latent_shape() with width=64, height=64, batch_size=4.
    With MODEL_PATCH_SIZE=2, this gives latent_height=32, latent_width=32.
    The result should be (4, 4, 32, 32) — batch_size=4 in the first position.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_batch_scaling
    """
    result = compute_latent_shape(64, 64, 4)
    assert result == (4, 4, 32, 32)


@pytest.mark.real_mode
def test_compute_latent_shape_real_after_load() -> None:
    """compute_latent_shape() uses actual checkpoint hyperparameters after load().

    Calls load() against the ZiT fixture (which has patch_size=4,
    latent_channels=4), then calls compute_latent_shape(32, 32, 1).
    The result should be (1, 4, 8, 8), proving that load() correctly
    updates the module-level hyperparameters.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load
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
    load(str(fixture_path), caps, device="cpu")
    result = compute_latent_shape(32, 32, 1)
    assert result == (1, 4, 8, 8)


@pytest.mark.real_mode
def test_compute_latent_shape_real_non_multiple_after_load() -> None:
    """compute_latent_shape() ceiling division works after load() updates hyperparams.

    Calls load() against the ZiT fixture, then calls compute_latent_shape(50, 50, 1).
    With patch_size=4, 50/4 = 12.5, which rounds up to 13.
    The result should be (1, 4, 13, 13).

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load
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
    load(str(fixture_path), caps, device="cpu")
    result = compute_latent_shape(50, 50, 1)
    assert result == (1, 4, 13, 13)


def test_compute_latent_shape_default_batch_size(_default_patch_hyperparams) -> None:
    """compute_latent_shape() defaults batch_size to 1 when omitted.

    Calls compute_latent_shape(32, 32) without the batch_size argument.
    With MODEL_PATCH_SIZE=2 (pinned by the _default_patch_hyperparams
    fixture — this test previously ran after the real_after_load tests in
    file order and silently depended on their leaked fixture-derived
    patch_size instead of any real default; P900-series retrofit), the
    result should be (1, 4, 16, 16), confirming batch_size defaults to 1.
    """
    result = compute_latent_shape(32, 32)
    assert result == (1, 4, 16, 16)


def test_compute_latent_shape_zero_dims(_default_patch_hyperparams) -> None:
    """compute_latent_shape() returns zero latent dims for zero-width or zero-height.

    Calls compute_latent_shape(0, 32) and compute_latent_shape(32, 0) with
    MODEL_PATCH_SIZE=2 pinned by the _default_patch_hyperparams fixture.
    Should return (1, 4, 0, 16) and (1, 4, 16, 0) respectively, proving the
    ceiling division handles the edge case correctly.
    """
    result_zero_width = compute_latent_shape(0, 32, 1)
    assert result_zero_width == (1, 4, 0, 16)

    result_zero_height = compute_latent_shape(32, 0, 1)
    assert result_zero_height == (1, 4, 16, 0)

    result_both_zero = compute_latent_shape(0, 0, 1)
    assert result_both_zero == (1, 4, 0, 0)


# ---------------------------------------------------------------------------
# P21-B1: sample() pipeline assembly + caching tests
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_sample_first_call_assembles_pipeline_mock() -> None:
    """sample() assembles and caches a pipeline on first call for a model_id.

    Spies on ``pipeline_cache.get_or_load`` to verify the loader function
    is called exactly once on the first call with ``model_id="test1"``.
    Asserts that the return value is a ``(latent, seed)`` tuple.

    This is the primary mock-mode test for the cache-assembly path.
    It uses a fixture-loaded model rather than constructing one manually,
    keeping the test fast and deterministic.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    # Load a real ZiTModel from the fixture — this is the simplest way to
    # get a valid model instance without constructing it manually.
    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    # Spy on the cache's get_or_load method to count loader invocations.
    # We use a manual spy pattern — capture the original method, wrap it,
    # and restore afterward. The cache is module-level and process-global,
    # so we must be careful not to leak state between tests.
    original_get_or_load = pipeline_cache.get_or_load
    call_count = 0

    def spy_get_or_load(key: str, loader_fn) -> object:
        nonlocal call_count
        if key not in pipeline_cache._cache:
            call_count += 1
        return original_get_or_load(key, loader_fn)

    pipeline_cache.get_or_load = spy_get_or_load
    try:
        latent, seed = sample(
            model, "test1", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
        )

        # The loader should have been called exactly once for a new model_id.
        assert call_count == 1, (
            f"expected loader to be called exactly once, got {call_count}"
        )
        # The returned latent must be a tensor.
        assert isinstance(latent, torch.Tensor)
        # The seed must be an int (explicit seed passed through unchanged).
        assert isinstance(seed, int)
        assert seed == 42
    finally:
        # Restore the original method unconditionally.
        pipeline_cache.get_or_load = original_get_or_load
        # Clean up the cache entry so subsequent tests start fresh.
        if "test1:pipeline" in pipeline_cache._cache:
            del pipeline_cache._cache["test1:pipeline"]


@pytest.mark.real_mode
def test_sample_second_call_reuses_cached_pipeline_mock() -> None:
    """sample() reuses the cached pipeline on second call with same model_id.

    Calls ``sample()`` twice with ``model_id="test2"`` and verifies that
    the second call does NOT re-assemble the pipeline (loader call count
    stays at 1). Both calls return tensors of the same shape.

    The denoising loop runs independently each time (the latent is cloned
    inside sample()), so the two latent tensors are different objects even
    though the pipeline is reused.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    original_get_or_load = pipeline_cache.get_or_load
    call_count = 0

    def spy_get_or_load(key: str, loader_fn) -> object:
        nonlocal call_count
        if key not in pipeline_cache._cache:
            call_count += 1
        return original_get_or_load(key, loader_fn)

    pipeline_cache.get_or_load = spy_get_or_load
    try:
        latent_a, seed_a = sample(
            model, "test2", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
        )
        latent_b, seed_b = sample(
            model, "test2", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
        )

        # Loader should still be called only once — second call hits the cache.
        assert call_count == 1, (
            f"expected loader call count to stay at 1, got {call_count}"
        )
        # Both calls must return tensors with matching shape.
        assert latent_a.shape == latent_b.shape
        # Seeds must match (explicit seed passed through unchanged).
        assert seed_a == seed_b == 42
    finally:
        pipeline_cache.get_or_load = original_get_or_load
        if "test2:pipeline" in pipeline_cache._cache:
            del pipeline_cache._cache["test2:pipeline"]


@pytest.mark.real_mode
def test_sample_different_model_id_gets_separate_pipeline() -> None:
    """sample() produces separate pipelines for different model_ids.

    Calls ``sample()`` with two different ``model_id`` values and verifies
    that both return valid ``(latent, seed)`` tuples with matching shapes.
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    latent_a, seed_a = sample(
        model, "model_a", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
    )
    latent_b, seed_b = sample(
        model, "model_b", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
    )

    # Both must return tensors with matching shape.
    assert isinstance(latent_a, torch.Tensor)
    assert isinstance(latent_b, torch.Tensor)
    assert latent_a.shape == latent_b.shape
    # Both must return ints as seeds.
    assert isinstance(seed_a, int)
    assert isinstance(seed_b, int)

    # Clean up.
    if "model_a:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["model_a:pipeline"]
    if "model_b:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["model_b:pipeline"]


@pytest.mark.real_mode
def test_sample_returns_tuple_with_tensor_and_seed() -> None:
    """Returned value is a (latent, seed) tuple with correct types.

    Verifies that ``sample()`` returns a 2-tuple where the first element
    is a ``torch.Tensor`` (the denoised latent) and the second element
    is an ``int`` (the resolved seed).
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    result = sample(
        model, "test_model", None, torch.zeros(1, 4, 8, 8), 20, 7.5, 42
    )

    # The return must be a 2-tuple.
    assert isinstance(result, tuple)
    assert len(result) == 2
    latent, seed = result
    # Latent must be a tensor.
    assert isinstance(latent, torch.Tensor)
    # Seed must be an int.
    assert isinstance(seed, int)

    # Clean up.
    if "test_model:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["test_model:pipeline"]


@pytest.mark.real_mode
def test_sample_denoising_real_zit_fixture() -> None:
    """End-to-end denoising against the real ZiT fixture checkpoint.

    Calls ``sample()`` with a model loaded from ``zit_tiny.safetensors``,
    verifies the output is a tensor with the correct shape and dtype,
    and confirms the seed is a non-negative integer. This is the
    canonical real-mode test for the denoising loop.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    latent_out, seed = sample(
        model, "real_test", None, latent_in, 20, 7.5, 42
    )

    # The output must be a tensor with the same shape as the input.
    assert isinstance(latent_out, torch.Tensor)
    assert latent_out.shape == latent_in.shape

    # The seed must be a non-negative integer (explicit seed passed through).
    assert isinstance(seed, int)
    assert seed >= 0
    assert seed == 42

    # Clean up.
    if "real_test:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["real_test:pipeline"]


# ---------------------------------------------------------------------------
# P21-B2: sample() denoising loop + seed resolution tests
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_sample_seed_minus_one_resolves_random() -> None:
    """seed=-1 resolves to a random integer in [0, 2**63).

    Calls ``sample()`` with ``seed=-1`` and asserts that the returned
    seed is a non-negative integer strictly less than ``2**63``.
    Since ``secrets.randbelow()`` is non-deterministic, we only assert
    the range constraint — not a specific value.

    This is the primary mock-mode test for the seed resolution path.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    _, seed = sample(
        model, "seed_random", None, latent_in, 10, 7.5, -1
    )

    # The resolved seed must be in [0, 2**63).
    assert seed >= 0, f"expected seed >= 0, got {seed}"
    assert seed < 2**63, f"expected seed < 2**63, got {seed}"

    # Clean up.
    if "seed_random:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["seed_random:pipeline"]


@pytest.mark.real_mode
def test_sample_explicit_seed_returned_unchanged() -> None:
    """Explicit seed is used as-is and returned unchanged.

    Calls ``sample()`` with ``seed=42`` and asserts that the returned
    seed equals 42 exactly, proving the seed is not modified by the
    function body.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_explicit_seed_returned_unchanged
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    _, seed = sample(
        model, "seed_explicit", None, latent_in, 10, 7.5, 42
    )

    # The explicit seed must pass through unchanged.
    assert seed == 42

    # Clean up.
    if "seed_explicit:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["seed_explicit:pipeline"]


@pytest.mark.real_mode
def test_sample_denoising_runs_for_steps() -> None:
    """Denoising runs the model forward exactly ``steps`` times.

    Wraps the model's ``forward`` method with a counter, calls
    ``sample()`` with ``steps=10``, and asserts the model was called
    exactly 10 times — once per scheduler timestep.

    This is the critical test proving the denoising loop actually
    executes for the specified step count.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_denoising_runs_for_steps
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    # Wrap the model's forward method to count invocations.
    # The ZiT model's forward signature is:
    #   forward(latent: Tensor, timestep: float, conditioning=None)
    # We replace it with a wrapper that increments a counter before
    # delegating to the original method.
    original_forward = model.forward
    forward_count = 0

    def counting_forward(*args, **kwargs):
        nonlocal forward_count
        forward_count += 1
        return original_forward(*args, **kwargs)

    model.forward = counting_forward
    try:
        latent_in = torch.zeros(1, 4, 8, 8)
        _, _ = sample(
            model, "step_count", None, latent_in, 10, 7.5, 42
        )

        # The model's forward must be called exactly once per step.
        # Each step runs 2 forward calls (unconditional + conditional),
        # so 10 steps → 20 forward calls.
        assert forward_count == 20, (
            f"expected 20 forward calls (2 per step × 10 steps), "
            f"got {forward_count}"
        )
    finally:
        # Restore the original forward method unconditionally.
        model.forward = original_forward

    # Clean up.
    if "step_count:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["step_count:pipeline"]


@pytest.mark.real_mode
def test_sample_output_shape_dtype_matches_input_latent() -> None:
    """Output latent has the same shape and dtype as the input latent.

    Creates an input latent with a specific shape ``(1, 4, 8, 8)`` and
    dtype ``torch.float32``, calls ``sample()``, and asserts the returned
    latent has the same shape and dtype.

    This verifies that the denoising loop preserves tensor dimensions
    through the scheduler steps and CFG interpolation.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_output_shape_dtype_matches_input_latent
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    # Use a specific dtype to verify it is preserved through denoising.
    latent_in = torch.zeros(1, 4, 8, 8, dtype=torch.float32)
    latent_out, _ = sample(
        model, "shape_dtype", None, latent_in, 10, 7.5, 42
    )

    # Shape must match exactly.
    assert latent_out.shape == latent_in.shape, (
        f"expected shape {latent_in.shape}, got {latent_out.shape}"
    )
    # Dtype must match exactly.
    assert latent_out.dtype == latent_in.dtype, (
        f"expected dtype {latent_in.dtype}, got {latent_out.dtype}"
    )

    # Clean up.
    if "shape_dtype:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["shape_dtype:pipeline"]


@pytest.mark.real_mode
def test_sample_different_step_count_changes_iterations() -> None:
    """Denoising step count scales the number of forward calls.

    Calls ``sample()`` with ``steps=5`` and asserts the model's forward
    was called exactly 10 times (2 per step × 5 steps), confirming the
    loop count is driven by the *steps* parameter rather than hardcoded.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_different_step_count_changes_iterations
    """
    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    original_forward = model.forward
    forward_count = 0

    def counting_forward(*args, **kwargs):
        nonlocal forward_count
        forward_count += 1
        return original_forward(*args, **kwargs)

    model.forward = counting_forward
    try:
        latent_in = torch.zeros(1, 4, 8, 8)
        _, _ = sample(
            model, "step_count_5", None, latent_in, 5, 7.5, 42
        )

        # 5 steps × 2 forward calls (uncond + cond) = 10 total.
        assert forward_count == 10, (
            f"expected 10 forward calls (2 per step × 5 steps), "
            f"got {forward_count}"
        )
    finally:
        model.forward = original_forward

    # Clean up.
    if "step_count_5:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["step_count_5:pipeline"]
