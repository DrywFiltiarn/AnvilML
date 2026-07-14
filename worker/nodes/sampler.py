"""Sampler node — runs a denoising diffusion step to produce a latent.

The Sampler node takes a model, conditioning, CLIP, latent, steps, cfg, and seed
as inputs and produces a latent and seed as outputs. In mock mode it returns a
sentinel dict with the input latent's shape and the resolved seed. The real
branch is deferred to P21-C2 which will dispatch to the arch.diffusion
get_module(model.arch).sample() call.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


@register
class Sampler(BaseNode):
    """A generic sampling node that runs a denoising diffusion step.

    Takes a model, conditioning, CLIP, latent, steps, cfg, and seed as inputs,
    and produces a latent and seed as outputs. Both mock and real branches are
    exercised by dedicated tests per the dual-mode parity marker convention
    (ANVILML_DESIGN.md §10.6).

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Seven input slots — model, conditioning, clip, latent,
            steps, cfg, and seed.
        OUTPUT_SLOTS: Two output slots — latent and seed.
    """
    NODE_TYPE = "Sampler"
    CATEGORY = "Sampling"
    DISPLAY_NAME = "Sampler"
    DESCRIPTION = (
        "Runs a denoising diffusion step to produce a latent from a model, "
        "conditioning, and latent input."
    )
    INPUT_SLOTS = [
        SlotSpec("model", "MODEL"),
        SlotSpec("conditioning", "CONDITIONING"),
        SlotSpec("clip", "CLIP"),
        SlotSpec("latent", "LATENT"),
        SlotSpec("steps", "INT"),
        SlotSpec("cfg", "FLOAT"),
        SlotSpec("seed", "INT"),
    ]
    OUTPUT_SLOTS = [
        SlotSpec("latent", "LATENT"),
        SlotSpec("seed", "INT"),
    ]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the sampler node.

        Branches on the mock flag at the top: mock mode returns a sentinel
        dict with the input latent's shape and a deterministically resolved
        seed (-1 maps to 0), while the real branch is deferred to P21-C2.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Named input values keyed by slot name. Must contain
                "model", "conditioning", "clip", "latent", "steps", "cfg",
                and "seed".

        Returns:
            Dict with keys "latent" (dict with mock sentinel and shape)
            and "seed" (int, resolved from input).

        Raises:
            NotImplementedError: When ctx.mock is False — the real branch
                is deferred to P21-C2 which will dispatch to the arch module.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with the input latent's
            # shape and the deterministically resolved seed. This ensures
            # reproducible mock-mode test output regardless of the seed
            # value passed in (-1 always maps to 0).
            seed = inputs["seed"] if inputs["seed"] != -1 else 0
            return {
                "latent": {"mock": True, "shape": inputs["latent"].get("shape")},
                "seed": seed,
            }
        else:
            # Real branch: deferred to P21-C2 — dispatches to
            # arch.diffusion.get_module(model.arch).sample().
            # defers_to: P21-C2
            raise NotImplementedError(
                "Sampler real branch deferred to P21-C2 — "
                "dispatches to arch.diffusion.get_module(model.arch).sample()"
            )
