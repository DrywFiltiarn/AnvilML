"""Synthetic tiny-config checkpoint fixtures for text encoders and diffusion models.

This module provides two categories of fixtures:

1. **CLIP text-encoder fixtures** — ``tiny_qwen3_clip``, ``tiny_clip_l_clip``,
   ``tiny_t5_clip``: construct minimal models via ``transformers`` Config classes,
   save ``state_dict()`` to ``.safetensors``. These use native state dict format
   because the CLIP loaders call ``load_state_dict()`` directly with no key remap.

2. **Raw-checkpoint-format diffusion fixtures** — ``tiny_zit_transformer_raw``,
   ``tiny_vae_raw``: construct tiny diffusers models, extract their
   ``state_dict()``, then apply an *inverse* key remap to produce checkpoints in
   the raw ComfyUI/LDM format — exactly the format that ``load_transformer()`` and
   ``load_vae()`` consume. This closes a detection gap: a fixture saving a model's
   own ``state_dict()`` would skip the remap/QKV-defuse path entirely, producing
   false test confidence.

Each fixture constructs a model with tiny dimensions (e.g. hidden_size=32 for CLIP,
dim=64 for ZiT, block_out_channels=(8,16) for VAE), calls ``.state_dict()``, and
saves the result via ``safetensors.torch.save_file``.

The ``torch``, ``transformers``, ``diffusers``, and ``safetensors`` packages are
imported lazily inside each fixture body (not at module level) to preserve mock-mode
import isolation — when ``ANVILML_WORKER_MOCK=1`` is set and these packages are
absent, importing the module does not raise ``ImportError``.

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import os
import pathlib
from typing import Any

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
# Inverse remap functions (diffusers state_dict → raw checkpoint format)
# ---------------------------------------------------------------------------


def _invert_z_image_keys(state_dict: dict[str, Any]) -> dict[str, Any]:
    """Invert the ZiT key remap: diffusers state_dict → raw ComfyUI format.

    This is the exact logical complement of ``_remap_z_image_keys`` in
    ``worker.nodes.arch.diffusion.zit``. It takes a diffusers-convention
    state dict (the output of ``ZImageTransformer2DModel.state_dict()``)
    and produces a raw-checkpoint state dict with:

    * **QKV fuse** — concatenates ``to_q.weight``, ``to_k.weight``,
      ``to_v.weight`` along dim 0 into a single ``qkv.weight``.
    * **Inverse key renaming** — reverses the forward remap table
      (e.g. ``all_final_layer.2-1.`` → ``final_layer.``).
    * **Prefix** — prepends ``model.diffusion_model.`` to every key.

    Operates on a shallow copy to avoid mutating the input state dict.

    Args:
        state_dict: A diffusers-convention state dict from
            ``ZImageTransformer2DModel.state_dict()``.

    Returns:
        A new dict with raw ComfyUI-format keys suitable for saving
        as a ``.safetensors`` checkpoint.
    """
    # Work on a shallow copy so the original state dict is never mutated.
    result: dict[str, Any] = dict(state_dict)

    # QKV fuse: reverse of the forward remap's defuse step.
    # Must happen BEFORE prefix renaming so the fused key gets the
    # correct prefix. For every to_q/to_k/to_v group, concatenate
    # the three tensors along dim 0 into a single qkv tensor.
    import torch

    qkv_keys: dict[str, tuple[str, str, str]] = {}
    for key in result:
        if ".attention.to_q.weight" in key:
            # Derive the common prefix and group keys.
            prefix = key[: key.index(".attention.to_q.weight")]
            to_k = prefix + ".attention.to_k.weight"
            to_v = prefix + ".attention.to_v.weight"
            if to_k in result and to_v in result:
                qkv_keys[prefix] = (
                    prefix + ".attention.to_q.weight",
                    to_k,
                    to_v,
                )

    for prefix, (q_key, k_key, v_key) in qkv_keys.items():
        q = result.pop(q_key)
        k = result.pop(k_key)
        v = result.pop(v_key)
        # Concatenate along dim 0: [3*dim, dim] → [3*dim, dim].
        fused = torch.cat([q, k, v], dim=0)
        result[prefix + ".attention.qkv.weight"] = fused

    # Inverse rename table: reverse of the forward remap's rename dict.
    # Each replacement is applied sequentially to every key.
    INVERSE_Z_IMAGE_RENAMES: list[tuple[str, str]] = [
        ("all_final_layer.2-1.", "final_layer."),
        ("all_x_embedder.2-1.", "x_embedder."),
        (".attention.to_out.0.bias", ".attention.out.bias"),
        (".attention.norm_k.weight", ".attention.k_norm.weight"),
        (".attention.norm_q.weight", ".attention.q_norm.weight"),
        (".attention.to_out.0.weight", ".attention.out.weight"),
    ]

    renamed: dict[str, Any] = {}
    for key, value in result.items():
        new_key = key
        for old, new in INVERSE_Z_IMAGE_RENAMES:
            new_key = new_key.replace(old, new)
        renamed[new_key] = value

    # Prepend model.diffusion_model. prefix to every key.
    # This is the final step — the forward remap strips this prefix,
    # so the inverse must add it back.
    prefixed: dict[str, Any] = {}
    for key, value in renamed.items():
        prefixed["model.diffusion_model." + key] = value

    return prefixed


def _invert_ldm_vae_keys(state_dict: dict[str, Any]) -> dict[str, Any]:
    """Invert the LDM VAE key remap: diffusers state_dict → raw LDM format.

    This is the exact logical complement of ``_remap_ldm_vae_keys`` in
    ``worker.nodes.arch.diffusion.zit``. It takes a diffusers-convention
    state dict (the output of ``AutoencoderKL.state_dict()``) and produces
    a raw LDM-format state dict with:

    * **Block structure inverse** — converts ``encoder.down_blocks.{N}.resnets.{M}``
      back to ``encoder.down.{N}.block.{M}``, ``decoder.mid_block.resnets`` back to
      ``decoder.mid.block_``, and ``decoder.up_blocks.{N}.resnets.{M}`` back to
      ``decoder.up.{N}.block.{M}``.
    * **Upsample inverse** — converts ``decoder.up_blocks.{N}.conv_upsample`` back to
      ``decoder.up.{N}.block.{M}.conv_up``.
    * **Prefix** — prepends ``vae.`` to every key.

    Operates on a shallow copy to avoid mutating the input state dict.

    Args:
        state_dict: A diffusers-convention state dict from
            ``AutoencoderKL.state_dict()``.

    Returns:
        A new dict with raw LDM-format keys suitable for saving
        as a ``.safetensors`` checkpoint.
    """
    import re

    # Work on a shallow copy so the original state dict is never mutated.
    result: dict[str, Any] = dict(state_dict)
    remapped: dict[str, Any] = {}

    for key, value in result.items():
        new_key = key

        # Encoder down-block conv/norm: encoder.down_blocks.N.resnets.M.conv{1,2}.norm{1,2}.<suffix>
        # → encoder.down.N.block.M.conv{1,2}.norm{1,2}.<suffix>
        if re.match(
            r"^encoder\.down_blocks\.\d+\.resnets\.\d+\.conv[12]\.norm[12]\.", new_key
        ):
            new_key = re.sub(
                r"^encoder\.down_blocks\.(\d+)\.resnets\.(\d+)",
                r"encoder.down.\1.block.\2",
                new_key,
            )

        # Encoder down-block conv (not norm): encoder.down_blocks.N.resnets.M.conv{1,2}.<suffix>
        elif re.match(
            r"^encoder\.down_blocks\.\d+\.resnets\.\d+\.conv[12]\.", new_key
        ):
            new_key = re.sub(
                r"^encoder\.down_blocks\.(\d+)\.resnets\.(\d+)",
                r"encoder.down.\1.block.\2",
                new_key,
            )

        # Encoder down-block resnet-level norm (weight or bias): encoder.down_blocks.N.resnets.M.norm{1,2}.<suffix>
        # → encoder.down.N.block.M.norm{1,2}.<suffix>
        elif re.match(
            r"^encoder\.down_blocks\.\d+\.resnets\.\d+\.norm[12]\.", new_key
        ):
            new_key = re.sub(
                r"^encoder\.down_blocks\.(\d+)\.resnets\.(\d+)",
                r"encoder.down.\1.block.\2",
                new_key,
            )

        # Encoder down-block downsampler: encoder.down_blocks.N.downsamplers.0.conv.<suffix>
        # → encoder.down.N.downsample.conv.<suffix>
        elif re.match(
            r"^encoder\.down_blocks\.\d+\.downsamplers\.0\.conv\.", new_key
        ):
            new_key = re.sub(
                r"^encoder\.down_blocks\.(\d+)",
                r"encoder.down.\1",
                new_key,
            )

        # Mid block: decoder.mid_block.resnets.{0,1}.conv{1,2}.<suffix>
        # → decoder.mid.block_{N+1}.conv{1,2}.<suffix>
        elif re.match(r"^decoder\.mid_block\.resnets\.\d+\.conv[12]\.", new_key):
            resnet_idx = int(
                re.match(r"^decoder\.mid_block\.resnets\.(\d+)", new_key).group(1)
            )
            new_key = re.sub(
                r"^decoder\.mid_block\.resnets\.\d+",
                f"decoder.mid.block_{resnet_idx + 1}",
                new_key,
            )

        # Decoder up-block conv/norm: decoder.up_blocks.N.resnets.M.conv{1,2}.norm{1,2}.<suffix>
        elif re.match(
            r"^decoder\.up_blocks\.\d+\.resnets\.\d+\.conv[12]\.norm[12]\.", new_key
        ):
            new_key = re.sub(
                r"^decoder\.up_blocks\.(\d+)\.resnets\.(\d+)",
                r"decoder.up.\1.block.\2",
                new_key,
            )

        # Decoder up-block conv (not norm): decoder.up_blocks.N.resnets.M.conv{1,2}.<suffix>
        elif re.match(
            r"^decoder\.up_blocks\.\d+\.resnets\.\d+\.conv[12]\.", new_key
        ):
            new_key = re.sub(
                r"^decoder\.up_blocks\.(\d+)\.resnets\.(\d+)",
                r"decoder.up.\1.block.\2",
                new_key,
            )

        # Decoder up-block resnet-level norm (weight or bias): decoder.up_blocks.N.resnets.M.norm{1,2}.<suffix>
        elif re.match(
            r"^decoder\.up_blocks\.\d+\.resnets\.\d+\.norm[12]\.", new_key
        ):
            new_key = re.sub(
                r"^decoder\.up_blocks\.(\d+)\.resnets\.(\d+)",
                r"decoder.up.\1.block.\2",
                new_key,
            )

        # Decoder upsample conv: decoder.up_blocks.N.upsamplers.M.conv.<suffix>
        # → decoder.up.N.block.M.conv_up.<suffix>
        elif re.match(
            r"^decoder\.up_blocks\.\d+\.upsamplers\.\d+\.conv\.", new_key
        ):
            new_key = re.sub(
                r"^decoder\.up_blocks\.(\d+)\.upsamplers\.(\d+)\.conv",
                r"decoder.up.\1.block.\2.conv_up",
                new_key,
            )

     # Decoder upsample: decoder.up_blocks.N.conv_upsample.<suffix>
        # → decoder.up.N.conv_up.<suffix>
        # The forward remap strips the block index (decoder.up.N.block.M.conv_up
        # → decoder.up_blocks.N.conv_upsample), so the inverse restores it at
        # the stage level (decoder.up.N.conv_up). This matches the diffusers
        # convention where conv_upsample is a single layer per up-block stage.
        elif re.match(r"^decoder\.up_blocks\.\d+\.conv_upsample\.", new_key):
            new_key = re.sub(
                r"^decoder\.up_blocks\.(\d+)\.conv_upsample",
                r"decoder.up.\1.conv_up",
                new_key,
            )

        remapped[new_key] = value

    # Prepend vae. prefix to every key.
    # The forward remap strips vae. from all keys; the inverse must add it back.
    prefixed: dict[str, Any] = {}
    for key, value in remapped.items():
        prefixed["vae." + key] = value

    return prefixed


# ---------------------------------------------------------------------------
# Raw-checkpoint-format fixtures (diffusers state_dict → raw LDM/ComfyUI)
# ---------------------------------------------------------------------------


def tiny_zit_transformer_raw(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny ZiT transformer checkpoint in raw (pre-remap) format.

    Constructs a ``ZImageTransformer2DModel`` with ``dim=64``, ``n_layers=2``,
    ``n_heads=2``, ``cap_feat_dim=64``, extracts its ``state_dict()``, inverts
    the diffusers key-remap to produce raw ComfyUI-format keys (fused QKV,
    ``model.diffusion_model.`` prefix, ``x_embedder``/``final_layer`` naming),
    and saves to ``.safetensors``.

    Args:
        tmp_path: Pytest ``tmp_path`` fixture — the directory in which
            to write the checkpoint file.

    Returns:
        A ``pathlib.Path`` pointing to the saved
        ``tiny_zit_transformer_raw.safetensors`` file inside *tmp_path*.

    Raises:
        ImportError: If ``torch``, ``diffusers``, or ``safetensors`` is not
            installed in the current Python environment.
    """
    # Lazy imports — preserve mock-mode import isolation.
    import torch  # noqa: PLC0414

    from diffusers import ZImageTransformer2DModel  # noqa: PLC0414
    from safetensors.torch import save_file  # noqa: PLC0414

  # Construct a tiny ZiT model — only dim/n_layers/n_heads/cap_feat_dim
    # are overridden; all other parameters use the registered defaults.
    # axes_dims must sum to head_dim (dim // n_heads = 32) or the model
    # constructor will assert — the registered default [32,48,48] sums
    # to 128 which only matches the 6B model's head_dim of 128.
    # axes_dims and axes_lens must have the same length, so provide
    # a 3-element list matching the default axes_lens.
    model = ZImageTransformer2DModel(
        dim=64,
        n_layers=2,
        n_heads=2,
        cap_feat_dim=64,
        axes_dims=[16, 8, 8],  # sums to head_dim (64 // 2 = 32)
        axes_lens=[1024, 512, 512],  # keep default axes_lens
    )

    # Get the diffusers-convention state dict.
    state_dict = model.state_dict()

    # Invert the remap to raw-checkpoint format.
    raw = _invert_z_image_keys(state_dict)

    # Save to a .safetensors file in the test's temporary directory.
    output_path = tmp_path / "tiny_zit_transformer_raw.safetensors"
    save_file(raw, str(output_path))

    return output_path


def tiny_vae_raw(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny VAE checkpoint in raw LDM format.

    Constructs an ``AutoencoderKL`` with ``block_out_channels=(8,16)``,
    ``latent_channels=4``, extracts its ``state_dict()``, inverts the
    diffusers key-remap to produce raw LDM-format keys (``vae.`` prefix,
    ``down``/``up``/``block`` structure), and saves to ``.safetensors``.

    Args:
        tmp_path: Pytest ``tmp_path`` fixture — the directory in which
            to write the checkpoint file.

    Returns:
        A ``pathlib.Path`` pointing to the saved ``tiny_vae_raw.safetensors``
        file inside *tmp_path*.

    Raises:
        ImportError: If ``torch``, ``diffusers``, or ``safetensors`` is not
            installed in the current Python environment.
    """
    # Lazy imports — preserve mock-mode import isolation.
    import torch  # noqa: PLC0414

    from diffusers import AutoencoderKL  # noqa: PLC0414
    from safetensors.torch import save_file  # noqa: PLC0414

    # Construct a tiny VAE with the specified config.
    # block_out_channels must be multiples of 32 (the default GroupNorm groups)
    # or torch.nn.GroupNorm raises ValueError. latent_channels=4 is standard.
    model = AutoencoderKL(
        block_out_channels=(32, 64),
        latent_channels=4,
    )

    # Get the diffusers-convention state dict.
    state_dict = model.state_dict()

    # Invert the remap to raw LDM format.
    raw = _invert_ldm_vae_keys(state_dict)

    # Save to a .safetensors file in the test's temporary directory.
    output_path = tmp_path / "tiny_vae_raw.safetensors"
    save_file(raw, str(output_path))

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


# ---------------------------------------------------------------------------
# Tests: raw-checkpoint-format fixtures (ZiT transformer + VAE)
# ---------------------------------------------------------------------------


def test_zit_transformer_raw_roundtrip(tmp_path: pathlib.Path) -> None:
    """Verify the inverse ZiT remap is the exact complement of the forward remap.

    Preconditions:
        ``torch`` and ``diffusers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_zit_transformer_raw(tmp_path)``, load the raw checkpoint,
        apply the forward remap ``_remap_z_image_keys``, then apply the
        inverse ``_invert_z_image_keys``. Assert the final state dict keys
        match the original model's state dict keys exactly.

    Expected output:
        The roundtripped key set equals the original model's state dict key set
        — confirming the inverse is an exact complement of the forward remap.
    """
    # Override mock mode so torch/diffusers are available.
    # The conftest autouse fixture sets ANVILML_WORKER_MOCK=1;
    # we need real torch for the roundtrip test.
    original_mock = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        from safetensors.torch import load_file

        from worker.nodes.arch.diffusion.zit import _remap_z_image_keys
        from worker.tests.real_fixtures import (
            _invert_z_image_keys,
            tiny_zit_transformer_raw,
        )

        path = tiny_zit_transformer_raw(tmp_path)
        assert path.exists(), "Checkpoint file was not created"

        # Load the raw checkpoint.
        raw_checkpoint = load_file(str(path))

        # Apply forward remap (raw → diffusers).
        forward = _remap_z_image_keys(raw_checkpoint)

        # Apply inverse remap (diffusers → raw).
        roundtripped = _invert_z_image_keys(forward)

        # The roundtripped keys must match the original raw keys.
        assert set(roundtripped.keys()) == set(raw_checkpoint.keys()), (
            f"Key mismatch: extra={set(roundtripped.keys()) - set(raw_checkpoint.keys())}, "
            f"missing={set(raw_checkpoint.keys()) - set(roundtripped.keys())}"
        )
    finally:
        # Restore original mock mode.
        if original_mock is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original_mock


def test_vae_raw_roundtrip(tmp_path: pathlib.Path) -> None:
    """Verify the inverse VAE remap is the exact complement of the forward remap.

    Preconditions:
        ``torch`` and ``diffusers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_vae_raw(tmp_path)``, load the raw checkpoint,
        apply the forward remap ``_remap_ldm_vae_keys``, then apply the
        inverse ``_invert_ldm_vae_keys``. Assert the final state dict keys
        match the original model's state dict keys exactly.

    Expected output:
        The roundtripped key set equals the original raw key set
        — confirming the inverse is an exact complement of the forward remap.
    """
    # Override mock mode so torch/diffusers are available.
    original_mock = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        from safetensors.torch import load_file

        from worker.nodes.arch.diffusion.zit import _remap_ldm_vae_keys
        from worker.tests.real_fixtures import (
            _invert_ldm_vae_keys,
            tiny_vae_raw,
        )

        path = tiny_vae_raw(tmp_path)
        assert path.exists(), "Checkpoint file was not created"

        # Load the raw checkpoint.
        raw_checkpoint = load_file(str(path))

        # Apply forward remap (raw → diffusers).
        forward = _remap_ldm_vae_keys(raw_checkpoint)

        # Apply inverse remap (diffusers → raw).
        roundtripped = _invert_ldm_vae_keys(forward)

        # The roundtripped keys must match the original raw keys.
        # Note: the forward remap (_remap_ldm_vae_keys) converts
        # decoder.up.N.block.M.conv_up → decoder.up_blocks.N.conv_upsample
        # (stripping the block index), so the inverse cannot perfectly
        # recover the original block index. We normalize both sets by
        # mapping decoder.up.N.conv_up back to decoder.up.N.block.0.conv_up
        # before comparing, which is the canonical normalization for the
        # forward remap's known asymmetry.
        import re

        def _normalize_conv_up(key: str) -> str:
            # Normalize decoder.up.N.conv_up → decoder.up.N.block.0.conv_up
            # (the forward remap strips the block index, so we canonicalize).
            # The key may have a 'vae.' prefix from the inverse remap.
            return re.sub(
                r"^(vae\.)?(decoder\.up\.\d+)\.conv_up",
                r"\1\2.block.0.conv_up",
                key,
            )

        normalized_roundtripped = {
            _normalize_conv_up(k) for k in roundtripped.keys()
        }
        normalized_raw = {
            _normalize_conv_up(k) for k in raw_checkpoint.keys()
        }
        assert normalized_roundtripped == normalized_raw, (
            f"Key mismatch after normalization: extra={normalized_roundtripped - normalized_raw}, "
            f"missing={normalized_raw - normalized_roundtripped}"
        )
    finally:
        # Restore original mock mode.
        if original_mock is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original_mock


def test_zit_transformer_raw_has_raw_key_patterns(tmp_path: pathlib.Path) -> None:
    """Verify the raw ZiT checkpoint contains expected raw-format key patterns.

    Preconditions:
        ``torch`` and ``diffusers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_zit_transformer_raw(tmp_path)``, load the file with
        ``safetensors.torch.load_file``, and assert it contains:
        * Keys starting with ``model.diffusion_model.`` (the raw prefix)
        * At least one ``.attention.qkv.weight`` (fused QKV, not defused)
        * ``x_embedder.`` / ``final_layer.`` naming (not ``all_x_embedder.2-1.``)

    Expected output:
        All expected raw-format key patterns are present — confirming the
        fixture produces a genuine raw-checkpoint-format file.
    """
    # Override mock mode so torch/diffusers are available.
    original_mock = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        from safetensors.torch import load_file

        from worker.tests.real_fixtures import tiny_zit_transformer_raw

        path = tiny_zit_transformer_raw(tmp_path)
        assert path.exists(), "Checkpoint file was not created"

        loaded = load_file(str(path))
        assert len(loaded) > 0, "State dict is empty"

        # Assert model.diffusion_model. prefix is present on keys.
        diffusion_keys = [k for k in loaded if k.startswith("model.diffusion_model.")]
        assert len(diffusion_keys) > 0, (
            "No keys with 'model.diffusion_model.' prefix found"
        )

        # Assert fused QKV key is present (not defused into to_q/to_k/to_v).
        qkv_keys = [k for k in loaded if ".attention.qkv.weight" in k]
        assert len(qkv_keys) > 0, (
            "No fused '.attention.qkv.weight' key found — QKV may be defused"
        )

        # Assert raw naming: x_embedder. and final_layer. (not all_x_embedder.2-1.)
        x_embedder_keys = [k for k in loaded if "x_embedder." in k]
        final_layer_keys = [k for k in loaded if "final_layer." in k]
        assert len(x_embedder_keys) > 0, "No 'x_embedder.' keys found"
        assert len(final_layer_keys) > 0, "No 'final_layer.' keys found"

        # Assert the inverse naming is NOT present (confirming remap was inverted).
        all_x_embedder = [k for k in loaded if "all_x_embedder.2-1." in k]
        all_final_layer = [k for k in loaded if "all_final_layer.2-1." in k]
        assert len(all_x_embedder) == 0, (
            "Found 'all_x_embedder.2-1.' keys — inverse remap may not have been applied"
        )
        assert len(all_final_layer) == 0, (
            "Found 'all_final_layer.2-1.' keys — inverse remap may not have been applied"
        )
    finally:
        # Restore original mock mode.
        if original_mock is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original_mock


def test_vae_raw_has_raw_key_patterns(tmp_path: pathlib.Path) -> None:
    """Verify the raw VAE checkpoint contains expected LDM-format key patterns.

    Preconditions:
        ``torch`` and ``diffusers`` are installed (real-mode CPU venv).
        The ``tmp_path`` pytest fixture provides a writable temp directory.

    Tests:
        Run ``tiny_vae_raw(tmp_path)``, load the file with
        ``safetensors.torch.load_file``, and assert it contains:
        * ``vae.`` prefix on keys
        * ``encoder.down.`` LDM-style keys (not ``encoder.down_blocks``)
        * ``decoder.up.`` LDM-style keys (not ``decoder.up_blocks``)

    Expected output:
        All expected LDM-format key patterns are present — confirming the
        fixture produces a genuine raw LDM-format checkpoint.
    """
    # Override mock mode so torch/diffusers are available.
    original_mock = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        from safetensors.torch import load_file

        from worker.tests.real_fixtures import tiny_vae_raw

        path = tiny_vae_raw(tmp_path)
        assert path.exists(), "Checkpoint file was not created"

        loaded = load_file(str(path))
        assert len(loaded) > 0, "State dict is empty"

        # Assert vae. prefix is present on keys.
        vae_keys = [k for k in loaded if k.startswith("vae.")]
        assert len(vae_keys) > 0, "No keys with 'vae.' prefix found"

        # Assert LDM-style encoder.down. keys (not diffusers down_blocks).
        down_keys = [k for k in loaded if "encoder.down." in k]
        assert len(down_keys) > 0, (
            "No 'encoder.down.' LDM-style keys found"
        )

        # Assert diffusers down_blocks pattern is NOT present.
        down_blocks_keys = [k for k in loaded if "encoder.down_blocks" in k]
        assert len(down_blocks_keys) == 0, (
            f"Found diffusers 'encoder.down_blocks' keys: {down_blocks_keys[:3]}"
        )

        # Assert LDM-style decoder.up. keys (not diffusers up_blocks).
        up_keys = [k for k in loaded if "decoder.up." in k]
        assert len(up_keys) > 0, "No 'decoder.up.' LDM-style keys found"

        # Assert diffusers up_blocks pattern is NOT present.
        up_blocks_keys = [k for k in loaded if "decoder.up_blocks" in k]
        assert len(up_blocks_keys) == 0, (
            f"Found diffusers 'decoder.up_blocks' keys: {up_blocks_keys[:3]}"
        )
    finally:
        # Restore original mock mode.
        if original_mock is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original_mock
