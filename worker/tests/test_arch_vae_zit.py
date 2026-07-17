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
    """load() returns a ZiTVaeModel with meta-device parameters (zero real memory).

    Calls load() against the regular fixture with bf16=True in caps and
    asserts the returned module is a ZiTVaeModel with all parameters on
    torch.device("meta"), confirming no real memory was allocated during
    construction — the meta device means param.numel() > 0 but actual
    memory is zero.

    This is the primary test for the meta-construction contract: it proves
    the ~15 GB crash from P904 is prevented because no real memory is
    allocated when constructing the model on meta-device.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the meta device.
    for param in model.parameters():
        assert param.device.type == "meta", (
            f"expected parameter on meta device, got {param.device}"
        )

    # Assert no real memory was allocated — meta device tensors have
    # numel() > 0 but consume zero actual memory.
    total_numel = sum(p.numel() for p in model.parameters())
    assert total_numel > 0, "model should have parameters with non-zero numel"

    # Assert the selected dtype is bf16 (caps.bf16=True, native_dtype=fp32).
    # On meta device, this checks the dtype metadata, not actual tensor data.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16, (
            f"expected dtype bfloat16, got {param.dtype}"
        )

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_meta_construction_no_metadata_fixture() -> None:
    """load() against the no-metadata fixture variant succeeds with meta-device parameters.

    Calls load() against ``zit_vae_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header and uses xyz_ prefixed keys)
    and asserts it returns a valid ZiTVaeModel with meta-device parameters.

    This verifies the metadata-fallback path in _infer_hyperparams works
    correctly when exercised through the full load() pipeline.
    """
    from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, load

    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_no_metadata.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a ZiTVaeModel.
    assert isinstance(model, ZiTVaeModel)

    # Assert all parameters are on the meta device.
    for param in model.parameters():
        assert param.device.type == "meta", (
            f"expected parameter on meta device, got {param.device}"
        )

    # Assert the .arch attribute is set (even though the fixture has no
    # metadata, the model class sets it in __init__).
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"


@pytest.mark.real_mode
def test_load_dtype_selection_applied() -> None:
    """Model parameters have the dtype selected by _select_dtype() (fp32 when all caps are False).

    Calls load() with caps that select fp32 (all capability flags False)
    and asserts the model's parameters have dtype == torch.float32.
    On meta device, this checks the dtype metadata, not actual tensor data.

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

    # Assert parameters are on meta device.
    for param in model.parameters():
        assert param.device.type == "meta"

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

    On meta device, this checks the dtype metadata, not actual tensor data.
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

    # Assert parameters are on meta device.
    for param in model.parameters():
        assert param.device.type == "meta"

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

    On meta device, this checks the dtype metadata, not actual tensor data.
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

    # Assert parameters are on meta device.
    for param in model.parameters():
        assert param.device.type == "meta"

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

    On meta device, this checks the dtype metadata, not actual tensor data.
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

    # Assert parameters are on meta device.
    for param in model.parameters():
        assert param.device.type == "meta"

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

    On meta device, this checks the dtype metadata, not actual tensor data.
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

    # Assert parameters are on meta device.
    for param in model.parameters():
        assert param.device.type == "meta"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "zit_vae"
