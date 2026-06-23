#!/usr/bin/env python3
"""Real-path verification: ClipTextEncode (Phase 18, task P18-D16).

Loads a real Qwen3 CLIP object (via LoadClip, same as 01_loaders.py) and
encodes a positive + negative prompt through RealClip.encode(), then
inspects the resulting Conditioning object's tensor shapes.

Expected output (per worker/nodes/encoder.py + RealClip.encode() in
worker/nodes/loader.py):

    ClipTextEncode -> {"conditioning": Conditioning}
        .positive -> list[torch.FloatTensor], one tensor per item in the
                     filtered (non-padding) sequence, each shaped
                     (seq_len, hidden_dim) where hidden_dim == 2560 for
                     Qwen3-4B (per the verbatim config in qwen3.py).
        .negative -> same shape contract as .positive.

Known issue this script is expected to surface (see KNOWN_ISSUES.md item 3):
    arch/diffusion/zit.py's sample() reads conditioning.tokenizer and
    conditioning.text_encoder via getattr(..., None) when assembling the
    ZImagePipeline in real mode -- but Conditioning (worker/nodes/encoder.py)
    only stores .positive/.negative. This script does not call sample()
    directly (that's 04_sampler.py's job) but documents the mismatch here
    since it originates at the Conditioning object this script produces.

Run standalone (after 01_loaders.py passes LoadClip):
    ANVILML_WORKER_MOCK=0 \\
    ANVILML_MODELS_DIR=/path/to/models \\
    ANVILML_ZIT_CLIP=qwen3_4b.safetensors \\
    python3 02_clip_encode.py
"""

from __future__ import annotations

import _harness_common as h

EXPECTED_HIDDEN_DIM = 2560  # Qwen3-4B hidden_size, per qwen3.py's config_values


def run_load_clip():
    from worker.nodes.loader import LoadClip

    ctx = h.make_real_context()
    node = LoadClip(ctx)
    return node.execute(model_id=h.zit_clip_path(), clip_type="qwen3")["clip"]


def run_encode(clip, text: str, negative_text: str):
    from worker.nodes.encoder import ClipTextEncode

    ctx = h.make_real_context()
    node = ClipTextEncode(ctx)
    return node.execute(clip=clip, text=text, negative_text=negative_text)


def main() -> None:
    print(f"Device: {h.device()}")

    clip = h.step("LoadClip (prerequisite)", run_load_clip)
    if clip is None:
        print("Cannot proceed without a real clip object — aborting.")
        h.summary_and_exit()
        return

    cond_out = h.step(
        "ClipTextEncode(text='a red fox in a snowy forest', negative_text='')",
        lambda: run_encode(clip, "a red fox in a snowy forest", ""),
    )

    if cond_out is not None:
        conditioning = cond_out["conditioning"]
        assert hasattr(conditioning, "positive"), "Conditioning must expose .positive"
        assert hasattr(conditioning, "negative"), "Conditioning must expose .negative"

        pos = conditioning.positive
        neg = conditioning.negative
        print(f"    positive: list of {len(pos)} tensor(s)")
        print(f"    negative: list of {len(neg)} tensor(s)")

        for i, t in enumerate(pos[:3]):  # sample first 3 to avoid flooding output
            print(f"      positive[{i}] -> {h.describe(t)}")
        for i, t in enumerate(neg[:3]):
            print(f"      negative[{i}] -> {h.describe(t)}")

        if pos:
            last_dim = tuple(pos[0].shape)[-1] if hasattr(pos[0], "shape") else None
            if last_dim is not None:
                assert last_dim == EXPECTED_HIDDEN_DIM, (
                    f"expected hidden_dim={EXPECTED_HIDDEN_DIM} for Qwen3-4B, "
                    f"got {last_dim}"
                )
                print(f"    hidden_dim check: {last_dim} == {EXPECTED_HIDDEN_DIM} OK")

    h.summary_and_exit()


if __name__ == "__main__":
    main()
