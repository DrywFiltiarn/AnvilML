"""PassThrough node — trivial no-op passthrough for testing the dispatch pipeline."""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register


@register
class PassThrough(BaseNode):
    """A trivial no-op node that passes its input value through unchanged.

    Used to verify the dispatch pipeline and marker convention. Both mock and
    real branches return the input value identically — the branch exists solely
    to satisfy the dual-mode parity marker convention (§10.6) without implying
    that the real path has a different implementation.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Single input slot named "value" with type "ANY".
        OUTPUT_SLOTS: Single output slot named "value" with type "ANY".
    """
    NODE_TYPE = "PassThrough"
    CATEGORY = "Debug"
    DISPLAY_NAME = "Pass Through"
    DESCRIPTION = (
        "A trivial no-op node that passes its input value through unchanged. "
        "Used to verify the dispatch pipeline and marker convention."
    )
    INPUT_SLOTS = [SlotSpec("value", "ANY")]
    OUTPUT_SLOTS = [SlotSpec("value", "ANY")]

    # REAL_PATH_VERIFIED: worker/tests/test_passthrough.py::test_execute_real_returns_input
    # MOCK_PATH_VERIFIED: worker/tests/test_passthrough.py::test_execute_mock_returns_input
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the pass-through node.

        Both mock and real branches return the input value unchanged.
        The branch exists solely to satisfy the dual-mode parity marker
        convention (§10.6) — this node has no meaningfully different
        behavior between modes.

        Args:
            ctx: Runtime context (unused by this node; present for
                consistency with the execute() signature).
            **inputs: Must contain a "value" key. The value is returned
                as the sole output under the same key.

        Returns:
            Dict with key "value" containing the same value that was
            passed in via inputs["value"].
        """
        if ctx.mock:
            # Mock branch: return input unchanged (no torch, no side effects).
            return {"value": inputs["value"]}
        else:
            # Real branch: same passthrough logic — no torch dependency exists
            # in this node, so the real path is identical to mock.
            return {"value": inputs["value"]}
