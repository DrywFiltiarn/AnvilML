#!/usr/bin/env python3
"""Real-path verification: LoadModel, LoadVae, LoadClip (Phase 18, group A/D).

Invokes each loader node's execute() directly against real .safetensors
files, with ANVILML_WORKER_MOCK unset, and prints the resulting object's
type and key attributes.

Expected output shapes (per docs/ANVILML_DESIGN.md §10.3 and the current
worker/nodes/loader.py implementation):

    LoadModel -> {"model": RealModel}
        .arch        -> str, e.g. "zit"
        .in_channels -> int, read from transformer.config.in_channels

    LoadVae -> {"vae": diffusers.AutoencoderKL}
        loaded via AutoencoderKL.from_single_file()

    LoadClip(clip_type="qwen3") -> {"clip": RealClip}
        .tokenizer    -> transformers.Qwen2Tokenizer
        .text_encoder -> transformers.Qwen3ForCausalLM

Known issues this script is expected to surface (see KNOWN_ISSUES.md items 1 and 5):
    1. worker/nodes/arch/clip/qwen3.py resolves its tokenizer asset directory
       as Path(__file__).parent.parent / "assets" / "qwen25_tokenizer", which
       from worker/nodes/arch/clip/qwen3.py resolves to
       worker/nodes/arch/assets/qwen25_tokenizer — but the real directory is
       worker/assets/qwen25_tokenizer (one level higher; worker/nodes/arch/clip/t5.py
       uses .parent.parent.parent and is correct).
    5. worker/nodes/loader.py:618 (LoadClip.execute()) calls
       torch.bfloat16 without ever importing torch in that scope — this is
       a plain NameError that fires BEFORE item 1's bug has a chance to,
       since the torch_dtype argument is evaluated before module.load() is
       entered. Expect LoadClip to fail with NameError: name 'torch' is not
       defined first; only after that's fixed will item 1's OSError surface.

Run standalone:
    ANVILML_WORKER_MOCK=0 \\
    ANVILML_MODELS_DIR=/path/to/models \\
    ANVILML_ZIT_MODEL=zit_fp8.safetensors \\
    ANVILML_ZIT_VAE=zit_vae.safetensors \\
    ANVILML_ZIT_CLIP=qwen3_4b.safetensors \\
    python3 01_loaders.py
"""

from __future__ import annotations

import _harness_common as h


def run_load_model() -> dict:
    """Invoke LoadModel.execute() against the real ZiT diffusion file."""
    from worker.nodes.loader import LoadModel

    ctx = h.make_real_context()
    node = LoadModel(ctx)
    return node.execute(model_id=h.zit_model_path())


def run_load_vae() -> dict:
    """Invoke LoadVae.execute() against the real ZiT VAE file."""
    from worker.nodes.loader import LoadVae

    ctx = h.make_real_context()
    node = LoadVae(ctx)
    return node.execute(model_id=h.zit_vae_path())


def run_load_clip() -> dict:
    """Invoke LoadClip.execute() against the real Qwen3 text encoder file."""
    from worker.nodes.loader import LoadClip

    ctx = h.make_real_context()
    node = LoadClip(ctx)
    return node.execute(model_id=h.zit_clip_path(), clip_type="qwen3")


def main() -> None:
    print(f"Models dir : {h.models_dir()}")
    print(f"Device     : {h.device()}")

    model_out = h.step("LoadModel(zit fp8 .safetensors)", run_load_model)
    if model_out is not None:
        model = model_out["model"]
        assert hasattr(model, "arch"), "RealModel must expose .arch"
        assert model.arch == "zit", f"expected arch='zit', got {model.arch!r}"
        assert isinstance(model.in_channels, int), (
            "RealModel.in_channels must be an int "
            f"(got {type(model.in_channels)})"
        )
        print(f"    arch={model.arch!r} in_channels={model.in_channels}")

    vae_out = h.step("LoadVae(zit vae .safetensors)", run_load_vae)
    if vae_out is not None:
        vae = vae_out["vae"]
        # AutoencoderKL exposes .config.latent_channels — cross-check
        # against the VAE_SCALE_FACTOR=8 assumption baked into
        # arch/diffusion/zit.py's compute_latent_shape().
        if hasattr(vae, "config"):
            print(f"    vae.config keys of interest: "
                  f"latent_channels={getattr(vae.config, 'latent_channels', '?')}, "
                  f"block_out_channels={getattr(vae.config, 'block_out_channels', '?')}")

    clip_out = h.step("LoadClip(qwen3 .safetensors, clip_type='qwen3')", run_load_clip)
    if clip_out is not None:
        clip = clip_out["clip"]
        assert hasattr(clip, "tokenizer"), "RealClip must expose .tokenizer"
        assert hasattr(clip, "text_encoder"), "RealClip must expose .text_encoder"
        print(f"    tokenizer={type(clip.tokenizer).__name__} "
              f"text_encoder={type(clip.text_encoder).__name__}")

    h.summary_and_exit()


if __name__ == "__main__":
    main()
