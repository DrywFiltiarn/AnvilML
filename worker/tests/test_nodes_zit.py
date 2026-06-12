"""Tests for the ZiT node classes and SaveImage node.

All tests run in mock mode (``ANVILML_WORKER_MOCK=1``) so that
``torch`` and ``diffusers`` are never imported.
"""

from __future__ import annotations

import os
import sys
import threading
from unittest import mock

import pytest

# Ensure mock mode so torch/diffusers are never imported.
os.environ["ANVILML_WORKER_MOCK"] = "1"

from worker.nodes.base import (
    NODE_REGISTRY,
    NodeContext,
    register,
)


@pytest.fixture(autouse=True)
def _clear_registry() -> None:
    """Ensure NODE_REGISTRY is empty and re-import node modules.

    Because ``@register`` runs at module-load time, clearing
    ``NODE_REGISTRY`` alone is insufficient — we must also remove the
    cached module from ``sys.modules`` so the decorator re-runs on
    the next import.
    """
    NODE_REGISTRY.clear()
    # Remove cached node modules so @register re-runs on next import.
    for key in list(sys.modules):
        if key == "worker.nodes.zit" or key.startswith("worker.nodes.zit."):
            del sys.modules[key]
        if key == "worker.nodes.common" or key.startswith("worker.nodes.common."):
            del sys.modules[key]


def _make_ctx(**overrides: object) -> NodeContext:
    """Build a minimal ``NodeContext`` with sensible defaults."""
    defaults = dict(
        pipeline_cache=mock.MagicMock(),
        device_str="cpu",
        emit_fn=mock.MagicMock(),
        cancel_flag=threading.Event(),
        job_id="test-job-0",
    )
    defaults.update(overrides)
    return NodeContext(**defaults)


# ── ZitLoadPipeline ────────────────────────────────────────────────────────────


class TestZitLoadPipeline:
    """Tests for the ``ZitLoadPipeline`` node."""

    def test_output_slots_match_declaration(self) -> None:
        """ZitLoadPipeline.OUTPUT_SLOTS == [\"pipeline\"]."""
        from worker.nodes.zit import ZitLoadPipeline  # noqa: F811

        assert ZitLoadPipeline.OUTPUT_SLOTS == ["pipeline"]

    def test_returns_pipeline_key(self) -> None:
        """execute returns a dict with a \"pipeline\" key in mock mode."""
        from worker.nodes.zit import ZitLoadPipeline  # noqa: F811

        ctx = _make_ctx()
        node = ZitLoadPipeline(ctx)
        result = node.execute(model_id="test-model")

        assert "pipeline" in result
        # Mock sentinel is a _MockPipeline instance.
        assert result["pipeline"].name == "zit_pipeline_mock"

    def test_registered_in_registry(self) -> None:
        """ZitLoadPipeline is registered under its NODE_TYPE."""
        from worker.nodes.zit import ZitLoadPipeline  # noqa: F811

        assert "ZitLoadPipeline" in NODE_REGISTRY
        assert NODE_REGISTRY["ZitLoadPipeline"] is ZitLoadPipeline


# ── ZitTextEncode ──────────────────────────────────────────────────────────────


class TestZitTextEncode:
    """Tests for the ``ZitTextEncode`` node."""

    def test_output_slots_match_declaration(self) -> None:
        """ZitTextEncode.OUTPUT_SLOTS == [\"conditioning\"]."""
        from worker.nodes.zit import ZitTextEncode  # noqa: F811

        assert ZitTextEncode.OUTPUT_SLOTS == ["conditioning"]

    def test_returns_conditioning_key(self) -> None:
        """execute returns a dict with a \"conditioning\" key in mock mode."""
        from worker.nodes.zit import ZitTextEncode  # noqa: F811

        ctx = _make_ctx()
        node = ZitTextEncode(ctx)
        result = node.execute(pipeline=mock.MagicMock(), prompt="a cat")

        assert "conditioning" in result
        cond = result["conditioning"]
        # Mock conditioning is a tuple of two _MockTensor.
        assert isinstance(cond, tuple)
        assert len(cond) == 2

    def test_registered_in_registry(self) -> None:
        """ZitTextEncode is registered under its NODE_TYPE."""
        from worker.nodes.zit import ZitTextEncode  # noqa: F811

        assert "ZitTextEncode" in NODE_REGISTRY
        assert NODE_REGISTRY["ZitTextEncode"] is ZitTextEncode


# ── ZitSampler ─────────────────────────────────────────────────────────────────


class TestZitSampler:
    """Tests for the ``ZitSampler`` node."""

    def test_output_slots_match_declaration(self) -> None:
        """ZitSampler.OUTPUT_SLOTS == [\"latents\", \"seed\"]."""
        from worker.nodes.zit import ZitSampler  # noqa: F811

        assert ZitSampler.OUTPUT_SLOTS == ["latents", "seed"]

    def test_returns_latents_and_seed(self) -> None:
        """execute returns dicts with \"latents\" and \"seed\" keys."""
        from worker.nodes.zit import ZitSampler  # noqa: F811

        ctx = _make_ctx()
        node = ZitSampler(ctx)
        result = node.execute(
            pipeline=mock.MagicMock(),
            conditioning=(mock.MagicMock(), mock.MagicMock()),
            steps=8,
            seed=-1,
        )

        assert "latents" in result
        assert "seed" in result
        assert isinstance(result["seed"], int)
        assert result["seed"] >= 0

    def test_seed_resolution(self) -> None:
        """seed=-1 is resolved to a random int in [0, 2^63-1]."""
        from worker.nodes.zit import ZitSampler  # noqa: F811

        ctx = _make_ctx()
        node = ZitSampler(ctx)
        result = node.execute(
            pipeline=mock.MagicMock(),
            conditioning=(mock.MagicMock(), mock.MagicMock()),
            steps=8,
            seed=-1,
        )
        assert 0 <= result["seed"] <= 2**63 - 1

    def test_seed_passthrough(self) -> None:
        """Explicit seed value is passed through unchanged."""
        from worker.nodes.zit import ZitSampler  # noqa: F811

        ctx = _make_ctx()
        node = ZitSampler(ctx)
        result = node.execute(
            pipeline=mock.MagicMock(),
            conditioning=(mock.MagicMock(), mock.MagicMock()),
            steps=8,
            seed=42,
        )
        assert result["seed"] == 42

    def test_registered_in_registry(self) -> None:
        """ZitSampler is registered under its NODE_TYPE."""
        from worker.nodes.zit import ZitSampler  # noqa: F811

        assert "ZitSampler" in NODE_REGISTRY
        assert NODE_REGISTRY["ZitSampler"] is ZitSampler


# ── ZitDecode ──────────────────────────────────────────────────────────────────


class TestZitDecode:
    """Tests for the ``ZitDecode`` node."""

    def test_output_slots_match_declaration(self) -> None:
        """ZitDecode.OUTPUT_SLOTS == [\"image\"]."""
        from worker.nodes.zit import ZitDecode  # noqa: F811

        assert ZitDecode.OUTPUT_SLOTS == ["image"]

    def test_returns_image_key(self) -> None:
        """execute returns a dict with an \"image\" key in mock mode."""
        from worker.nodes.zit import ZitDecode  # noqa: F811

        ctx = _make_ctx()
        node = ZitDecode(ctx)
        result = node.execute(pipeline=mock.MagicMock(), latents=mock.MagicMock())

        assert "image" in result

    def test_registered_in_registry(self) -> None:
        """ZitDecode is registered under its NODE_TYPE."""
        from worker.nodes.zit import ZitDecode  # noqa: F811

        assert "ZitDecode" in NODE_REGISTRY
        assert NODE_REGISTRY["ZitDecode"] is ZitDecode


# ── SaveImage ──────────────────────────────────────────────────────────────────


class TestSaveImage:
    """Tests for the ``SaveImage`` node."""

    def test_output_slots_empty(self) -> None:
        """SaveImage.OUTPUT_SLOTS == []."""
        from worker.nodes.common import SaveImage  # noqa: F811

        assert SaveImage.OUTPUT_SLOTS == []

    def test_returns_empty_dict(self) -> None:
        """execute returns an empty dict (side-effect: ImageReady emission)."""
        from worker.nodes.common import SaveImage  # noqa: F811

        ctx = _make_ctx()
        node = SaveImage(ctx)
        result = node.execute(image="mock")  # string sentinel triggers mock path

        assert result == {}

    def test_emits_imageready_with_correct_fields(self) -> None:
        """SaveImage emits ImageReady with job_id, image_b64, width, height,
        format, seed, steps, prompt."""
        from worker.nodes.common import SaveImage  # noqa: F811

        emit_mock = mock.MagicMock()
        ctx = _make_ctx(emit_fn=emit_mock)
        node = SaveImage(ctx)
        node.execute(
            image="mock",
            prompt="test prompt",
            seed=12345,
            steps=20,
        )

        emit_mock.assert_called_once()
        event = emit_mock.call_args[0][0]

        assert event["_type"] == "ImageReady"
        assert event["job_id"] == "test-job-0"
        assert event["width"] == 64
        assert event["height"] == 64
        assert event["format"] == "png"
        assert isinstance(event["image_b64"], str) and len(event["image_b64"]) > 0
        assert event["seed"] == 12345
        assert event["steps"] == 20
        assert event["prompt"] == "test prompt"

    def test_seed_resolved_when_negative(self) -> None:
        """seed=-1 is replaced with a random int in ImageReady."""
        from worker.nodes.common import SaveImage  # noqa: F811

        emit_mock = mock.MagicMock()
        ctx = _make_ctx(emit_fn=emit_mock)
        node = SaveImage(ctx)
        node.execute(image="mock", seed=-1)

        event = emit_mock.call_args[0][0]
        assert 0 <= event["seed"] <= 2**63 - 1

    def test_registered_in_registry(self) -> None:
        """SaveImage is registered under its NODE_TYPE."""
        from worker.nodes.common import SaveImage  # noqa: F811

        assert "SaveImage" in NODE_REGISTRY
        assert NODE_REGISTRY["SaveImage"] is SaveImage
