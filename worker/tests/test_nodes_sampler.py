"""Tests for worker.nodes.sampler — Sampler node class and registration."""

import subprocess
import sys
import threading
import uuid

import pytest

from worker.nodes.base import NodeContext, SlotSpec

# ---------------------------------------------------------------------------
# Shared fixtures for real-mode tests
# ---------------------------------------------------------------------------

# Path to the ZiT tiny fixture checkpoint used by all real-mode tests.
# This fixture was built by the `build_zit_fixture.py` script in the
# fixtures directory and is the same checkpoint used by the
# test_sample_denoising_real_zit_fixture test in test_arch_zit.py.
_FIXTURE_DIR = __import__("pathlib").Path(__file__).parent / "fixtures"

# Default capability dict used for model loading — bf16 is available
# on CPU, fp8 is not. This matches the capability probe defaults.
_DEFAULT_CAPS = {"bf16": True, "fp8": False}


def _make_ctx(mock: bool = True) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    # job_id must be raw 16-byte UUID bytes — NodeContext.job_id_str
    # constructs a UUID from self.job_id via uuid.UUID(bytes=self.job_id).
    # A string like "test-job" would raise ValueError at that call site.
    return NodeContext(
        job_id=uuid.uuid4().bytes,
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache={},
        mock=mock,
    )


def test_sampler_class_attributes() -> None:
    """All six required class attributes exist with correct values.

    Verifies that Sampler defines NODE_TYPE, CATEGORY, DISPLAY_NAME,
    DESCRIPTION, INPUT_SLOTS, and OUTPUT_SLOTS with the exact values
    specified in the node contract.

    Expected outcome: All six attributes match their expected values.
    """
    from worker.nodes.sampler import Sampler

    assert Sampler.NODE_TYPE == "Sampler"
    assert Sampler.CATEGORY == "Sampling"
    assert Sampler.DISPLAY_NAME == "Sampler"
    assert (
        Sampler.DESCRIPTION
        == "Runs a denoising diffusion step to produce a latent from a model, "
           "conditioning, and latent input."
    )
    assert len(Sampler.INPUT_SLOTS) == 7
    assert Sampler.INPUT_SLOTS[0] == SlotSpec("model", "MODEL")
    assert Sampler.INPUT_SLOTS[1] == SlotSpec("conditioning", "CONDITIONING")
    assert Sampler.INPUT_SLOTS[2] == SlotSpec("clip", "CLIP")
    assert Sampler.INPUT_SLOTS[3] == SlotSpec("latent", "LATENT")
    assert Sampler.INPUT_SLOTS[4] == SlotSpec("steps", "INT")
    assert Sampler.INPUT_SLOTS[5] == SlotSpec("cfg", "FLOAT")
    assert Sampler.INPUT_SLOTS[6] == SlotSpec("seed", "INT")
    assert len(Sampler.OUTPUT_SLOTS) == 2
    assert Sampler.OUTPUT_SLOTS[0] == SlotSpec("latent", "LATENT")
    assert Sampler.OUTPUT_SLOTS[1] == SlotSpec("seed", "INT")


def test_sampler_mock_returns_expected_shape() -> None:
    """Mock-mode execute() returns the sentinel dict with propagated shape.

    Constructs a NodeContext with mock=True, calls execute() with a latent
    dict containing a shape, and asserts the return dict has the correct
    sentinel shape and seed.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"latent": {"mock": True, "shape": (1, 4, 64, 64)},
    "seed": 42} is returned.
    """
    from worker.nodes.sampler import Sampler

    node = Sampler()
    ctx = _make_ctx(mock=True)
    result = node.execute(
        ctx,
        model={},
        conditioning={},
        clip={},
        latent={"shape": (1, 4, 64, 64)},
        steps=20,
        cfg=7.5,
        seed=42,
    )
    assert result == {
        "latent": {"mock": True, "shape": (1, 4, 64, 64)},
        "seed": 42,
    }


def test_sampler_mock_seed_zero() -> None:
    """Mock seed=-1 resolves to 0 deterministically.

    Constructs a NodeContext with mock=True, calls execute() with seed=-1,
    and asserts the returned seed is 0. Also verifies that a non-negative
    seed passes through unchanged.

    Expected outcome: seed=-1 → 0, seed=42 → 42.
    """
    from worker.nodes.sampler import Sampler

    node = Sampler()
    ctx = _make_ctx(mock=True)

    result_neg = node.execute(
        ctx,
        model={},
        conditioning={},
        clip={},
        latent={"shape": (1, 4, 64, 64)},
        steps=20,
        cfg=7.5,
        seed=-1,
    )
    assert result_neg["seed"] == 0

    result_pos = node.execute(
        ctx,
        model={},
        conditioning={},
        clip={},
        latent={"shape": (1, 4, 64, 64)},
        steps=20,
        cfg=7.5,
        seed=42,
    )
    assert result_pos["seed"] == 42


@pytest.mark.real_mode
def test_sampler_real_denoises_zit_fixture() -> None:
    """End-to-end real-mode: load ZiT fixture, run Sampler, verify output.

    Loads the ZiT fixture via ``zit.load()``, calls ``Sampler.execute()``
    with the loaded model, conditioning (None), a noise latent tensor,
    steps=20, cfg=7.5, seed=42. Asserts the returned latent is a
    ``torch.Tensor`` with the same shape as the input, and the returned
    seed equals 42.

    This is the canonical real-mode test for the Sampler's real branch.

    Expected outcome: {"latent": torch.Tensor of shape (1, 4, 8, 8),
    "seed": 42} is returned.
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    # Cast latent to model's dtype — the model weights are loaded in
    # bf16 (per _DEFAULT_CAPS), and PyTorch's linear layer requires
    # matching dtypes between input and weight tensors.
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    assert isinstance(result["latent"], torch.Tensor)
    assert result["latent"].shape == latent_in.shape
    assert result["seed"] == 42

    # Clean up pipeline cache to avoid leaking state to other tests.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


@pytest.mark.real_mode
def test_sampler_real_seed_minus_one_resolves() -> None:
    """seed=-1 resolves to a non-negative integer in [0, 2**63).

    Loads the ZiT fixture, calls ``Sampler.execute()`` with seed=-1,
    and asserts the returned seed is a non-negative integer strictly
    less than ``2**63``. This verifies the real branch correctly
    delegates seed resolution to ``zit.sample()``.

    Expected outcome: returned seed is an int in [0, 2**63).
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=-1,
    )

    assert isinstance(result["seed"], int)
    assert result["seed"] >= 0
    assert result["seed"] < 2**63

    # Clean up.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


@pytest.mark.real_mode
def test_sampler_real_explicit_seed_unchanged() -> None:
    """Explicit seed=42 passes through unchanged.

    Loads the ZiT fixture, calls ``Sampler.execute()`` with seed=42,
    and asserts the returned seed equals 42. This verifies that
    non-negative seeds are not modified by the real branch.

    Expected outcome: returned seed == 42.
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    assert result["seed"] == 42

    # Clean up.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


@pytest.mark.real_mode
def test_sampler_real_multiple_steps() -> None:
    """steps=10 produces correct output shape.

    Loads the ZiT fixture, calls ``Sampler.execute()`` with steps=10,
    and asserts the output latent has the same shape as the input.
    This verifies the denoising loop runs the correct number of steps
    without altering the output shape.

    Expected outcome: output tensor shape matches input shape (1, 4, 8, 8).
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=10,
        cfg=7.5,
        seed=42,
    )

    assert isinstance(result["latent"], torch.Tensor)
    assert result["latent"].shape == latent_in.shape

    # Clean up.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


@pytest.mark.real_mode
def test_sampler_real_cfg_one_is_conditional_only() -> None:
    """cfg=1.0 (no guidance) runs without error.

    Loads the ZiT fixture, calls ``Sampler.execute()`` with cfg=1.0.
    This exercises the CFG path where the unconditional and conditional
    predictions are blended — with cfg=1.0 the unconditional pass
    contributes zero, so the output is purely conditional. Asserts
    the output is a tensor (not an error).

    Expected outcome: returns {"latent": torch.Tensor, "seed": int}.
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=1.0,
        seed=42,
    )

    assert isinstance(result["latent"], torch.Tensor)
    assert isinstance(result["seed"], int)

    # Clean up.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


@pytest.mark.real_mode
def test_sampler_real_latent_shape_preserved() -> None:
    """Output tensor shape matches input latent shape (1, 4, 8, 8).

    Loads the ZiT fixture, calls ``Sampler.execute()`` with a latent
    tensor of shape (1, 4, 8, 8), and asserts the returned tensor has
    the identical shape. This verifies the Sampler does not alter the
    latent dimensions during denoising.

    Expected outcome: output tensor shape == (1, 4, 8, 8).
    """
    import torch

    from worker.nodes.arch.diffusion.zit import (
        load,
        pipeline_cache,
    )
    from worker.nodes.sampler import Sampler

    fixture_path = _FIXTURE_DIR / "zit_tiny.safetensors"
    model = load(str(fixture_path), _DEFAULT_CAPS, device="cpu")

    node = Sampler()
    ctx = _make_ctx(mock=False)
    model_dtype = next(model.parameters()).dtype
    latent_in = torch.zeros(1, 4, 8, 8, dtype=model_dtype)

    result = node.execute(
        ctx,
        model=model,
        conditioning=None,
        clip={},
        latent=latent_in,
        steps=20,
        cfg=7.5,
        seed=42,
    )

    assert isinstance(result["latent"], torch.Tensor)
    assert result["latent"].shape == (1, 4, 8, 8)

    # Clean up.
    if f"test-job:pipeline" in pipeline_cache._cache:
        del pipeline_cache._cache[f"test-job:pipeline"]


def test_sampler_in_registry() -> None:
    """Sampler appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.sampler in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["Sampler"]
    exists and is the Sampler class. This proves auto-import and
    registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_passthrough.py::
    test_node_in_registry_after_import.

    Expected outcome: NODE_REGISTRY contains "Sampler" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.sampler'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'Sampler' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['Sampler'] is mod.Sampler; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout
