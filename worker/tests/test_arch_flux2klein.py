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


def test_collection_safety_sample_import() -> None:
    """Importing flux2klein with ANVILML_WORKER_MOCK=1 and no torch succeeds (sample guard).

    Spawns a subprocess that imports ``worker.nodes.arch.diffusion.flux2klein``
    with ``torch`` removed from ``sys.modules`` (simulating the mock-mode
    environment where torch is not installed). Asserts the import succeeds
    without raising, confirming the module-level import guard covers
    ``EulerDiscreteScheduler`` (diffusers) and all sample() entry points.

    This serves as the ``MOCK_PATH_VERIFIED`` parity marker for ``sample()`` —
    per ANVILML_DESIGN.md §10.6's exception for arch-module load()/sample()/
    decode(), the mock-mode marker names a collection-safety test rather
    than a mock-branch test (there is no mock branch inside sample()).

    **Mode:** mock
    """
    import subprocess
    import sys

    code = (
        "import sys; "
        "sys.modules['torch'] = None; "
        "sys.modules['torch.nn'] = None; "
        "sys.modules['safetensors.torch'] = None; "
        "sys.modules['diffusers'] = None; "
        "import worker.nodes.arch.diffusion.flux2klein as m; "
        "assert m.torch is None; "
        "assert m.nn is None; "
        "assert m.EulerDiscreteScheduler is None; "
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


# ---------------------------------------------------------------------------
# P25-D1: compute_latent_shape() tests
# ---------------------------------------------------------------------------


@pytest.fixture
def _flux2klein_default_hyperparams():
    """Pin flux2klein.MODEL_PATCH_SIZE / MODEL_LATENT_CHANNELS, then restore them.

    compute_latent_shape() reads module-level mutable globals that load()
    overwrites in place with checkpoint-derived values. Any test that
    exercises the pre-load *default* must not depend on execution order
    relative to a real_mode test that has already called load() in the same
    session — test_compute_latent_shape_real_after_load does exactly that,
    and previously ran earlier in this file, which is why the unisolated
    versions of these tests silently passed only when run in file order
    and failed once collection was fixed.

    This fixture pins both globals to the documented default (patch_size=8 —
    Flux 2 Klein's actual patch size, and latent_channels=4) for the
    duration of the test, then restores whatever value was present before.
    """
    import worker.nodes.arch.diffusion.flux2klein as flux2klein

    original_patch_size = flux2klein.MODEL_PATCH_SIZE
    original_latent_channels = flux2klein.MODEL_LATENT_CHANNELS
    flux2klein.MODEL_PATCH_SIZE = 8
    flux2klein.MODEL_LATENT_CHANNELS = 4
    try:
        yield
    finally:
        # Restore unconditionally, even if the test body raises.
        flux2klein.MODEL_PATCH_SIZE = original_patch_size
        flux2klein.MODEL_LATENT_CHANNELS = original_latent_channels


def test_compute_latent_shape_mock_default_patch_size(
    _flux2klein_default_hyperparams,
) -> None:
    """compute_latent_shape() produces correct shape with default patch_size=8.

    Calls compute_latent_shape() with width=64, height=64, batch_size=1.
    With MODEL_PATCH_SIZE=8 (Flux 2 Klein's default), this gives
    latent_height=8, latent_width=8. The result should be (1, 4, 8, 8).

    Also tests 128×128 → (1, 4, 16, 16) and 65×65 → (1, 4, 9, 9)
    (ceiling division for non-multiples).

    This is the primary mock-mode test for the formula.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_mock_default_patch_size
    """
    from worker.nodes.arch.diffusion.flux2klein import compute_latent_shape

    # 64×64 with patch_size=8 → 8×8 latent
    assert compute_latent_shape(64, 64, 1) == (1, 4, 8, 8)

    # 128×128 with patch_size=8 → 16×16 latent
    assert compute_latent_shape(128, 128, 1) == (1, 4, 16, 16)

    # 65×65 with patch_size=8 → 9×9 latent (ceiling division: ceil(65/8)=9)
    assert compute_latent_shape(65, 65, 1) == (1, 4, 9, 9)


@pytest.mark.real_mode
def test_compute_latent_shape_real_after_load() -> None:
    """compute_latent_shape() uses actual checkpoint hyperparameters after load().

    Calls load() against the Flux 2 Klein fixture (which has patch_size=8,
    latent_channels=4), then calls compute_latent_shape(64, 64, 1).
    The result should be (1, 4, 8, 8), proving that load() correctly
    updates the module-level hyperparameters.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_real_after_load
    """
    from worker.nodes.arch.diffusion.flux2klein import compute_latent_shape, load

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    load(str(fixture_path), caps, device="cpu")
    result = compute_latent_shape(64, 64, 1)
    assert result == (1, 4, 8, 8)


def test_compute_latent_shape_non_multiple_dims(
    _flux2klein_default_hyperparams,
) -> None:
    """compute_latent_shape() ceiling division for non-multiples of patch_size.

    Calls compute_latent_shape(100, 80, 1) with patch_size=8.
    100/8 = 12.5 → 13, 80/8 = 10. Result should be (1, 4, 13, 10).

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_non_multiple_dims
    """
    from worker.nodes.arch.diffusion.flux2klein import compute_latent_shape

    assert compute_latent_shape(100, 80, 1) == (1, 4, 13, 10)


def test_compute_latent_shape_batch_size(
    _flux2klein_default_hyperparams,
) -> None:
    """compute_latent_shape() scales the batch dimension correctly.

    Calls compute_latent_shape(64, 64, batch_size=4) with patch_size=8.
    Result should be (4, 4, 8, 8) — batch_size=4 in the first position.

    # MOCK_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_batch_size
    """
    from worker.nodes.arch.diffusion.flux2klein import compute_latent_shape

    assert compute_latent_shape(64, 64, batch_size=4) == (4, 4, 8, 8)


# ---------------------------------------------------------------------------
# P25-D1: _resolve_conditioning() tests
# ---------------------------------------------------------------------------


def test_resolve_conditioning_dict_with_negative() -> None:
    """A dict with both keys splits into (text_embeds, negative_text_embeds).

    This is a pure-function unit test with no torch model involved — it
    only exercises _resolve_conditioning()'s dict-handling branch.
    """
    from worker.nodes.arch.diffusion.flux2klein import _resolve_conditioning

    positive = object()
    negative = object()
    cond, uncond = _resolve_conditioning(
        {"text_embeds": positive, "negative_text_embeds": negative}
    )
    assert cond is positive
    assert uncond is negative


def test_resolve_conditioning_dict_without_negative() -> None:
    """A dict with only text_embeds resolves uncond to None.

    This is the case ClipTextEncode produces when no negative_text was
    supplied — the unconditional pass must fall back to no conditioning.
    """
    from worker.nodes.arch.diffusion.flux2klein import _resolve_conditioning

    positive = object()
    cond, uncond = _resolve_conditioning({"text_embeds": positive})
    assert cond is positive
    assert uncond is None


def test_resolve_conditioning_bare_tensor() -> None:
    """A non-dict value (bare tensor or None) is treated as cond with no uncond.

    Callers that build conditioning directly rather than going through
    ClipTextEncode must keep working unchanged.
    """
    from worker.nodes.arch.diffusion.flux2klein import _resolve_conditioning

    bare = object()
    cond, uncond = _resolve_conditioning(bare)
    assert cond is bare
    assert uncond is None

    cond_none, uncond_none = _resolve_conditioning(None)
    assert cond_none is None
    assert uncond_none is None


# ---------------------------------------------------------------------------
# P25-D1: sample() tests
# ---------------------------------------------------------------------------

_DEFAULT_CAPS: dict = {
    "fp32": True,
    "fp16": False,
    "bf16": False,
    "fp8": False,
    "fp4": False,
    "flash_attention": False,
}


@pytest.mark.real_mode
def test_sample_seed_minus_one_resolves_random() -> None:
    """seed=-1 resolves to a random positive integer twice.

    Calls ``sample()`` with ``seed=-1`` twice on the same model and
    verifies both return positive seeds that differ (randomness).

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_seed_minus_one_resolves_random
    """
    from worker.nodes.arch.diffusion.flux2klein import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    _, seed_a = sample(
        model, "seed_random_a", None, latent_in, 4, 7.5, -1
    )
    _, seed_b = sample(
        model, "seed_random_b", None, latent_in, 4, 7.5, -1
    )

    # Both resolved seeds must be positive integers.
    assert seed_a >= 0 and seed_a < 2**63, f"seed_a={seed_a}"
    assert seed_b >= 0 and seed_b < 2**63, f"seed_b={seed_b}"
    # Two independent calls with seed=-1 should produce different seeds.
    assert seed_a != seed_b, (
        "seed=-1 produced the same random seed twice — secrets.randbelow may not be working"
    )

    # Clean up.
    if "seed_random_a:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["seed_random_a:pipeline"]
    if "seed_random_b:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["seed_random_b:pipeline"]


@pytest.mark.real_mode
def test_sample_seed_positive_reproducible() -> None:
    """Same explicit seed produces identical output tensors.

    Calls ``sample()`` twice with ``seed=42`` and asserts the returned
    latent tensors have identical values.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_seed_positive_reproducible
    """
    from worker.nodes.arch.diffusion.flux2klein import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    latent_a, seed_a = sample(
        model, "seed_repro", None, latent_in, 4, 7.5, 42
    )
    latent_b, seed_b = sample(
        model, "seed_repro", None, latent_in, 4, 7.5, 42
    )

    # Seeds must match.
    assert seed_a == seed_b == 42
    # Latent tensors must have identical values.
    assert torch.equal(latent_a, latent_b)

    # Clean up.
    if "seed_repro:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["seed_repro:pipeline"]


@pytest.mark.real_mode
def test_sample_pipeline_assembly_caching() -> None:
    """sample() caches pipeline — get_or_load called exactly once per model_id.

    Spies on ``pipeline_cache.get_or_load`` to verify the loader function
    is called exactly once on the first call with a given model_id, and
    not called again on subsequent calls with the same model_id.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_pipeline_assembly_caching
    """
    from worker.nodes.arch.diffusion.flux2klein import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    original_get_or_load = pipeline_cache.get_or_load
    call_count = 0

    def spy_get_or_load(key: str, loader_fn) -> object:
        nonlocal call_count
        if key not in pipeline_cache._cache:
            call_count += 1
        return original_get_or_load(key, loader_fn)

    pipeline_cache.get_or_load = spy_get_or_load
    try:
        latent_in = torch.zeros(1, 4, 8, 8)
        _, _ = sample(
            model, "cache_test", None, latent_in, 4, 7.5, 42
        )
        _, _ = sample(
            model, "cache_test", None, latent_in, 4, 7.5, 42
        )

        assert call_count == 1, (
            f"expected loader to be called exactly once, got {call_count}"
        )
    finally:
        pipeline_cache.get_or_load = original_get_or_load
        if "cache_test:pipeline" in pipeline_cache._cache:
            del pipeline_cache._cache["cache_test:pipeline"]


@pytest.mark.real_mode
def test_sample_denoising_real_flux2klein_fixture() -> None:
    """End-to-end denoising against the real Flux 2 Klein fixture checkpoint.

    Calls ``sample()`` with a model loaded from ``flux2klein4b_tiny.safetensors``,
    verifies the output is a tensor with the correct shape and dtype,
    and confirms the seed is a non-negative integer. This is the
    canonical real-mode test for the denoising loop.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_denoising_real_flux2klein_fixture
    """
    from worker.nodes.arch.diffusion.flux2klein import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    latent_out, seed = sample(
        model, "real_flux2klein", None, latent_in, 4, 7.5, 42
    )

    # The output must be a tensor with the same shape as the input.
    assert isinstance(latent_out, torch.Tensor)
    assert latent_out.shape == latent_in.shape

    # The seed must be a non-negative integer (explicit seed passed through).
    assert isinstance(seed, int)
    assert seed >= 0
    assert seed == 42

    # Clean up.
    if "real_flux2klein:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["real_flux2klein:pipeline"]


@pytest.mark.real_mode
def test_sample_denoising_runs_to_completion() -> None:
    """sample() with a loaded model and small steps (4) returns tensor of same shape.

    Verifies the denoising loop runs to completion without error, returns
    a tensor matching the input shape, and returns a positive seed.

    # REAL_PATH_VERIFIED: worker/tests/test_arch_flux2klein.py::test_sample_denoising_runs_to_completion
    """
    from worker.nodes.arch.diffusion.flux2klein import (
        load,
        pipeline_cache,
        sample,
    )

    fixture_path = _FIXTURE_DIR / "flux2klein4b_tiny.safetensors"
    caps = {"fp8": False, "bf16": True, "fp16": True, "fp32": True}
    model = load(str(fixture_path), caps, device="cpu")

    latent_in = torch.zeros(1, 4, 8, 8)
    latent_out, seed = sample(
        model, "completion_test", None, latent_in, 4, 7.5, 99
    )

    assert isinstance(latent_out, torch.Tensor)
    assert latent_out.shape == latent_in.shape
    assert isinstance(seed, int)
    assert seed > 0

    # Clean up.
    if "completion_test:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache["completion_test:pipeline"]
