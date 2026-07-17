"""Tests for worker.nodes.decode — VaeDecode node class and registration."""

import subprocess
import sys
import threading
from pathlib import Path

import pytest

from worker.nodes.base import NodeContext


def _make_ctx(mock: bool = True, pipeline_cache: object | None = None) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.
        pipeline_cache: Optional pipeline cache to use. Defaults to
            an empty dict for backward compatibility with existing tests.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    return NodeContext(
        job_id="test-job",
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache=pipeline_cache if pipeline_cache is not None else {},
        mock=mock,
    )


def test_vae_decode_class_attributes() -> None:
    """VaeDecode defines all six required class attributes with correct values.

    Verifies NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS
    (2 slots: vae, latent), and OUTPUT_SLOTS (1 slot: image) match the
    values specified in ANVILML_DESIGN.md §10.3.

    This test exercises the class definition and satisfies the class-attribute
    portion of the acceptance criteria.

    Expected outcome: All assertions pass with the correct values.
    """
    from worker.nodes.decode import VaeDecode
    from worker.nodes.base import SlotSpec

    assert VaeDecode.NODE_TYPE == "VaeDecode"
    assert VaeDecode.CATEGORY == "Decoding"
    assert VaeDecode.DISPLAY_NAME == "VAE Decode"
    assert (
        VaeDecode.DESCRIPTION
        == "Decodes a denoised latent to a PIL image using the explicitly provided VAE."
    )

    # Verify INPUT_SLOTS: 2 slots with correct names and types.
    assert len(VaeDecode.INPUT_SLOTS) == 2
    assert VaeDecode.INPUT_SLOTS[0] == SlotSpec("vae", "VAE")
    assert VaeDecode.INPUT_SLOTS[1] == SlotSpec("latent", "LATENT")

    # Verify OUTPUT_SLOTS: 1 slot with correct name and type.
    assert len(VaeDecode.OUTPUT_SLOTS) == 1
    assert VaeDecode.OUTPUT_SLOTS[0] == SlotSpec("image", "IMAGE")


def test_vae_decode_mock_returns_sentinel() -> None:
    """Mock-mode execute() returns the sentinel dict with propagated shape.

    Constructs a NodeContext with mock=True, calls execute() with
    vae={} and latent={"shape": (1, 4, 64, 64)}, and asserts the return
    dict matches the expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"image": {"mock": True, "shape": (1, 4, 64, 64)}}
    is returned.
    """
    from worker.nodes.decode import VaeDecode

    node = VaeDecode()
    ctx = _make_ctx(mock=True)
    result = node.execute(
        ctx,
        vae={},
        latent={"shape": (1, 4, 64, 64)},
    )
    assert result == {"image": {"mock": True, "shape": (1, 4, 64, 64)}}


def test_vae_decode_in_registry() -> None:
    """VaeDecode appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.decode in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["VaeDecode"]
    exists and is the VaeDecode class. This proves auto-import and
    registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_nodes_encoder.py::
    test_clip_text_encode_in_registry.

    Expected outcome: NODE_REGISTRY contains "VaeDecode" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.decode'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'VaeDecode' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['VaeDecode'] is mod.VaeDecode; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


# ---------------------------------------------------------------------------
# Real-mode tests — VaeDecode dispatches to arch.vae module (P24-B2)
# ---------------------------------------------------------------------------

_FIXTURE_DIR = Path(__file__).parent / "fixtures"
_VAE_FIXTURE = str(_FIXTURE_DIR / "zit_vae_tiny.safetensors")


def test_vae_decode_real_decodes_zit_vae_fixture() -> None:
    """End-to-end real-mode test: load ZiT VAE fixture, create latent, execute VaeDecode.

    Loads the ZiT VAE checkpoint from the fixture file, creates a (1, 4, 8, 8)
    latent tensor, constructs a non-mock NodeContext, and calls VaeDecode.execute().
    Asserts that:
      - Result is a dict with key "image".
      - "image" is a list of exactly 1 PIL.Image.Image.
      - Image mode is "RGB".
      - Image size matches latent spatial dimensions (8, 8).

    This test exercises the full real code path: arch.vae.get_module() →
    zit_vae.decode() → PIL Image creation, and satisfies the
    REAL_PATH_VERIFIED marker on VaeDecode.execute().

    Expected outcome: A valid PIL.Image.Image with mode "RGB" and size (8, 8).
    """
    import torch
    from PIL import Image

    from worker.nodes.decode import VaeDecode
    from worker.nodes.arch.vae.zit_vae import load

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    # Load the VAE from the fixture.
    vae = load(_VAE_FIXTURE, caps={"bf16": True, "fp8": False}, device="cpu")

    # Create a (1, 4, 8, 8) latent tensor — matches the fixture's latent_channels=4
    # and the decoder's spatial output of 8x8 (the forward pass preserves spatial size).
    latent = torch.randn(1, 4, 8, 8)

    result = node.execute(ctx, vae=vae, latent=latent)

    assert "image" in result, "Result dict must contain 'image' key"
    images = result["image"]
    assert isinstance(images, list), f"Expected list, got {type(images).__name__}"
    assert len(images) == 1, f"Expected 1 image for batch=1, got {len(images)}"

    img = images[0]
    assert isinstance(img, Image.Image), f"Expected PIL.Image.Image, got {type(img).__name__}"
    assert img.mode == "RGB", f"Expected RGB mode, got {img.mode}"
    assert img.size == (8, 8), f"Expected size (8, 8), got {img.size}"


def test_vae_decode_real_batched_latent() -> None:
    """Batched latent (batch=2) produces exactly 2 PIL Images.

    Loads the ZiT VAE fixture, creates a (2, 4, 8, 8) latent tensor,
    executes VaeDecode, and asserts the output list has 2 images.

    Expected outcome: len(result["image"]) == 2.
    """
    import torch

    from worker.nodes.decode import VaeDecode
    from worker.nodes.arch.vae.zit_vae import load

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    vae = load(_VAE_FIXTURE, caps={"bf16": True, "fp8": False}, device="cpu")
    latent = torch.randn(2, 4, 8, 8)

    result = node.execute(ctx, vae=vae, latent=latent)

    images = result["image"]
    assert isinstance(images, list)
    assert len(images) == 2, f"Expected 2 images for batch=2, got {len(images)}"
    for img in images:
        from PIL import Image
        assert isinstance(img, Image.Image)


def test_vae_decode_real_output_rgb_uint8() -> None:
    """Output PIL Image has valid uint8 pixel values in [0, 255] range.

    Loads the ZiT VAE fixture, decodes a latent, and asserts that the
    resulting PIL Image's pixel data consists of valid uint8 values
    (0–255) in all channels.

    Expected outcome: All pixel values are in [0, 255].
    """
    import torch
    import numpy as np
    from PIL import Image

    from worker.nodes.decode import VaeDecode
    from worker.nodes.arch.vae.zit_vae import load

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    vae = load(_VAE_FIXTURE, caps={"bf16": True, "fp8": False}, device="cpu")
    latent = torch.randn(1, 4, 8, 8)

    result = node.execute(ctx, vae=vae, latent=latent)
    img = result["image"][0]

    assert isinstance(img, Image.Image)
    # Convert to numpy and verify pixel values are valid uint8.
    arr = np.array(img)
    assert arr.min() >= 0 and arr.max() <= 255, (
        f"Pixel values out of [0, 255] range: min={arr.min()}, max={arr.max()}"
    )


def test_vae_decode_real_arch_dispatch_uses_vae_arch() -> None:
    """arch.vae.get_module() is called with the correct arch key from the loaded VAE.

    Loads the ZiT VAE fixture (which sets .arch = "zit_vae"), then patches
    arch.vae.get_module to capture the key argument and verify it equals "zit_vae".

    Expected outcome: get_module("zit_vae") is called.
    """
    import torch
    from unittest.mock import patch

    from worker.nodes.decode import VaeDecode
    from worker.nodes.arch.vae.zit_vae import load

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    vae = load(_VAE_FIXTURE, caps={"bf16": True, "fp8": False}, device="cpu")
    latent = torch.randn(1, 4, 8, 8)

    captured_keys: list[str] = []
    # Import the real get_module before patching — importing inside the
    # side_effect would resolve to the patched version, causing recursion.
    from worker.nodes.arch.vae import get_module as _real_get_module

    def _capture_get_module(key):
        captured_keys.append(key)
        return _real_get_module(key)

    with patch("worker.nodes.arch.vae.get_module", side_effect=_capture_get_module):
        result = node.execute(ctx, vae=vae, latent=latent)

    assert "image" in result
    assert len(captured_keys) == 1, f"Expected 1 call to get_module, got {len(captured_keys)}"
    assert captured_keys[0] == "zit_vae", (
        f"Expected get_module('zit_vae'), got get_module({captured_keys[0]!r})"
    )


def test_vae_decode_real_missing_arch_raises() -> None:
    """Passing a dict-like vae input without .arch attribute raises ValueError.

    Constructs a non-mock context and calls execute with a plain dict (no .arch)
    and a valid latent tensor. Asserts that ValueError is raised with a descriptive
    message containing "arch".

    Expected outcome: ValueError raised.
    """
    import torch

    from worker.nodes.decode import VaeDecode

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    latent = torch.randn(1, 4, 8, 8)

    with pytest.raises(ValueError, match="arch"):
        node.execute(ctx, vae={}, latent=latent)


def test_vae_decode_real_unregistered_arch_raises() -> None:
    """get_module() returning None raises RuntimeError with descriptive message.

    Loads the ZiT VAE fixture, patches arch.vae.get_module to return None
    (simulating an unregistered arch key), and asserts that RuntimeError
    is raised with a message containing the arch key.

    Expected outcome: RuntimeError raised with arch=... in message.
    """
    import torch
    from unittest.mock import patch

    from worker.nodes.decode import VaeDecode
    from worker.nodes.arch.vae.zit_vae import load

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    vae = load(_VAE_FIXTURE, caps={"bf16": True, "fp8": False}, device="cpu")
    latent = torch.randn(1, 4, 8, 8)

    with patch("worker.nodes.arch.vae.get_module", return_value=None):
        with pytest.raises(RuntimeError, match="no registered VAE module"):
            node.execute(ctx, vae=vae, latent=latent)


def test_vae_decode_real_missing_vae_input_raises() -> None:
    """Calling execute without the 'vae' input raises ValueError.

    Constructs a non-mock context and calls execute with only a latent
    input (no vae). Asserts that ValueError is raised.

    Expected outcome: ValueError raised.
    """
    import torch

    from worker.nodes.decode import VaeDecode

    node = VaeDecode()
    ctx = _make_ctx(mock=False)

    latent = torch.randn(1, 4, 8, 8)

    with pytest.raises(ValueError, match="'vae' input is required"):
        node.execute(ctx, latent=latent)
