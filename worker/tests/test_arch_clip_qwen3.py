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
