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


# ---------------------------------------------------------------------------
# P25-B2: can_handle() and dispatch registration tests
# ---------------------------------------------------------------------------


def test_can_handle_matches_flux2klein() -> None:
    """can_handle(\"flux2klein\") returns True — the primary match path.

    Calls can_handle() with the canonical Flux 2 Klein architecture string
    and asserts it returns True, proving the dispatcher will route a
    ``"flux2klein"`` key to this module.
    """
    from worker.nodes.arch.diffusion.flux2klein import can_handle

    assert can_handle("flux2klein") is True


def test_can_handle_rejects_zit_key() -> None:
    """can_handle(\"zit\") returns False — the module rejects unrelated keys.

    Calls can_handle() with zit's architecture string and asserts it returns
    False, proving the dispatcher will skip this module for non-Flux2Klein keys.
    This is the cross-check against zit.py's fixture key.
    """
    from worker.nodes.arch.diffusion.flux2klein import can_handle

    assert can_handle("zit") is False


def test_get_module_returns_flux2klein_for_flux2klein_key() -> None:
    """get_module(\"flux2klein\") returns the flux2klein module — end-to-end dispatch.

    Calls get_module() with ``"flux2klein"`` and asserts the result is not None
    and is the flux2klein module, proving that importing and registering flux2klein
    in __init__.py makes the dispatcher find it as the second registered module.
    """
    from worker.nodes.arch.diffusion import flux2klein, get_module

    result = get_module("flux2klein")
    assert result is not None
    assert result is flux2klein


def test_get_module_returns_zit_for_zit_key() -> None:
    """get_module(\"zit\") returns the zit module — two-module coexistence.

    Calls get_module() with ``"zit"`` and asserts the result is not None
    and is the zit module, proving that both zit and flux2klein coexist in
    _REGISTERED_MODULES and each is correctly disambiguated by its own
    can_handle(). This is the primary test for the two-module disambiguation
    that ANVILML_DESIGN.md §20 requires.
    """
    from worker.nodes.arch.diffusion import get_module, zit

    result = get_module("zit")
    assert result is not None
    assert result is zit


# ---------------------------------------------------------------------------
# P25-C1: Flux2KleinModel, _select_dtype(), and load() tests
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_load_meta_construction_regular_fixture() -> None:
    """load() against the regular 4B fixture returns a Flux2KleinModel on the target device.

    Calls ``load()`` against ``flux2klein4b_tiny.safetensors`` with
    bf16 capability and asserts:
    - The returned model is a ``Flux2KleinModel`` instance.
    - ``model.arch == "flux2klein"``.
    - All parameters are on the ``"cpu"`` device (not ``"meta"``).
    - The selected dtype is ``torch.bfloat16`` (bf16=True, fp8=False, fp16=True).

    This is the primary real-mode test for the load() function and serves
    as the ``REAL_PATH_VERIFIED`` parity marker.

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import Flux2KleinModel, load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    # Verify model type and architecture identifier.
    assert isinstance(model, Flux2KleinModel)
    assert model.arch == "flux2klein"

    # Verify all parameters are materialized on the target device (not meta).
    for param in model.parameters():
        assert param.device.type == "cpu", f"param {param.dtype} on {param.device}"

    # Verify the selected dtype is bf16.
    for param in model.parameters():
        assert param.dtype == torch.bfloat16


@pytest.mark.real_mode
def test_load_meta_construction_no_metadata_fixture() -> None:
    """load() against the no-metadata fixture returns a Flux2KleinModel on the target device.

    Calls ``load()`` against ``flux2klein4b_tiny_no_metadata.safetensors``
    (which has no ``"arch"`` metadata key) and asserts the same invariants
    as the regular fixture test — the metadata-fallback path in
    ``_infer_hyperparams()`` identifies the architecture from key patterns.

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import Flux2KleinModel, load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny_no_metadata.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    assert isinstance(model, Flux2KleinModel)
    assert model.arch == "flux2klein"

    for param in model.parameters():
        assert param.device.type == "cpu"


def test_dtype_selection_fp8_caps() -> None:
    """_select_dtype() returns float8_e4m3fn when caps.fp8=True and native_dtype is fp8.

    Verifies the first branch of the §11.5 precedence chain: fp8 is selected
    only when BOTH the worker supports fp8 AND the checkpoint was saved in
    an FP8 format.

    **Mode:** both
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import _select_dtype

    result = _select_dtype({"fp8": True, "bf16": True, "fp16": True}, "fp8")
    assert result == torch.float8_e4m3fn


def test_dtype_selection_bf16_caps() -> None:
    """_select_dtype() returns bfloat16 when bf16 is available and fp8 is not viable.

    Verifies the second branch: bf16 is selected when caps.bf16=True and
    fp8 is not viable (either caps.fp8=False or native_dtype != "fp8").

    **Mode:** both
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import _select_dtype

    result = _select_dtype(
        {"fp8": False, "bf16": True, "fp16": True},
        "fp32",  # native is not fp8, so fp8 branch is skipped
    )
    assert result == torch.bfloat16


def test_dtype_selection_fp16_caps() -> None:
    """_select_dtype() returns float16 when fp16 is available but bf16 is not.

    Verifies the third branch: fp16 is selected when caps.fp16=True,
    bf16=False, and fp8 is not viable.

    **Mode:** both
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import _select_dtype

    result = _select_dtype(
        {"fp8": False, "bf16": False, "fp16": True},
        "fp32",
    )
    assert result == torch.float16


def test_dtype_selection_fp32_caps() -> None:
    """_select_dtype() returns float32 when no higher precision is available.

    Verifies the fourth branch: fp32 is the universal fallback when
    caps.fp8=False, caps.bf16=False, and caps.fp16=False.

    **Mode:** both
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import _select_dtype

    result = _select_dtype(
        {"fp8": False, "bf16": False, "fp16": False},
        "fp32",
    )
    assert result == torch.float32


def test_collection_safety_load_import() -> None:
    """Importing flux2klein with ANVILML_WORKER_MOCK=1 and no torch succeeds.

    Spawns a subprocess that imports ``worker.nodes.arch.diffusion.flux2klein``
    with ``torch`` removed from ``sys.modules`` (simulating the mock-mode
    environment where torch is not installed). Asserts the import succeeds
    without raising, confirming the module-level import guard works.

    This serves as the ``MOCK_PATH_VERIFIED`` parity marker for ``load()`` —
    per ANVILML_DESIGN.md §10.6's exception for arch-module load()/sample()/
    decode(), the mock-mode marker names a collection-safety test rather
    than a mock-branch test (there is no mock branch inside load()).

    **Mode:** mock
    """
    import subprocess
    import sys

    # Import the module in a subprocess with torch unavailable — this
    # verifies the module-level import guard (try/except ImportError)
    # works correctly and keeps the module importable without torch.
    code = (
        "import sys; "
        "sys.modules['torch'] = None; "
        "sys.modules['torch.nn'] = None; "
        "sys.modules['safetensors.torch'] = None; "
        "import worker.nodes.arch.diffusion.flux2klein as m; "
        "assert m.torch is None; "
        "assert m.nn is None; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert result.returncode == 0, (
        f"flux2klein import failed in mock-mode (no torch). "
        f"stderr={result.stderr}"
    )
    assert "OK" in result.stdout


# ---------------------------------------------------------------------------
# P25-C2: Weight loading, key remapping, dtype, and .arch tests
# ---------------------------------------------------------------------------


@pytest.mark.real_mode
def test_load_key_remapping_regular_fixture() -> None:
    """load() remaps checkpoint keys correctly — at least some params are non-zero.

    Calls ``load()`` against the regular fixture with bf16 capability, then
    inspects the model's parameters to confirm that the key remapping
    + weight loading successfully loaded at least some weights. Specifically
    checks that ``time_text_emb.weight`` is non-zero (the cleanest remap:
    ``time_text_embed.timestep_embedder.0.weight`` → ``time_text_emb.weight``
    has matching shape 128×128).

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    # time_text_emb.weight should be non-zero because the checkpoint
    # key ``time_text_embed.timestep_embedder.0.weight`` (128, 128)
    # remaps to ``time_text_emb.weight`` (128, 128) — exact shape match.
    weight_param = model.time_text_emb.weight
    assert weight_param is not None, "time_text_emb.weight not found"
    assert weight_param.norm().item() > 0, (
        "time_text_emb.weight is all zeros — key remapping failed"
    )

    # img_attn.norm → img_norm1.weight also has a matching shape (128,).
    norm_param = model.double_blocks[0]["img_norm1"].weight
    assert norm_param is not None, "img_norm1.weight not found"
    assert norm_param.norm().item() > 0, (
        "img_norm1.weight is all zeros — norm remapping failed"
    )


@pytest.mark.real_mode
def test_load_arch_attribute_set() -> None:
    """model.arch == "flux2klein" after load() — dedicated .arch contract test.

    Calls ``load()`` against the regular fixture and asserts that
    ``model.arch`` equals ``"flux2klein"``, confirming the `.arch`
    attribute contract set in P25-B2 persists through the full
    load() pipeline including weight loading.

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    assert model.arch == "flux2klein"


@pytest.mark.real_mode
def test_load_tensor_dtype_bf16() -> None:
    """All parameters are torch.bfloat16 after load() with bf16 caps.

    Calls ``load()`` with bf16=True and asserts every parameter is
    ``torch.bfloat16``, confirming the cast-before-assign ordering
    works correctly (tensors are cast to target_dtype BEFORE
    ``load_state_dict(assign=True)``).

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    for param in model.parameters():
        assert param.dtype == torch.bfloat16, (
            f"expected bf16, got {param.dtype} on {param.shape}"
        )


@pytest.mark.real_mode
def test_load_tensor_dtype_fp16() -> None:
    """All parameters are torch.float16 after load() with fp16-only caps.

    Calls ``load()`` with bf16=False, fp16=True and asserts every
    parameter is ``torch.float16``, testing the dtype fallback path
    through the load → cast → assign chain.

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": False, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    for param in model.parameters():
        assert param.dtype == torch.float16, (
            f"expected fp16, got {param.dtype} on {param.shape}"
        )


@pytest.mark.real_mode
def test_load_no_metadata_key_remapping() -> None:
    """xyz_ prefixed keys are correctly remapped — at least some params are non-zero.

    Calls ``load()`` against the no-metadata fixture (which uses
    ``xyz_`` prefixed keys) and inspects parameters to confirm that
    the xyz_ → dot remapping + Flux 2 Klein remapping successfully
    loaded at least some weights. Specifically checks that
    ``time_text_emb.weight`` is non-zero.

    **Mode:** real
    """
    pytest.importorskip("torch")

    from worker.nodes.arch.diffusion.flux2klein import load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny_no_metadata.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    # The no-metadata fixture has ``xyz_time_text_embed_timestep_embedder``
    # (128, 128) which converts to ``time_text_embed.timestep_embedder``
    # then remaps to ``time_text_emb.weight`` (128, 128) — exact shape match.
    weight_param = model.time_text_emb.weight
    assert weight_param is not None, "time_text_emb.weight not found"
    assert weight_param.norm().item() > 0, (
        "time_text_emb.weight is all zeros — xyz_ remapping failed"
    )

    # img_attn.norm → img_norm1.weight also remaps via the xyz_ chain.
    norm_param = model.double_blocks[0]["img_norm1"].weight
    assert norm_param is not None, "img_norm1.weight not found"
    assert norm_param.norm().item() > 0, (
        "img_norm1.weight is all zeros — xyz_ norm remapping failed"
    )
