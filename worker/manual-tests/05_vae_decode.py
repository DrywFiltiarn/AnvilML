#!/usr/bin/env python3
"""Real-path verification: VaeDecode (Phase 18, task P18-D20).

Decodes a latent tensor (a real noise latent from EmptyLatent, since
Sampler's real path may not be wired yet — see 04_sampler.py) through the
real VAE and inspects the resulting image.

Expected output (per worker/nodes/decode.py once D20 lands):

    VaeDecode(vae=<real AutoencoderKL>, latent=<torch.Tensor>)
        -> {"image": PIL.Image.Image}
           size == (width, height) matching the original EmptyLatent request
           mode == "RGB"

As of this writing VaeDecode.execute()'s real-mode branch is a hard
`raise NotImplementedError` (D20 not yet landed) — this script reports
that as an expected, named failure rather than an unexplained crash, and
should be re-run once D20 lands to confirm it then PASSes.

Run standalone (after 01/03 pass):
    ANVILML_WORKER_MOCK=0 \\
    ANVILML_MODELS_DIR=/path/to/models \\
    ANVILML_ZIT_MODEL=zit_fp8.safetensors \\
    ANVILML_ZIT_VAE=zit_vae.safetensors \\
    python3 05_vae_decode.py
"""

from __future__ import annotations

import _harness_common as h


def build_prereqs():
    """Load a real VAE and a real latent tensor (bypassing Sampler)."""
    from worker.nodes.loader import LoadModel, LoadVae
    from worker.nodes.sampler import EmptyLatent

    ctx = h.make_real_context()

    model = LoadModel(ctx).execute(model_id=h.zit_model_path())["model"]
    vae = LoadVae(ctx).execute(model_id=h.zit_vae_path())["vae"]
    latent = EmptyLatent(ctx).execute(
        width=1024, height=1024, batch_size=1, model=model
    )["latent"]

    return {"vae": vae, "latent": latent, "width": 1024, "height": 1024}


def run_vae_decode(prereqs: dict):
    from worker.nodes.decode import VaeDecode

    ctx = h.make_real_context()
    node = VaeDecode(ctx)
    return node.execute(vae=prereqs["vae"], latent=prereqs["latent"])


def main() -> None:
    print(f"Device: {h.device()}")

    prereqs = h.step("Build prerequisites (vae + raw noise latent)", build_prereqs)
    if prereqs is None:
        print("Cannot proceed without prerequisites — aborting.")
        h.summary_and_exit()
        return

    decode_out = h.step("VaeDecode.execute()", lambda: run_vae_decode(prereqs))

    if decode_out is not None:
        image = decode_out["image"]
        if hasattr(image, "size"):
            print(f"    decoded image size: {image.size}  mode: {getattr(image, 'mode', '?')}")
            expected_size = (prereqs["width"], prereqs["height"])
            assert image.size == expected_size, (
                f"expected decoded image size {expected_size}, got {image.size}"
            )

    h.summary_and_exit()


if __name__ == "__main__":
    main()
