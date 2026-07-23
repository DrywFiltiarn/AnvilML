"""Tests for worker.nodes.arch.vae.flux2_vae — full loading contract."""

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

from worker.nodes.arch.vae.flux2_vae import (
    ARCH,
    Flux2VaeModel,
    _build_key_remapping,
    _infer_hyperparams,
)

_FIXTURE_DIR = Path(__file__).parent / "fixtures"


def test_infer_hyperparams_regular_fixture() -> None:
    """_infer_hyperparams() returns correct hyperparameters for the regular Flux 2 VAE fixture.

    Calls _infer_hyperparams() against ``flux2_vae_tiny.safetensors`` (which has
    ``arch="flux2"`` metadata and recognizable VAE key prefixes) and asserts
    the returned dict has the expected keys and correct values.

    The fixture has:
    - encoder.blocks.0.conv.weight with shape (6, 8, 3, 3) → encoder_channels=8 (shape[1])
    - decoder.blocks.0.conv.weight with shape (6, 4, 3, 3) → decoder_channels=6 (shape[0])
    - latents with shape (1, 4, 8, 8) → latent_channels=4
    - arch="flux2" from safetensors metadata
    - native_dtype="fp32" (torch.randn() defaults to float32)
    """
    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
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
    # encoder.blocks.0.conv.weight has shape (6, 8, 3, 3), shape[1]=8
    assert result["encoder_channels"] == 8
    # decoder.blocks.0.conv.weight has shape (6, 4, 3, 3), shape[0]=6
    assert result["decoder_channels"] == 6
    assert result["latent_channels"] == 4
    assert result["arch"] == "flux2"
    # torch.randn() defaults to float32 → safetensors stores "F32" → "fp32"
    assert result["native_dtype"] == "fp32"


def test_infer_hyperparams_no_metadata_fixture() -> None:
    """_infer_hyperparams() infers arch from key patterns when metadata is absent.

    Calls _infer_hyperparams() against ``flux2_vae_tiny_no_metadata.safetensors``
    (which has no "arch" key in its safetensors header and uses xyz_
    prefixed keys) and asserts the metadata-fallback path succeeds.

    The fallback path must:
    1. Detect Flux 2 VAE architecture from key naming patterns (xyz_ prefixed keys).
    2. Return the same channel-based hyperparameters as the regular fixture.
    """
    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny_no_metadata.safetensors"
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
    assert result["arch"] == "flux2"

    # Channel counts: the no-metadata fixture uses hardcoded shapes that
    # differ slightly from the regular fixture (it does not interpolate
    # channel counts across blocks). The encoder_channels and latent_channels
    # match, but decoder_channels is 4 (latent_channels) instead of 6.
    # xyz_encoder_block0_conv has shape (4, 8, 3, 3), shape[1]=8
    assert result["encoder_channels"] == 8
    # xyz_decoder_block0_conv has shape (4, 4, 3, 3), shape[0]=4
    assert result["decoder_channels"] == 4
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
    """
    corrupt_data = b"\x00\x01\x02\x03\x04\x05\x06\x07"

    with tempfile.NamedTemporaryFile(suffix=".safetensors", delete=False) as tmp:
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


def test_infer_hyperparams_rejects_zit_vae_no_metadata_fixture() -> None:
    """_infer_hyperparams() raises ValueError for an unrecognized (ZiT VAE) checkpoint.

    P900-series retrofit regression test — the Flux 2 VAE counterpart of
    ``test_arch_vae_zit.py``'s identical regression test. Prior to this
    fix, ``_infer_hyperparams_inner()`` ended with an unconditional
    "if arch is still None, default to flux2" — meaning it NEVER raised
    for an unrecognized checkpoint, and would have silently misclassified
    ``zit_vae_tiny_no_metadata.safetensors`` (a different architecture
    family, whose ``"xyz_encoder_block*_conv.weight"``-style no-metadata
    keys carry a ``.weight`` suffix that Flux 2 VAE's own no-metadata
    fixture omits) as ``"flux2"`` instead of raising. This broke
    ``worker/nodes/arch/vae/__init__.py``'s ``detect_arch()`` fallback,
    which relies on each VAE module's ``_infer_hyperparams()`` correctly
    rejecting checkpoints it doesn't recognize.

    Expected outcome: ValueError is raised, not a silently-wrong
    ``{"arch": "flux2", ...}`` result.
    """
    fixture_path = _FIXTURE_DIR / "zit_vae_tiny_no_metadata.safetensors"
    with pytest.raises(ValueError, match="unknown VAE architecture"):
        _infer_hyperparams(str(fixture_path))


def test_arch_constant() -> None:
    """ARCH equals "flux2" — the canonical architecture identifier.

    Imports ARCH from flux2_vae and asserts it equals "flux2", confirming
    the module's architecture identifier is set correctly. This is used
    by can_handle() for dispatch matching.
    """
    assert ARCH == "flux2"


def test_can_handle_matches_flux2_key() -> None:
    """can_handle() returns True when the key matches the module's ARCH constant.

    Imports can_handle from flux2_vae and calls it with "flux2", asserting
    the dispatcher will route requests with the Flux 2 VAE architecture key
    to this module.
    """
    from worker.nodes.arch.vae.flux2_vae import can_handle

    assert can_handle("flux2") is True


def test_can_handle_rejects_zit_vae_key() -> None:
    """can_handle() returns False for the zit_vae key (disambiguation).

    Calls can_handle("zit_vae") and asserts it returns False, confirming
    the dispatcher correctly rejects keys that do not match this module.
    """
    from worker.nodes.arch.vae.flux2_vae import can_handle

    assert can_handle("zit_vae") is False


def test_get_module_returns_flux2_vae_for_matching_key() -> None:
    """get_module() returns the flux2_vae module when given the matching key.

    Imports get_module from the VAE dispatcher and calls it with "flux2",
    asserting the returned module's __name__ matches the full dotted path
    of the flux2_vae module — confirming end-to-end registration works.
    """
    from worker.nodes.arch.vae import get_module

    module = get_module("flux2")
    assert module is not None
    assert module.__name__ == "worker.nodes.arch.vae.flux2_vae"


# ---------------------------------------------------------------------------
# Tests for load() — meta construction + dtype selection (P25-E1)
# ---------------------------------------------------------------------------
# These tests call load() which requires torch to be importable, so they
# are marked real_mode. They are collected in mock-mode CI (the guarded
# torch import in flux2_vae.py prevents import errors) but only run in
# real-mode where torch is installed.


@pytest.mark.real_mode
def test_load_meta_construction_succeeds() -> None:
    """load() returns a Flux2VaeModel with parameters on the target device.

    Calls load() against the regular fixture with bf16=True in caps and
    asserts the returned module is a Flux2VaeModel with all parameters on
    the target device (cpu), confirming the full load pipeline (meta
    construction → materialization → weight loading) completed successfully.
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

    # Assert all parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )

    # Assert parameters have non-zero numel (real tensors, not meta placeholders).
    total_numel = sum(p.numel() for p in model.parameters())
    assert total_numel > 0, "model should have parameters with non-zero numel"

    # Assert parameters have bf16 dtype (caps.bf16=True, native_dtype=fp32).
    for param in model.parameters():
        assert param.dtype == torch.bfloat16, (
            f"expected dtype bfloat16, got {param.dtype}"
        )

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_meta_construction_no_metadata_fixture() -> None:
    """load() against the no-metadata fixture variant succeeds with loaded parameters.

    Calls load() against ``flux2_vae_tiny_no_metadata.safetensors`` (which has
    no "arch" key in its safetensors header and uses xyz_ prefixed keys)
    and asserts it returns a valid Flux2VaeModel with parameters on the target
    device.
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny_no_metadata.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

    # Assert all parameters are on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", (
            f"expected parameter on cpu device, got {param.device}"
        )

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_dtype_fp32_fallback() -> None:
    """Model parameters are float32 when all capability flags are False (fp32 fallback).

    Calls load() with caps that select fp32 (all capability flags False)
    and asserts the model's parameters have dtype == torch.float32.

    This verifies the default fp32 branch of _select_dtype() is exercised
    through the full load() pipeline.
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    # All capability flags False → _select_dtype returns torch.float32.
    caps: dict = {"bf16": False, "fp16": False, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

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
    assert model.arch == "flux2"


# ---------------------------------------------------------------------------
# Tests for _build_key_remapping() (P25-E1)
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


# ---------------------------------------------------------------------------
# Tests for full load() — weight loading, .arch, dtype (P25-E1)
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_load_weights_loaded_regular_fixture() -> None:
    """Full load() against regular fixture: weights actually loaded, shapes match, values non-zero.

    Calls load() against ``flux2_vae_tiny.safetensors`` with bf16=True in caps
    and asserts the returned model has parameters on the cpu device with
    bf16 dtype, and at least one tensor has non-zero values (proving data
    flowed through the load path).
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

    # Assert all parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert parameters have bf16 dtype.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16

    # Assert at least one tensor has non-zero values — proves data flowed
    # through the load path.
    has_nonzero = any(p.abs().sum() > 0 for p in model.parameters())
    assert has_nonzero, "at least one parameter should have non-zero values after load"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_weights_loaded_no_metadata_fixture() -> None:
    """Full load() against no-metadata fixture: model loads, .arch set, no matching weights.

    Calls load() against ``flux2_vae_tiny_no_metadata.safetensors`` (which has
    no "arch" key and uses xyz_ prefixed keys) and asserts it returns a
    valid Flux2VaeModel with parameters on the cpu device and .arch set.
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny_no_metadata.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

    # Assert all parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert hasattr(model, "arch")
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_arch_attribute_set() -> None:
    """.arch attribute is "flux2" after load() returns.

    Calls load() against the regular fixture and asserts the returned
    model has ``.arch == "flux2"``, confirming step 4 of the loading
    contract is satisfied.
    """
    from worker.nodes.arch.vae.flux2_vae import load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    assert hasattr(model, "arch")
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_mock_returns_sentinel() -> None:
    """Mock-mode: load_file patched to return sentinel tensors; verifies remap + load_state_dict path.

    Patches ``load_file`` to return a dict of tensors with known sentinel
    values (ones instead of random). This exercises the remapping and
    load_state_dict code paths without requiring a real checkpoint file.
    """
    from unittest.mock import patch

    from worker.nodes.arch.vae.flux2_vae import load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    # Build a state dict of sentinel tensors (ones) matching the VAE
    # checkpoint key patterns. These will be remapped and loaded.
    # Shapes match the actual flux2_vae_tiny.safetensors fixture:
    # encoder: 8→6→4, decoder: 6→4→8, mid_block: 4→4.
    sentinel_tensors: dict[str, torch.Tensor] = {
        "encoder.blocks.0.conv.weight": torch.ones(6, 8, 3, 3),
        "encoder.blocks.0.norm.weight": torch.ones(6),
        "encoder.blocks.1.conv.weight": torch.ones(4, 4, 3, 3),
        "encoder.blocks.1.norm.weight": torch.ones(4),
        "decoder.blocks.0.conv.weight": torch.ones(6, 4, 3, 3),
        "decoder.blocks.0.norm.weight": torch.ones(6),
        "decoder.blocks.1.conv.weight": torch.ones(8, 4, 3, 3),
        "decoder.blocks.1.norm.weight": torch.ones(8),
        "mid_block.conv.weight": torch.ones(4, 4, 3, 3),
        "mid_block.norm.weight": torch.ones(4),
    }

    # Patch load_file to return our sentinel tensors.
    with patch(
        "worker.nodes.arch.vae.flux2_vae.load_file", return_value=sentinel_tensors
    ):
        model = load(str(fixture_path), caps, "cpu")

    # Assert the model was constructed successfully.
    assert isinstance(model, Flux2VaeModel)

    # Assert parameters are on the cpu device.
    for param in model.parameters():
        assert param.device.type == "cpu"

    # Assert the .arch attribute is set.
    assert model.arch == "flux2"


@pytest.mark.real_mode
def test_load_real_flux2_vae_fixture() -> None:
    """End-to-end: full load pipeline against ``flux2_vae_tiny.safetensors``, all steps verified.

    Calls load() against the regular fixture with bf16 caps and asserts:
    1. The returned model is a Flux2VaeModel.
    2. All parameters are on the cpu device.
    3. All parameters have bf16 dtype.
    4. At least one tensor has non-zero values.
    5. The .arch attribute is "flux2".
    """
    from worker.nodes.arch.vae.flux2_vae import Flux2VaeModel, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")

    # Assert the returned module is a Flux2VaeModel.
    assert isinstance(model, Flux2VaeModel)

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
    assert model.arch == "flux2"


# ---------------------------------------------------------------------------
# Tests for decode() — latent-to-image (P25-E1)
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_decode_real_flux2_vae_fixture() -> None:
    """decode() against a loaded fixture model produces valid PIL Images.

    Calls load() to get a Flux2VaeModel, creates a (1, 4, 8, 8) latent tensor,
    calls decode(), and asserts the result is a list of exactly 1 PIL Image
    with mode "RGB". This is the primary real-mode test for the decode()
    function — it exercises the full pipeline: forward pass, clamping,
    channel selection, and PIL image creation.
    """
    from worker.nodes.arch.vae.flux2_vae import decode, load

    fixture_path = _FIXTURE_DIR / "flux2_vae_tiny.safetensors"
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    model = load(str(fixture_path), caps, "cpu")
    latent = torch.randn(1, 4, 8, 8)

    images = decode(model, latent)

    # Assert the result is a list of exactly 1 PIL Image.
    assert isinstance(images, list)
    assert len(images) == 1

    # Assert the image has mode "RGB".
    assert images[0].mode == "RGB"

    # Assert the image dimensions match the latent spatial dimensions (8×8).
    assert images[0].size == (8, 8)


@pytest.mark.real_mode
def test_decode_mock_returns_sentinel() -> None:
    """Mock-mode path: decode() post-processing works with patched forward.

    Patches Flux2VaeModel.forward to return a sentinel tensor of known shape
    (1, 16, 8, 8), calls decode(), and asserts the result is a list of 1 PIL
    Image. This tests the post-processing path (clamp, convert, NCHW→HWC,
    PIL creation) without requiring the full forward pass.
    """
    from unittest.mock import MagicMock

    from worker.nodes.arch.vae.flux2_vae import decode

    # Create a minimal mock model — no need for a real loaded model.
    model = MagicMock()
    sentinel = torch.ones(1, 16, 8, 8)  # 16 channels, 8×8 spatial
    model.forward.return_value = sentinel
    mock_param = torch.ones(1, dtype=torch.float32)
    model.parameters.return_value = iter([mock_param])

    images = decode(model, torch.randn(1, 4, 8, 8))

    # Assert decode() produced exactly 1 PIL Image.
    assert isinstance(images, list)
    assert len(images) == 1
    assert images[0].mode == "RGB"
