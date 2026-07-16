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
