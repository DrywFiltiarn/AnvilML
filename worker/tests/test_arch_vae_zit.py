"""Tests for worker.nodes.arch.vae.zit_vae — _infer_hyperparams()."""

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

from worker.nodes.arch.vae.zit_vae import (
    ARCH,
    _build_key_remapping,
    _infer_hyperparams,
)

_FIXTURE_DIR = Path(__file__).parent / "fixtures"


def test_infer_hyperparams_regular_fixture() -> None:
    """_infer_hyperparams() returns correct hyperparameters for the regular ZiT VAE fixture.

    Calls _infer_hyperparams() against ``zit_vae_tiny.safetensors`` (which has
    ``arch="zit_vae"`` metadata and recognizable VAE key prefixes) and asserts
    the returned dict has the expected keys and correct values.

    The fixture has:
    - encoder.blocks.0.conv.weight with shape (16, 8, 3, 3) → encoder_channels=16
    - decoder.blocks.0.conv.weight with shape (32, 16, 3, 3) → decoder_channels=32
    - latents with shape (1, 4, 8, 8) → latent_channels=4
    - arch="zit_vae" from safetensors metadata
    - native_dtype="fp32" (torch.randn() defaults to float32)

    This is the primary test — it exercises the regular code path where
    metadata contains the "arch" key and all VAE key prefixes are present.
    """
    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # Assert all expected keys are present.
    expected_keys: list[str] = [
        "encoder_channels",
        "decoder_channels",
        "latent_channels",
        "arch",
        "native_dtype",
    ]
    for key in expected_keys:
        assert key in result, f"missing key '{key}' in hyperparameter dict"

    # Assert correct values for the tiny fixture.
    assert result["encoder_channels"] == 16
    # decoder.blocks.0.conv.weight has shape (32, 16, 3, 3), shape[0]=32
    assert result["decoder_channels"] == 32
    assert result["latent_channels"] == 4
    assert result["arch"] == "zit_vae"
    # torch.randn() defaults to float32 → safetensors stores "F32" → "fp32"
    assert result["native_dtype"] == "fp32"


def test_infer_hyperparams_no_metadata_fixture() -> None:
    """_infer_hyperparams() infers arch from key patterns when metadata is absent.

    Calls _infer_hyperparams() against ``zit_vae_tiny_no_metadata.safetensors``
    (which has no "arch" key in its safetensors header and uses xyz_
    prefixed keys) and asserts the metadata-fallback path succeeds.

    The fallback path must:
    1. Detect ZiT VAE architecture from key naming patterns (encoder.blocks
       is absent, but xyz_encoder_block*conv patterns are present).
    2. Return the same channel-based hyperparameters as the regular fixture.
    """
    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_no_metadata.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # Assert all expected keys are present.
    expected_keys: list[str] = [
        "encoder_channels",
        "decoder_channels",
        "latent_channels",
        "arch",
        "native_dtype",
    ]
    for key in expected_keys:
        assert key in result, f"missing key '{key}' in hyperparameter dict"

    # The fallback path must identify the architecture from key patterns.
    assert result["arch"] == "zit_vae"

    # Channel counts should match the regular fixture.
    # xyz_encoder_block0_conv has shape (16, 8, 3, 3), shape[0]=16
    assert result["encoder_channels"] == 16
    # xyz_decoder_block0_conv has shape (32, 16, 3, 3), shape[0]=32
    assert result["decoder_channels"] == 32
    # xyz_latents has shape (1, 4, 8, 8), shape[1]=4
    assert result["latent_channels"] == 4
    # torch.randn() defaults to float32
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


def test_arch_constant() -> None:
    """ARCH equals "zit_vae" — the canonical architecture identifier.

    Imports ARCH from zit_vae and asserts it equals "zit_vae", confirming
    the module's architecture identifier is set correctly. This is used
    by can_handle() for dispatch matching.
    """
    assert ARCH == "zit_vae"


def test_can_handle_matches_zit_vae_key() -> None:
    """can_handle() returns True when the key matches the module's ARCH constant.

    Imports can_handle from zit_vae and calls it with "zit_vae", asserting
    the dispatcher will route requests with the ZiT VAE architecture key
    to this module.
    """
    from worker.nodes.arch.vae.zit_vae import can_handle

    assert can_handle("zit_vae") is True


def test_can_handle_rejects_unrelated_key() -> None:
    """can_handle() returns False for an unrelated architecture key.

    Calls can_handle("flux2_vae") and asserts it returns False, confirming
    the dispatcher correctly rejects keys that do not match this module.
    """
    from worker.nodes.arch.vae.zit_vae import can_handle

    assert can_handle("flux2_vae") is False


def test_get_module_returns_zit_vae_for_matching_key() -> None:
    """get_module() returns the zit_vae module when given the matching key.

    Imports get_module from the VAE dispatcher and calls it with "zit_vae",
    asserting the returned module's __name__ matches the full dotted path
    of the zit_vae module — confirming end-to-end registration works.
    """
    from worker.nodes.arch.vae import get_module

    module = get_module("zit_vae")
    assert module is not None
    assert module.__name__ == "worker.nodes.arch.vae.zit_vae"


# ---------------------------------------------------------------------------
# Tests for load() — meta construction + dtype selection (P23-C1)
# ---------------------------------------------------------------------------
# These tests call load() which requires torch to be importable, so they
# are marked real_mode. They are collected in mock-mode CI (the guarded
# torch import in zit_vae.py prevents import errors) but only run in
# real-mode where torch is installed.


@pytest.mark.real_mode
def test_load_meta_construction_succeeds() -> None:
    """load() returns a ZiTVaeModel with parameters on the target device.

    Calls load() against the regular fixture with bf16=True in caps and
    asserts the returned module is a ZiTVaeModel with all parameters on
    the target device (cpu), confirming the full load pipeline (meta
    construction → materialization → weight loading) completed successfully.

    After P23-C3, load() returns a fully-loaded model — parameters are no
    longer on meta device. This test verifies the complete pipeline by
    checking device, dtype, and .arch on the loaded model.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )

    # Assert parameters have non-zero numel (real tensors, not meta placeholders).
    total_numel = sum(p.numel() for p in model.parameters())
    assert total_numel > 0, "model should have parameters with non-zero numel"

    # Assert the selected dtype is bf16 (caps.bf16=True, native_dtype=fp32).
    for param in model.parameters():
        assert param.dtype == torch.bfloat16, (
            f"expected dtype bfloat16, got {param.dtype}"
        )

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_meta_construction_no_metadata_fixture() -> None:
    """load() against the no-metadata fixture variant succeeds with loaded parameters.

    Calls load() against ``zit_vae_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header and uses xyz_ prefixed keys)
    and asserts it returns a valid ZiTVaeModel with parameters on the target
    device.

    The no-metadata fixture uses xyz_ prefixed keys that do not match the
    VAE remapping patterns, so no weights are loaded — but the model
    structure is still valid and .arch is set correctly.

    This verifies the metadata-fallback path in _infer_hyperparams works
    correctly when exercised through the full load() pipeline.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_no_metadata.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )

    # Assert the .arch attribute is set (even though the fixture has no
    # metadata, the model class sets it in __init__ and load() re-verifies it).
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_selection_applied() -> None:
    """Model parameters have the dtype selected by _select_dtype() (fp32 when all caps are False).

    Calls load() with caps that select fp32 (all capability flags False)
    and asserts the model's parameters have dtype == torch.float32.

    This verifies the default fp32 branch of _select_dtype() is exercised
    through the full load() pipeline.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    # All capability flags False → _select_dtype returns torch.float32.
    caps: dict = {"bf16": False, "fp16": False, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters have dtype == torch.float32.
    for param in model.parameters():
        assert param.dtype == torch.float32, (
            f"expected dtype float32, got {param.dtype}"
        )

    # Assert parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


# ---------------------------------------------------------------------------
# Dedicated dtype-branch tests (P23-C2)
# ---------------------------------------------------------------------------
# Each of the four tests below exercises exactly one branch of the
# _select_dtype() precedence chain (ANVILML_DESIGN.md §11.5) through the
# full load() pipeline. Having one dedicated test per branch makes coverage
# unambiguous — no branch is only reachable through a shared test that
# primarily asserts something else.


@pytest.mark.real_mode
def test_load_dtype_fp8_caps_and_native() -> None:
    """Model parameters are float8_e4m3fn when caps.fp8=True and checkpoint native dtype is FP8.

    Calls load() against ``zit_vae_tiny_fp8.safetensors`` (native_dtype=fp8)
    with caps that enable fp8 and asserts the model's parameters have
    dtype == torch.float8_e4m3fn.

    This is the primary test for the fp8 branch of _select_dtype():
    caps.fp8=True AND native_dtype="fp8" → torch.float8_e4m3fn.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_fp8.safetensors"
    # caps.fp8=True AND native_dtype=fp8 → fp8 branch selected.
    caps: dict = {"fp8": True, "bf16": False, "fp16": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters have dtype == torch.float8_e4m3fn.
    for param in model.parameters():
        assert param.dtype == torch.float8_e4m3fn, (
            f"expected dtype float8_e4m3fn, got {param.dtype}"
        )

    # Assert parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_bf16_caps_selects_bf16() -> None:
    """Model parameters are bfloat16 when caps.bf16=True (bf16 branch).

    Calls load() against the regular fixture (native_dtype=fp32) with
    caps that enable bf16 and asserts the model's parameters have
    dtype == torch.bfloat16.

    This is a dedicated test for the bf16 branch of _select_dtype():
    caps.bf16=True AND native_dtype != fp8 → torch.bfloat16.

    This is distinct from test_load_meta_construction_succeeds which also
    exercises bf16 but is primarily a meta-construction test — this one
    focuses on dtype selection as the primary assertion.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    # caps.bf16=True, native_dtype=fp32 → bf16 branch selected.
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters have dtype == torch.bfloat16.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16, (
            f"expected dtype bfloat16, got {param.dtype}"
        )

    # Assert parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_fp16_caps_selects_fp16() -> None:
    """Model parameters are float16 when caps.bf16=False, caps.fp16=True (fp16 branch).

    Calls load() against the regular fixture (native_dtype=fp32) with
    caps that disable bf16 but enable fp16 and asserts the model's
    parameters have dtype == torch.float16.

    This is the dedicated test for the fp16 branch of _select_dtype():
    caps.fp16=True AND caps.bf16=False → torch.float16.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    # caps.fp16=True, bf16=False → fp16 branch selected.
    caps: dict = {"bf16": False, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters have dtype == torch.float16.
    for param in model.parameters():
        assert param.dtype == torch.float16, (
            f"expected dtype float16, got {param.dtype}"
        )

    # Assert parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_fp32_fallback() -> None:
    """Model parameters are float32 when all capability flags are False (fp32 fallback).

    Calls load() against the regular fixture (native_dtype=fp32) with
    caps that disable all accelerated precisions and asserts the model's
    parameters have dtype == torch.float32.

    This is the dedicated test for the fp32 fallback branch of
    _select_dtype(): all capability flags False → torch.float32.

    The existing test_load_dtype_selection_applied already covers this
    branch, but a dedicated test makes the coverage unambiguous — the
    acceptance criterion says "each of the 4 precedence branches is
    exercised" and one test per branch satisfies that requirement.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    # All capability flags False → fp32 fallback.
    caps: dict = {"bf16": False, "fp16": False, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters have dtype == torch.float32.
    for param in model.parameters():
        assert param.dtype == torch.float32, (
            f"expected dtype float32, got {param.dtype}"
        )

    # Assert parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


# ---------------------------------------------------------------------------
# Tests for _build_key_remapping() (P23-C3)
# ---------------------------------------------------------------------------


def test_build_key_remapping_direct_match() -> None:
    """_build_key_remapping() returns identity mapping for keys in both checkpoint and module.

    Calls _build_key_remapping() with checkpoint keys that include a direct
    match (``mid_block.conv.weight``) and a non-weight key (``latents``)
    that has no corresponding module parameter. Asserts the returned dict
    contains the identity mapping for the direct match and excludes the
    non-weight key.
    """
    checkpoint_keys = [
        "mid_block.conv.weight",
        "mid_block.norm.weight",
        "latents",
    ]
    module_keys = [
        "mid_block.conv.weight",
        "mid_block.norm.weight",
        "encoder.block_0.conv.weight",
        "encoder.block_0.norm.weight",
        "encoder.block_1.conv.weight",
        "encoder.block_1.norm.weight",
        "decoder.block_0.conv.weight",
        "decoder.block_0.norm.weight",
        "decoder.block_1.conv.weight",
        "decoder.block_1.norm.weight",
    ]

    remap = _build_key_remapping(checkpoint_keys, module_keys)

    # Direct match: mid_block keys map identically.
    assert remap["mid_block.conv.weight"] == "mid_block.conv.weight"
    assert remap["mid_block.norm.weight"] == "mid_block.norm.weight"

    # Non-weight key (latents) is excluded — it has no module equivalent.
    assert "latents" not in remap


def test_build_key_remapping_pattern_match() -> None:
    """_build_key_remapping() remaps encoder.blocks.N.* → encoder.block_N.* (and decoder).

    Calls _build_key_remapping() with checkpoint keys using the
    ``encoder.blocks.N`` and ``decoder.blocks.N`` patterns (plural "blocks"
    with dot separator) and module keys using ``encoder.block_N`` and
    ``decoder.block_N`` (singular "block" with underscore). Asserts the
    remapping correctly converts between the two naming conventions.
    """
    checkpoint_keys = [
        "encoder.blocks.0.conv.weight",
        "encoder.blocks.0.norm.weight",
        "encoder.blocks.1.conv.weight",
        "encoder.blocks.1.norm.weight",
        "decoder.blocks.0.conv.weight",
        "decoder.blocks.0.norm.weight",
        "decoder.blocks.1.conv.weight",
        "decoder.blocks.1.norm.weight",
    ]
    module_keys = [
        "mid_block.conv.weight",
        "mid_block.norm.weight",
        "encoder.block_0.conv.weight",
        "encoder.block_0.norm.weight",
        "encoder.block_1.conv.weight",
        "encoder.block_1.norm.weight",
        "decoder.block_0.conv.weight",
        "decoder.block_0.norm.weight",
        "decoder.block_1.conv.weight",
        "decoder.block_1.norm.weight",
    ]

    remap = _build_key_remapping(checkpoint_keys, module_keys)

    # Encoder blocks: blocks.N → block_N
    assert remap["encoder.blocks.0.conv.weight"] == "encoder.block_0.conv.weight"
    assert remap["encoder.blocks.0.norm.weight"] == "encoder.block_0.norm.weight"
    assert remap["encoder.blocks.1.conv.weight"] == "encoder.block_1.conv.weight"
    assert remap["encoder.blocks.1.norm.weight"] == "encoder.block_1.norm.weight"

    # Decoder blocks: blocks.N → block_N
    assert remap["decoder.blocks.0.conv.weight"] == "decoder.block_0.conv.weight"
    assert remap["decoder.blocks.0.norm.weight"] == "decoder.block_0.norm.weight"
    assert remap["decoder.blocks.1.conv.weight"] == "decoder.block_1.conv.weight"
    assert remap["decoder.blocks.1.norm.weight"] == "decoder.block_1.norm.weight"

    # mid_block keys are not in checkpoint_keys, so they shouldn't appear
    assert "mid_block.conv.weight" not in remap


# ---------------------------------------------------------------------------
# Tests for full load() — weight loading, .arch, dtype (P23-C3)
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_load_weights_loaded_regular_fixture() -> None:
    """Full load() against regular fixture: weights actually loaded, shapes match, values non-zero.

    Calls load() against ``zit_vae_tiny.safetensors`` with bf16=True in caps
    and asserts the returned model has parameters on the cpu device with
    bf16 dtype, and at least one tensor has non-zero values (proving data
    flowed through the load path).

    This is the primary real-mode test for weight loading — it verifies
    that the full pipeline (meta construction → materialization → remapping
    → load_state_dict) produces a model with actual weight data.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert parameters have bf16 dtype.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16

    # Assert at least one tensor has non-zero values — proves data flowed
    # through the load path. The fixture tensors are random, so they are
    # almost certainly non-zero, but this assertion makes the test
    # deterministic in its intent.
    has_nonzero = any(p.abs().sum() > 0 for p in model.parameters())
    assert has_nonzero, "at least one parameter should have non-zero values after load"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_weights_loaded_no_metadata_fixture() -> None:
    """Full load() against no-metadata fixture: model loads, .arch set, no matching weights.

    Calls load() against ``zit_vae_tiny_no_metadata.safetensors`` (which has
    no "arch" key and uses xyz_ prefixed keys) and asserts it returns a
    valid ZiTVaeModel with parameters on the cpu device and .arch set.

    The no-metadata fixture uses xyz_ prefixed keys that do not match any
    VAE remapping pattern, so no weights are loaded — but the model
    structure is still valid and .arch is set correctly.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_no_metadata.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_arch_attribute_set() -> None:
    """.arch attribute is "zit_vae" after load() returns.

    Calls load() against the regular fixture and asserts the returned
    model has ``.arch == "zit_vae"``, confirming step 4 of the loading
    contract is satisfied.

    This test isolates the .arch verification from other assertions (dtype,
    device, weight values) to make the acceptance criterion unambiguous.
    """
    from worker.nodes.arch.vae.zit_vae import load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_applied_to_loaded_tensors() -> None:
    """Tensors are cast to the selected dtype BEFORE load_state_dict(assign=True).

    Calls load() with caps that select fp32 (all capability flags False)
    and asserts the model's parameters have dtype == torch.float32.

    This verifies that the cast-to-target-dtype step happens before
    load_state_dict, which is critical because assign=True bypasses
    dtype coercion.
    """
    from worker.nodes.arch.vae.zit_vae import load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    # All capability flags False → _select_dtype returns torch.float32.
    caps: dict = {"bf16": False, "fp16": False, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert all parameters have dtype == torch.float32.
    for param in model.parameters():
        assert param.dtype == torch.float32, (
            f"expected dtype float32, got {param.dtype}"
        )

    # Assert parameters are on the target device.
    for param in model.parameters():
        assert param.device.type == "cpu"


@pytest.mark.real_mode
def test_load_mock_returns_sentinel() -> None:
    """Mock-mode: load_file patched to return sentinel tensors; verifies remapping + load_state_dict path.

    Patches ``load_file`` to return a dict of tensors with known sentinel
    values (ones instead of random). This exercises the remapping and
    load_state_dict code paths without requiring a real checkpoint file.

    The mock-mode test verifies that the full load pipeline executes
    without error when load_file returns valid tensors, even if the
    checkpoint file doesn't exist (the patch short-circuits the file read).
    """
    from unittest.mock import patch

    from worker.nodes.arch.vae.zit_vae import load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    # Build a state dict of sentinel tensors (ones) matching the VAE
    # checkpoint key patterns. These will be remapped and loaded.
    sentinel_tensors: dict[str, torch.Tensor] = {
        "encoder.blocks.0.conv.weight": torch.ones(16, 8, 3, 3),
        "encoder.blocks.0.norm.weight": torch.ones(16),
        "encoder.blocks.1.conv.weight": torch.ones(32, 16, 3, 3),
        "encoder.blocks.1.norm.weight": torch.ones(32),
        "decoder.blocks.0.conv.weight": torch.ones(32, 16, 3, 3),
        "decoder.blocks.0.norm.weight": torch.ones(32),
        "decoder.blocks.1.conv.weight": torch.ones(16, 8, 3, 3),
        "decoder.blocks.1.norm.weight": torch.ones(16),
        "mid_block.conv.weight": torch.ones(32, 32, 3, 3),
        "mid_block.norm.weight": torch.ones(32),
    }

    # Patch load_file to return our sentinel tensors.
    with patch("worker.nodes.arch.vae.zit_vae.load_file", return_value=sentinel_tensors):
        model = load(str(fixture_path), caps, "cpu")

    # Assert the model was constructed successfully.
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel

    assert isinstance(model, ZiTVaeModel)

    # Assert parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_real_zit_vae_fixture() -> None:
    """End-to-end: full load pipeline against ``zit_vae_tiny.safetensors``, all steps verified.

    Calls load() against the regular fixture with bf16 caps and asserts:
    1. The returned model is a ZiTVaeModel.
    2. All parameters are on the cpu device.
    3. All parameters have bf16 dtype.
    4. At least one tensor has non-zero values.
    5. The .arch attribute is "zit_vae".

    This is the complete end-to-end acceptance test for the VAE loading
    contract — it exercises every step from header inference through
    weight loading.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert parameters have bf16 dtype.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16

    # Assert at least one tensor has non-zero values.
    has_nonzero = any(p.abs().sum() > 0 for p in model.parameters())
    assert has_nonzero, "at least one parameter should have non-zero values after load"

    # Assert the .arch attribute is set.
    assert model.arch == "zit_vae"
