"""Tests for worker.nodes.arch.diffusion.flux2klein — _infer_hyperparams()."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest

# torch is guarded, not imported unconditionally: tests in this file only
# exercise _infer_hyperparams() which uses safetensors (already in base.txt)
# and never imports torch at module level. The worker-*-mock CI job installs
# requirements/base.txt only (no torch) and only *collects* this file —
# it never runs real_mode-marked tests — so an unconditional `import torch`
# here would break collection for the whole file, including the mock-mode
# tests, per ANVILML_DESIGN.md §18.3.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]

from worker.nodes.arch.diffusion.flux2klein import _infer_hyperparams

_FIXTURE_DIR = Path(__file__).parent / "fixtures"


def test_infer_hyperparams_regular_fixture() -> None:
    """_infer_hyperparams() returns correct hyperparameters for the regular Flux 2 Klein 4B fixture.

    Calls _infer_hyperparams() against ``flux2klein4b_tiny.safetensors``
    (which has ``arch="flux2klein"`` metadata and recognizable Flux 2 Klein
    key prefixes) and asserts the returned dict has the expected keys and
    correct values.

    This is the primary test — it exercises the regular code path where
    metadata contains the ``"arch"`` key and all Flux 2 Klein key prefixes
    are present.
    """
    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
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
        "native_dtype",
    ]
    for key in expected_keys:
        assert key in result, f"missing key '{key}' in hyperparameter dict"

    # Assert correct values for the tiny fixture.
    assert result["hidden_dim"] == 128
    assert result["double_block_count"] == 1
    assert result["single_block_count"] == 1
    assert result["latent_channels"] == 4
    assert result["latent_height"] == 8
    assert result["latent_width"] == 8
    assert result["patch_size"] == 8
    assert result["arch"] == "flux2klein"
    assert result["native_dtype"] == "fp32"


def test_infer_hyperparams_no_metadata_fixture() -> None:
    """_infer_hyperparams() infers arch from key patterns when metadata is absent.

    Calls _infer_hyperparams() against
    ``flux2klein4b_tiny_no_metadata.safetensors`` (which has no ``"arch"``
    key in its safetensors header and uses ``xyz_`` prefixed keys) and
    asserts the metadata-fallback path succeeds.

    The fallback path must:
    1. Detect Flux 2 Klein architecture from key naming patterns
       (``double_block``, ``single_block``, ``final_layer``, ``img_mod``,
       ``txt_mod``).
    2. Return the same shape-based hyperparameters as the regular fixture.
    """
    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny_no_metadata.safetensors"
    result = _infer_hyperparams(str(fixture_path))

    # Shape-based hyperparameters should match the regular fixture.
    assert result["hidden_dim"] == 128
    assert result["double_block_count"] == 1
    assert result["single_block_count"] == 1
    assert result["latent_channels"] == 4
    assert result["latent_height"] == 8
    assert result["latent_width"] == 8
    assert result["patch_size"] == 8

    # The fallback path must identify the architecture from key patterns.
    # The no-metadata fixture uses xyz_ prefixed keys that still contain
    # the substrings "double_block", "single_block", "final_layer", etc.
    assert result["arch"] == "flux2klein"

    # Native dtype should default to fp32 since no keys end in ".weight"
    # in the no-metadata fixture (xyz_ prefix removes the dot separator).
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
    few bytes that do not form a valid safetensors header) and asserts
    that _infer_hyperparams() raises ValueError.
    """
    # Write a small binary blob that is not a valid safetensors file.
    # A valid safetensors file starts with an 8-byte little-endian u64
    # header length followed by a valid JSON header.
    corrupt_data = b"\x00\x01\x02\x03\x04\x05\x06\x07"

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
            os.unlink(tmp_path)
        except OSError:
            pass
