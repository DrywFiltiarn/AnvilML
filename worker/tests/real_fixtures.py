"""Synthetic tiny-config checkpoint fixtures for CLIP text encoders.

These pytest fixtures generate minimal safetensors checkpoints for the three
CLIP text-encoder architectures (qwen3, clip_l, t5) so that downstream real-
mode tests can exercise the full load pipeline without downloading multi-GB
HuggingFace weights.

Each fixture constructs a model with its real ``transformers`` Config class,
overrides only the core dimension parameters to tiny values (hidden_size=32,
num_hidden_layers=2 — or the T5 equivalents), calls ``.state_dict()`` to
obtain the raw tensor dictionary, and saves it via ``safetensors.torch.save_file``.

The ``torch``, ``transformers``, and ``safetensors`` packages are imported
lazily inside each fixture body (not at module level) to preserve mock-mode
import isolation — when ``ANVILML_WORKER_MOCK=1`` is set and these packages
are absent, importing the module does not raise ``ImportError``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import pathlib

import pytest


def tiny_qwen3_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny Qwen3 text-encoder checkpoint and return its path.

    Constructs a ``Qwen3ForCausalLM`` with ``hidden_size=32`` and
    ``num_hidden_layers=2``, saves its ``state_dict()`` to a
    ``.safetensors`` file, and returns the path.

    Args:
        tmp_path: Pytest ``tmp_path`` fixture — the directory in which
            to write the checkpoint file.

    Returns:
        A ``pathlib.Path`` pointing to the saved ``qwen3_clip.safetensors``
        file inside *tmp_path*.

    Raises:
        ImportError: If ``torch`` or ``transformers`` is not installed
            in the current Python environment.
    """
    # Lazy imports — preserve mock-mode import isolation. If torch is
    # absent the fixture body fails with a clear ImportError rather than
    # the entire test module failing to import.
    import torch  # noqa: PLC0414

    from safetensors.torch import save_file  # noqa: PLC0414
    from transformers import (  # noqa: PLC0414
        Qwen3Config,
        Qwen3ForCausalLM,
    )

    # Construct a minimal config — only the dimension parameters are
    # overridden; all other fields use the Config class defaults.
    # This mirrors the real arch modules which use verbatim config dicts
    # sourced from HuggingFace model config.json files.
    config = Qwen3Config(hidden_size=32, num_hidden_layers=2)

    # Instantiate the model and extract its raw state dict.
    # The model is tiny (32-dim, 2 layers) so this runs in milliseconds
    # on CPU without meaningful memory pressure.
    model = Qwen3ForCausalLM(config)
    state_dict = model.state_dict()

    # Save to a .safetensors file in the test's temporary directory.
    output_path = tmp_path / "qwen3_clip.safetensors"
    save_file(state_dict, str(output_path))

    return output_path


def tiny_clip_l_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny CLIP-L text-encoder checkpoint and return its path.

    Constructs a ``CLIPTextModelWithProjection`` with ``hidden_size=32``
    and ``num_hidden_layers=2``, saves its ``state_dict()`` to a
    ``.safetensors`` file, and returns the path.

    Args:
        tmp_path: Pytest ``tmp_path`` fixture — the directory in which
            to write the checkpoint file.

    Returns:
        A ``pathlib.Path`` pointing to the saved ``clip_l_clip.safetensors``
        file inside *tmp_path*.

    Raises:
        ImportError: If ``torch`` or ``transformers`` is not installed
            in the current Python environment.
    """
    # Lazy imports — preserve mock-mode import isolation.
    import torch  # noqa: PLC0414

    from safetensors.torch import save_file  # noqa: PLC0414
    from transformers import (  # noqa: PLC0414
        CLIPTextConfig,
        CLIPTextModelWithProjection,
    )

    # Construct a minimal config for CLIP-L. The projection_dim parameter
    # is not overridden here — it defaults to hidden_size (32), which is
    # consistent with the real model's config where projection_dim ==
    # hidden_size == 768.
    config = CLIPTextConfig(hidden_size=32, num_hidden_layers=2)

    model = CLIPTextModelWithProjection(config)
    state_dict = model.state_dict()

    output_path = tmp_path / "clip_l_clip.safetensors"
    save_file(state_dict, str(output_path))

    return output_path


def tiny_t5_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny T5-XXL text-encoder checkpoint and return its path.

    Constructs a ``T5EncoderModel`` with ``d_model=32`` and
    ``num_layers=2``, saves its ``state_dict()`` to a ``.safetensors``
    file, and returns the path.

    Args:
        tmp_path: Pytest ``tmp_path`` fixture — the directory in which
            to write the checkpoint file.

    Returns:
        A ``pathlib.Path`` pointing to the saved ``t5_clip.safetensors``
        file inside *tmp_path*.

    Raises:
        ImportError: If ``torch`` or ``transformers`` is not installed
            in the current Python environment.
    """
    # Lazy imports — preserve mock-mode import isolation.
    import torch  # noqa: PLC0414

    from safetensors.torch import save_file  # noqa: PLC0414
    from transformers import (  # noqa: PLC0414
        T5Config,
        T5EncoderModel,
    )

    # T5 uses d_model/num_layers naming instead of hidden_size/
    # num_hidden_layers. All other parameters use the Config class
    # defaults (e.g. d_kv=64, d_ff=2048, num_heads=8).
    config = T5Config(d_model=32, num_layers=2)

    model = T5EncoderModel(config)
    state_dict = model.state_dict()

    output_path = tmp_path / "t5_clip.safetensors"
    save_file(state_dict, str(output_path))

    return output_path


# ---------------------------------------------------------------------------
# Tests: fixture verification
# ---------------------------------------------------------------------------


def test_fixtures_exist_and_return_path() -> None:
    """Verify all three fixtures are importable and return a pathlib.Path.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture (the fixtures themselves are lazy-import safe so this
        does not affect them).

    Tests:
        Import each fixture function and assert it is callable.

    Expected output:
        All three fixtures are importable without raising ``ImportError``
        — confirming lazy imports preserve mock-mode isolation.
    """
    from worker.tests.real_fixtures import (
        tiny_clip_l_clip,
        tiny_qwen3_clip,
        tiny_t5_clip,
    )

    assert callable(tiny_qwen3_clip)
    assert callable(tiny_clip_l_clip)
    assert callable(tiny_t5_clip)


def test_qwen3_checkpoint_loadable(tmp_path: pathlib.Path) -> None:
    """Verify the qwen3 fixture produces a valid safetensors checkpoint.

    Preconditions:
        ``torch`` and ``transformers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_qwen3_clip(tmp_path)``, load the resulting file with
        ``safetensors.torch.load_file``, and assert that tensors exist
        with shapes consistent with ``hidden_size=32`` and
        ``num_hidden_layers=2``.

    Expected output:
        The loaded state dict contains tensors whose shapes match the
        expected dimensions — confirming the checkpoint is valid and
        the model was built with the correct config.
    """
    from safetensors.torch import load_file

    from worker.tests.real_fixtures import tiny_qwen3_clip

    path = tiny_qwen3_clip(tmp_path)
    assert path.exists(), "Checkpoint file was not created"

    loaded = load_file(str(path))
    assert len(loaded) > 0, "State dict is empty"

    # Verify at least one embedding tensor has hidden_size=32.
    # The embedding weight is always present in Qwen3ForCausalLM.
    embed_key = "model.embed_tokens.weight"
    assert embed_key in loaded, f"Missing expected key: {embed_key}"
    assert loaded[embed_key].shape[1] == 32, (
        f"Expected embedding dim 32, got {loaded[embed_key].shape[1]}"
    )


def test_clip_l_checkpoint_loadable(tmp_path: pathlib.Path) -> None:
    """Verify the clip_l fixture produces a valid safetensors checkpoint.

    Preconditions:
        ``torch`` and ``transformers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_clip_l_clip(tmp_path)``, load the resulting file with
        ``safetensors.torch.load_file``, and assert tensors exist with
        shapes consistent with ``hidden_size=32``.

    Expected output:
        The loaded state dict contains tensors whose shapes match the
        expected dimensions for a CLIP text encoder with hidden_size=32.
    """
    from safetensors.torch import load_file

    from worker.tests.real_fixtures import tiny_clip_l_clip

    path = tiny_clip_l_clip(tmp_path)
    assert path.exists(), "Checkpoint file was not created"

    loaded = load_file(str(path))
    assert len(loaded) > 0, "State dict is empty"

    # The embed_tokens weight must have hidden_size=32 as its second dim.
    embed_key = "embed_tokens.weight"
    assert embed_key in loaded, f"Missing expected key: {embed_key}"
    assert loaded[embed_key].shape[1] == 32, (
        f"Expected embedding dim 32, got {loaded[embed_key].shape[1]}"
    )


def test_t5_checkpoint_loadable(tmp_path: pathlib.Path) -> None:
    """Verify the t5 fixture produces a valid safetensors checkpoint.

    Preconditions:
        ``torch`` and ``transformers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_t5_clip(tmp_path)``, load the resulting file with
        ``safetensors.torch.load_file``, and assert tensors exist with
        shapes consistent with ``d_model=32``.

    Expected output:
        The loaded state dict contains tensors whose shapes match the
        expected dimensions for a T5 encoder with d_model=32.
    """
    from safetensors.torch import load_file

    from worker.tests.real_fixtures import tiny_t5_clip

    path = tiny_t5_clip(tmp_path)
    assert path.exists(), "Checkpoint file was not created"

    loaded = load_file(str(path))
    assert len(loaded) > 0, "State dict is empty"

    # The encoder.embed_tokens weight must have d_model=32 as its second dim.
    embed_key = "encoder.embed_tokens.weight"
    assert embed_key in loaded, f"Missing expected key: {embed_key}"
    assert loaded[embed_key].shape[1] == 32, (
        f"Expected embedding dim 32, got {loaded[embed_key].shape[1]}"
    )
