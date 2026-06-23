#!/usr/bin/env python3
"""Real-path verification: Sampler node + arch.diffusion.zit.sample() (P18-D18a/b/c/D19).

This is the script most relevant to the work currently in flight (D18c is
being implemented; D19 wires Sampler.execute() to call it). It exercises
TWO call sites, separately, because as of this writing Sampler.execute()'s
real-mode branch is a hard `raise NotImplementedError` (D19 not yet landed)
while arch.diffusion.zit.sample() itself is further along (through D18c):

    Call site A: arch.diffusion.zit.sample() directly
        -- this is what D19 will wire Sampler.execute() to call. Exercising
           it directly lets you verify D18a/b/c's pipeline assembly +
           invocation logic *before* D19 lands, without waiting on the
           Sampler.execute() wiring.

    Call site B: Sampler.execute() (the full node, via the real graph path)
        -- this is what worker_main.py's run_graph() actually calls. Once
           D19 lands, this should stop raising NotImplementedError and
           start producing the same tensor as call site A.

Expected output once both D18c and D19 are complete:

    sample(model, conditioning, latent, steps=4, cfg=1.0, seed=<int>,
           device=..., cancel_flag=..., emit_progress=..., vae=<real VAE>,
           pipeline_cache=<real PipelineCache>)
        -> (denoised_latent: torch.Tensor, resolved_seed: int)
           denoised_latent.shape == the input latent's shape (denoising
           does not change tensor shape, only values)

Known issues this script is expected to surface:

    KNOWN_ISSUES.md item 4 (call site A): arch/diffusion/zit.py's
    _make_callback() closure calls cancel_flag.is_set() (threading.Event
    API), but worker_main.py constructs cancel_flag as `[False]`
    (a list[bool], see worker_main.py:48). Once D18c wires the real
    pipeline invocation through _make_callback, the first denoising step
    will raise AttributeError: 'list' object has no attribute 'is_set'
    if this harness passes a list[bool] cancel_flag (which it does
    deliberately, to match production). Either zit.py must switch to
    list[bool] semantics (cancel_flag[0]), or worker_main.py must switch
    to threading.Event -- these two must agree before D18c can pass this
    check end-to-end.

    KNOWN_ISSUES.md item 3 (call site A): the loader_fn() inside sample()
    reads getattr(conditioning, "tokenizer", None) and
    getattr(conditioning, "text_encoder", None) -- but Conditioning
    (worker/nodes/encoder.py) has no such attributes, only .positive/
    .negative. Expect these to resolve to None via getattr's default,
    which means ZImagePipeline gets constructed with tokenizer=None,
    text_encoder=None. Whether that is tolerable depends on whether
    ZImagePipeline requires them at call time -- this script reports
    what actually happens rather than guessing.

    Call site B will simply FAIL with NotImplementedError until D19 lands;
    that is expected and this script reports it as such rather than
    treating it as a surprise.

Run standalone (after 01/02/03 pass):
    ANVILML_WORKER_MOCK=0 \\
    ANVILML_MODELS_DIR=/path/to/models \\
    ANVILML_ZIT_MODEL=zit_fp8.safetensors \\
    ANVILML_ZIT_VAE=zit_vae.safetensors \\
    ANVILML_ZIT_CLIP=qwen3_4b.safetensors \\
    python3 04_sampler.py
"""

from __future__ import annotations

import _harness_common as h


def build_prereqs():
    """Load real model, vae, clip, encode a prompt, and build an empty latent.

    Returns:
        dict with keys: model, vae, clip, conditioning, latent
    """
    from worker.nodes.loader import LoadModel, LoadVae, LoadClip
    from worker.nodes.encoder import ClipTextEncode
    from worker.nodes.sampler import EmptyLatent

    ctx = h.make_real_context()

    model = LoadModel(ctx).execute(model_id=h.zit_model_path())["model"]
    vae = LoadVae(ctx).execute(model_id=h.zit_vae_path())["vae"]
    clip = LoadClip(ctx).execute(model_id=h.zit_clip_path(), clip_type="qwen3")["clip"]
    conditioning = ClipTextEncode(ctx).execute(
        clip=clip, text="a red fox in a snowy forest", negative_text=""
    )["conditioning"]
    latent = EmptyLatent(ctx).execute(
        width=1024, height=1024, batch_size=1, model=model
    )["latent"]

    return {
        "model": model,
        "vae": vae,
        "clip": clip,
        "conditioning": conditioning,
        "latent": latent,
    }


def run_call_site_a(prereqs: dict):
    """Call arch.diffusion.zit.sample() directly (bypasses Sampler.execute())."""
    from worker.nodes.arch.diffusion import zit as zit_arch
    from worker.pipeline_cache import PipelineCache

    progress_log: list[tuple[int, int]] = []

    def emit_progress(step: int, total: int) -> None:
        progress_log.append((step, total))
        print(f"    [progress] step {step}/{total}")

    # Deliberately a list[bool], matching worker_main.py's production
    # wiring exactly (see worker_main.py:48 and KNOWN_ISSUES.md item 4).
    # If zit.py's callback expects .is_set(), this is where it breaks.
    cancel_flag = [False]

    result = zit_arch.sample(
        model=prereqs["model"],
        conditioning=prereqs["conditioning"],
        latent=prereqs["latent"],
        steps=4,
        cfg=1.0,
        seed=42,
        device=h.device(),
        cancel_flag=cancel_flag,
        emit_progress=emit_progress,
        vae=prereqs["vae"],
        pipeline_cache=PipelineCache(),
    )
    print(f"    progress events received: {len(progress_log)}")
    return result


def run_call_site_b(prereqs: dict):
    """Call Sampler.execute() — the actual node, as run_graph() would call it."""
    from worker.nodes.sampler import Sampler

    ctx = h.make_real_context()
    node = Sampler(ctx)
    return node.execute(
        model=prereqs["model"],
        conditioning=prereqs["conditioning"],
        latent=prereqs["latent"],
        steps=4,
        cfg=1.0,
        seed=42,
    )


def main() -> None:
    print(f"Device: {h.device()}")

    prereqs = h.step("Build prerequisites (model/vae/clip/conditioning/latent)",
                      build_prereqs)
    if prereqs is None:
        print("Cannot proceed without prerequisites — aborting.")
        h.summary_and_exit()
        return

    input_shape = tuple(prereqs["latent"].shape)
    print(f"    input latent shape: {input_shape}")

    a_result = h.step(
        "Call site A: arch.diffusion.zit.sample() direct",
        lambda: run_call_site_a(prereqs),
    )
    if a_result is not None:
        denoised, resolved_seed = a_result
        out_shape = tuple(denoised.shape) if hasattr(denoised, "shape") else None
        print(f"    output latent shape: {out_shape}  resolved_seed: {resolved_seed}")
        if out_shape is not None:
            assert out_shape == input_shape, (
                f"denoising changed tensor shape: {input_shape} -> {out_shape} "
                f"(expected unchanged)"
            )

    b_result = h.step(
        "Call site B: Sampler.execute() (full node)",
        lambda: run_call_site_b(prereqs),
    )
    if b_result is not None:
        denoised = b_result["latent"]
        resolved_seed = b_result["seed"]
        out_shape = tuple(denoised.shape) if hasattr(denoised, "shape") else None
        print(f"    output latent shape: {out_shape}  resolved_seed: {resolved_seed}")

    h.summary_and_exit()


if __name__ == "__main__":
    main()
