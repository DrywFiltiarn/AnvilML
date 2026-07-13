"""Tests for worker.nodes.arch.diffusion.zit — _infer_hyperparams(), can_handle(), and dispatch registration."""

from pathlib import Path

import pytest

from worker.nodes.arch.diffusion import get_module
from worker.nodes.arch.diffusion import zit
from worker.nodes.arch.diffusion.zit import _infer_hyperparams, can_handle

_FIXTURE_DIR = Path(__file__).parent / "fixtures"


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

    # Assert all expected keys are present.
    expected_keys: list[str] = [
        "hidden_dim",
        "double_block_count",
        "single_block_count",
        "latent_channels",
        "latent_height",
        "latent_width",
        "patch_size",
        "arch",
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
