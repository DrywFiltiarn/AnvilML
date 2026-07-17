"""Tests for worker.nodes.encoder — ClipTextEncode node class and registration."""

import subprocess
import sys
import threading
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


def test_clip_text_encode_class_attributes() -> None:
    """ClipTextEncode defines all six required class attributes with correct values.

    Verifies NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS
    (3 slots: clip, positive_text, negative_text), and OUTPUT_SLOTS (1 slot:
    conditioning) match the values specified in the plan.

    This test exercises the class definition and satisfies the class-attribute
    portion of the acceptance criteria.

    Expected outcome: All assertions pass with the correct values.
    """
    from worker.nodes.encoder import ClipTextEncode
    from worker.nodes.base import SlotSpec

    assert ClipTextEncode.NODE_TYPE == "ClipTextEncode"
    assert ClipTextEncode.CATEGORY == "Conditioning"
    assert ClipTextEncode.DISPLAY_NAME == "Clip Text Encode"
    assert (
        ClipTextEncode.DESCRIPTION
        == "Encodes a text prompt using a loaded CLIP-compatible encoder."
    )

    # Verify INPUT_SLOTS: 3 slots with correct names and types.
    assert len(ClipTextEncode.INPUT_SLOTS) == 3
    assert ClipTextEncode.INPUT_SLOTS[0] == SlotSpec("clip", "CLIP")
    assert ClipTextEncode.INPUT_SLOTS[1] == SlotSpec("positive_text", "STRING")
    assert ClipTextEncode.INPUT_SLOTS[2] == SlotSpec(
        "negative_text", "STRING", optional=True
    )

    # Verify OUTPUT_SLOTS: 1 slot with correct name and type.
    assert len(ClipTextEncode.OUTPUT_SLOTS) == 1
    assert ClipTextEncode.OUTPUT_SLOTS[0] == SlotSpec("conditioning", "CONDITIONING")


def test_clip_text_encode_mock_returns_sentinel() -> None:
    """Mock-mode execute() returns the sentinel dict with propagated positive_text.

    Constructs a NodeContext with mock=True, calls execute() with
    clip={} and positive_text="a red fox", and asserts the return dict
    matches the expected sentinel shape.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"conditioning": {"mock": True, "positive_text": "a red fox"}}
    is returned.
    """
    from worker.nodes.encoder import ClipTextEncode

    node = ClipTextEncode()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, clip={}, positive_text="a red fox")
    assert result == {
        "conditioning": {"mock": True, "positive_text": "a red fox"}
    }


def test_clip_text_encode_mock_without_negative_text() -> None:
    """Omitting optional negative_text input does not cause an error.

    Constructs a NodeContext with mock=True, calls execute() with only
    clip={} and positive_text="hello" (omitting negative_text entirely),
    and verifies the sentinel is returned correctly.

    This tests the optional input slot handling — the node should not
    require negative_text since it is declared as optional.

    Expected outcome: {"conditioning": {"mock": True, "positive_text": "hello"}}
    is returned without error.
    """
    from worker.nodes.encoder import ClipTextEncode

    node = ClipTextEncode()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, clip={}, positive_text="hello")
    assert result == {
        "conditioning": {"mock": True, "positive_text": "hello"}
    }


def test_clip_text_encode_in_registry() -> None:
    """ClipTextEncode appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.encoder in a subprocess (triggering @register
    at module load), then checks that NODE_REGISTRY["ClipTextEncode"]
    exists. This proves auto-import and registration work end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_nodes_loader.py::
    test_load_model_in_registry.

    Expected outcome: NODE_REGISTRY contains "ClipTextEncode" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.encoder'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'ClipTextEncode' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['ClipTextEncode'] is mod.ClipTextEncode; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


@pytest.mark.real_mode
def test_clip_text_encode_real_positive_only() -> None:
    """Real-mode ClipTextEncode with positive_text only produces valid conditioning.

    Loads the Qwen3 fixture checkpoint via qwen3.load(), replaces the
    full vocab tokenizer with a tiny matching tokenizer (vocab_size=128)
    to avoid index-out-of-range errors, then calls
    ClipTextEncode.execute() with mock=False and only positive_text
    (no negative_text). Verifies the return dict contains a
    "conditioning" key with "text_embeds" tensor of correct shape
    (1, 77, 64) on CPU.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"conditioning": {"text_embeds": Tensor(1, 77, 64)}}.
    """
    from pathlib import Path

    import torch
    from transformers import AutoTokenizer

    from worker.nodes.arch.clip.qwen3 import load as qwen3_load
    from worker.nodes.encoder import ClipTextEncode
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "qwen3_tiny.safetensors"
    )
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    # Load the encoder — this exercises qwen3.load() which attaches
    # the tokenizer to the model.
    clip_encoder = qwen3_load(fixture_path, caps, device="cpu")

    # Replace the full-vocab tokenizer with a tiny one matching the
    # fixture's vocab_size=128. The full Qwen3 tokenizer produces IDs
    # up to ~151,936 which exceeds the fixture's vocab_size=128,
    # causing IndexError in the embedding layer.
    tiny_tokenizer_path = str(
        Path(__file__).parent.parent / "assets" / "qwen3_tiny_tokenizer"
    )
    clip_encoder.tokenizer = AutoTokenizer.from_pretrained(
        tiny_tokenizer_path,
        local_files_only=True,
    )

    node = ClipTextEncode()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(
        ctx,
        clip=clip_encoder,
        positive_text="a red fox",
    )

    # Verify the return dict has the expected conditioning key.
    assert "conditioning" in result
    conditioning = result["conditioning"]

    # Verify text_embeds is present and is a torch tensor.
    assert "text_embeds" in conditioning
    text_embeds = conditioning["text_embeds"]
    assert isinstance(text_embeds, torch.Tensor)

    # Verify the tensor is on CPU (not meta or cuda).
    assert text_embeds.device.type == "cpu"

    # Verify the shape matches the fixture's hidden_dim=64.
    assert text_embeds.shape == (1, 77, 64)


@pytest.mark.real_mode
def test_clip_text_encode_real_with_negative() -> None:
    """Real-mode ClipTextEncode with both positive and negative text produces both embeds.

    Loads the Qwen3 fixture checkpoint, replaces the full vocab tokenizer
    with a tiny matching tokenizer (vocab_size=128), then calls
    execute() with mock=False, positive_text="fox", and negative_text="dog".
    Verifies the conditioning dict contains both "text_embeds" and
    "negative_text_embeds" tensors.

    Expected outcome: conditioning dict has both text_embeds and
    negative_text_embeds keys, each a Tensor(1, 77, 64).
    """
    from pathlib import Path

    import torch
    from transformers import AutoTokenizer

    from worker.nodes.arch.clip.qwen3 import load as qwen3_load
    from worker.nodes.encoder import ClipTextEncode
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "qwen3_tiny.safetensors"
    )
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    clip_encoder = qwen3_load(fixture_path, caps, device="cpu")

    # Replace with tiny tokenizer matching fixture vocab_size=128.
    tiny_tokenizer_path = str(
        Path(__file__).parent.parent / "assets" / "qwen3_tiny_tokenizer"
    )
    clip_encoder.tokenizer = AutoTokenizer.from_pretrained(
        tiny_tokenizer_path,
        local_files_only=True,
    )

    node = ClipTextEncode()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(
        ctx,
        clip=clip_encoder,
        positive_text="fox",
        negative_text="dog",
    )

    conditioning = result["conditioning"]

    # Both embeds must be present.
    assert "text_embeds" in conditioning
    assert "negative_text_embeds" in conditioning

    # Both must be tensors of the expected shape.
    text_embeds = conditioning["text_embeds"]
    negative_embeds = conditioning["negative_text_embeds"]
    assert isinstance(text_embeds, torch.Tensor)
    assert isinstance(negative_embeds, torch.Tensor)
    assert text_embeds.shape == (1, 77, 64)
    assert negative_embeds.shape == (1, 77, 64)


@pytest.mark.real_mode
def test_clip_text_encode_real_negative_omitted() -> None:
    """Omitting negative_text in real mode omits negative_text_embeds from conditioning.

    Loads the Qwen3 fixture checkpoint, replaces the full vocab tokenizer
    with a tiny matching tokenizer (vocab_size=128), then calls
    execute() with mock=False and only positive_text (no negative_text
    argument at all). Verifies the conditioning dict contains
    "text_embeds" but NOT "negative_text_embeds".

    This tests the optional input slot behavior — the node must not
    add a negative_text_embeds key when negative_text was not provided.

    Expected outcome: conditioning has "text_embeds" but no
    "negative_text_embeds" key.
    """
    from pathlib import Path

    import torch
    from transformers import AutoTokenizer

    from worker.nodes.arch.clip.qwen3 import load as qwen3_load
    from worker.nodes.encoder import ClipTextEncode
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "qwen3_tiny.safetensors"
    )
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    clip_encoder = qwen3_load(fixture_path, caps, device="cpu")

    # Replace with tiny tokenizer matching fixture vocab_size=128.
    tiny_tokenizer_path = str(
        Path(__file__).parent.parent / "assets" / "qwen3_tiny_tokenizer"
    )
    clip_encoder.tokenizer = AutoTokenizer.from_pretrained(
        tiny_tokenizer_path,
        local_files_only=True,
    )

    node = ClipTextEncode()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(
        ctx,
        clip=clip_encoder,
        positive_text="test prompt",
    )

    conditioning = result["conditioning"]
    assert "text_embeds" in conditioning
    assert "negative_text_embeds" not in conditioning


@pytest.mark.real_mode
def test_clip_text_encode_real_conditioning_shape() -> None:
    """Real-mode conditioning tensor has the expected shape (1, 77, 64).

    Loads the Qwen3 fixture checkpoint, replaces the full vocab tokenizer
    with a tiny matching tokenizer (vocab_size=128), then calls
    execute() with mock=False and a multi-word positive prompt.
    Verifies the text_embeds tensor has shape (1, 77, 64) where
    1=batch_size, 77=sequence_length (CLIP max tokens), and 64=hidden_dim
    from the fixture's inferred hyperparameters.

    Expected outcome: text_embeds.shape == (1, 77, 64).
    """
    from pathlib import Path

    import torch
    from transformers import AutoTokenizer

    from worker.nodes.arch.clip.qwen3 import load as qwen3_load
    from worker.nodes.encoder import ClipTextEncode
    from worker.pipeline_cache import PipelineCache

    fixture_path = str(
        Path(__file__).parent / "fixtures" / "qwen3_tiny.safetensors"
    )
    caps: dict = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}

    clip_encoder = qwen3_load(fixture_path, caps, device="cpu")

    # Replace with tiny tokenizer matching fixture vocab_size=128.
    tiny_tokenizer_path = str(
        Path(__file__).parent.parent / "assets" / "qwen3_tiny_tokenizer"
    )
    clip_encoder.tokenizer = AutoTokenizer.from_pretrained(
        tiny_tokenizer_path,
        local_files_only=True,
    )

    node = ClipTextEncode()
    ctx = _make_ctx(mock=False, pipeline_cache=PipelineCache())
    result = node.execute(
        ctx,
        clip=clip_encoder,
        positive_text="a quick brown fox jumps over the lazy dog",
    )

    conditioning = result["conditioning"]
    text_embeds = conditioning["text_embeds"]

    # batch=1, seq_len=77 (CLIP max tokens), hidden_dim=64 (fixture).
    assert text_embeds.shape == (1, 77, 64)
