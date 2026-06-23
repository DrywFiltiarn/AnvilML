#!/usr/bin/env python3
"""Real-path verification: EmptyLatent (Phase 18, task P18-D17).

Invokes EmptyLatent.execute() with a real model object (so the arch-dispatch
branch is taken instead of the mock-sentinel branch) and checks the
resulting noise tensor's shape against arch.diffusion.zit.compute_latent_shape().

Expected output (per worker/nodes/sampler.py + worker/nodes/arch/diffusion/zit.py):

    EmptyLatent(width=1024, height=1024, model=<RealModel arch=zit>)
        -> {"latent": torch.Tensor}
           shape == compute_latent_shape(batch_size, height, width, in_channels)
                 == (batch_size, in_channels, 128, 128) for a 1024x1024 image
                    at VAE_SCALE_FACTOR=8 with in_channels from the loaded model.
           dtype == torch.float32
           device == ctx.device

Known issue this script is expected to surface (see KNOWN_ISSUES.md item 2):
    worker/nodes/sampler.py's EmptyLatent.execute() real-mode branch ends with
        return {"latent": torch.randn(shape, dtype=torch.float32, device=ctx.device)}
    -- `ctx` is never bound in that scope (the method only has `self.ctx`).
    This is a plain NameError, not an architectural issue, and should be a
    one-line fix to `self.ctx.device`. Expect this step to FAIL with
    NameError: name 'ctx' is not defined until fixed.

Run standalone (after 01_loaders.py passes LoadModel):
    ANVILML_WORKER_MOCK=0 \\
    ANVILML_MODELS_DIR=/path/to/models \\
    ANVILML_ZIT_MODEL=zit_fp8.safetensors \\
    python3 03_empty_latent.py
"""

from __future__ import annotations

import _harness_common as h


def run_load_model():
    from worker.nodes.loader import LoadModel

    ctx = h.make_real_context()
    node = LoadModel(ctx)
    return node.execute(model_id=h.zit_model_path())["model"]


def run_empty_latent(model, width: int, height: int, batch_size: int = 1):
    from worker.nodes.sampler import EmptyLatent

    ctx = h.make_real_context()
    node = EmptyLatent(ctx)
    return node.execute(width=width, height=height, batch_size=batch_size, model=model)


def main() -> None:
    print(f"Device: {h.device()}")

    model = h.step("LoadModel (prerequisite)", run_load_model)
    if model is None:
        print("Cannot proceed without a real model object — aborting.")
        h.summary_and_exit()
        return

    width, height, batch_size = 1024, 1024, 1
    latent_out = h.step(
        f"EmptyLatent(width={width}, height={height}, batch_size={batch_size}, model=<real>)",
        lambda: run_empty_latent(model, width, height, batch_size),
    )

    if latent_out is not None:
        latent = latent_out["latent"]

        # Cross-check against the arch module's own shape formula so this
        # script fails if EmptyLatent and zit.py's compute_latent_shape()
        # ever disagree, not just if EmptyLatent crashes outright.
        from worker.nodes.arch.diffusion import zit as zit_arch

        expected_shape = zit_arch.compute_latent_shape(
            batch_size, height, width, model.in_channels
        )
        actual_shape = tuple(latent.shape)
        print(f"    expected shape (per compute_latent_shape): {expected_shape}")
        print(f"    actual shape   (per torch.randn output)  : {actual_shape}")
        assert actual_shape == expected_shape, (
            f"EmptyLatent produced {actual_shape}, but "
            f"compute_latent_shape() says {expected_shape}"
        )
        assert str(latent.dtype) == "torch.float32", (
            f"expected float32, got {latent.dtype}"
        )

    h.summary_and_exit()


if __name__ == "__main__":
    main()
