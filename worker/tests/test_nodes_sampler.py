"""Tests for worker.nodes.sampler — Sampler node class and registration."""

import subprocess
import sys
import threading
import pytest

from worker.nodes.base import NodeContext, SlotSpec


def _make_ctx(mock: bool = True) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.

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
def test_sampler_real_raises_not_implemented() -> None:
    """Real-mode execute() raises NotImplementedError with P21-C2 message.

    Constructs a NodeContext with mock=False, calls execute() with full
    inputs, and asserts that NotImplementedError is raised with the
    message referencing P21-C2. This is the collectible real-mode test
    for the REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError with message containing
    "deferred to P21-C2" is raised.
    """
    from worker.nodes.sampler import Sampler

    node = Sampler()
    ctx = _make_ctx(mock=False)
    with pytest.raises(
        NotImplementedError, match="deferred to P21-C2"
    ):
        node.execute(
            ctx,
            model={},
            conditioning={},
            clip={},
            latent={"shape": (1, 4, 64, 64)},
            steps=20,
            cfg=7.5,
            seed=42,
        )


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
