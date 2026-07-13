"""LoadModel node — loads a diffusion model from a safetensors file.

This node is the entry point for model loading in the AnvilML worker.
The mock branch returns a sentinel dict; the real branch is deferred
to P19-C2 which implements actual safetensors reading and arch dispatch.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


@register
class LoadModel(BaseNode):
    """Load a diffusion model from a safetensors file.

    This is the first node in the model-loading pipeline. In mock mode
    it returns a sentinel dict; in real mode it raises NotImplementedError
    pending P19-C2 which will implement actual safetensors reading and
    architecture dispatch.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Single input slot named "model_id" with type "STRING".
        OUTPUT_SLOTS: Single output slot named "model" with type "MODEL".
    """
    # defers_to: P19-C2 — real model loading logic (safetensors reading +
    # arch dispatch) is implemented in the subsequent task.
    NODE_TYPE = "LoadModel"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load Model"
    DESCRIPTION = "Loads a diffusion model from a safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadModel node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        is a bare placeholder that raises NotImplementedError.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the model to load.

        Returns:
            In mock mode: Dict with key "model" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Raises NotImplementedError (deferred to P19-C2).

        Raises:
            NotImplementedError: When ctx.mock is False — real model
                loading logic is deferred to P19-C2.
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"model": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: delegate to the pipeline cache so the
            # infrastructure (get_or_load + caching contract) is in
            # place for Phase 20. The loader_fn itself raises
            # NotImplementedError because no diffusion arch module
            # has been registered yet — real loading is deferred to
            # P20. The cache is not modified on exception per the
            # PipelineCache contract.
            return ctx.pipeline_cache.get_or_load(
                inputs["model_id"],
                lambda: (_ for _ in ()).throw(
                    NotImplementedError(
                        "no diffusion arch module registered yet"
                    )
                ),
            )
