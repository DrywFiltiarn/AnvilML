"""LoadModel node — loads a diffusion model from a safetensors file.

This node is the entry point for model loading in the AnvilML worker.
The mock branch returns a sentinel dict; the real branch dispatches
to the registered diffusion architecture module (currently "zit")
via ``arch.diffusion.get_module()`` and ``pipeline_cache.get_or_load()``.
"""

from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register

import logging

logger = logging.getLogger(__name__)


@register
class LoadModel(BaseNode):
    """Load a diffusion model from a safetensors file.

    This is the first node in the model-loading pipeline. In mock mode
    it returns a sentinel dict; in real mode it dispatches to the
    registered diffusion architecture module (currently "zit") via
    ``arch.diffusion.get_module()`` and caches the result via
    ``pipeline_cache.get_or_load()``.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Single input slot named "model_id" with type "STRING".
        OUTPUT_SLOTS: Single output slot named "model" with type "MODEL".
    """
    NODE_TYPE = "LoadModel"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load Model"
    DESCRIPTION = "Loads a diffusion model from a safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadModel node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        dispatches to the registered diffusion architecture module
        ("zit") via ``arch.diffusion.get_module()`` and caches the
        loaded model via ``pipeline_cache.get_or_load()``.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the model to load.

        Returns:
            In mock mode: Dict with key "model" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Dict with key "model" containing a
            ``torch.nn.Module`` (the loaded ZiTModel).

        Raises:
            RuntimeError: If no diffusion arch module is registered
                for key "zit" (should not occur after P20-B2).
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"model": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: dispatch to the registered diffusion arch
            # module. The arch key "zit" matches zit.py's can_handle()
            # contract. get_or_load provides caching: if the same
            # model_id is loaded again, the cached ZiTModel is returned
            # without re-loading. The cache is not modified on exception
            # per the PipelineCache contract.
            from worker.nodes.arch.diffusion import get_module

            module = get_module("zit")
            if module is None:
                # Defensive guard — zit is imported and appended to
                # _REGISTERED_MODULES in diffusion/__init__.py (P20-B2),
                # so this should never trigger in normal operation.
                raise RuntimeError(
                    f"no diffusion arch module registered for 'zit'; "
                    f"cannot load model '{inputs['model_id']}'"
                )

            logger.debug("LoadModel: requesting model_id=%s", inputs["model_id"])
            return {
                "model": ctx.pipeline_cache.get_or_load(
                    inputs["model_id"],
                    lambda: module.load(inputs["model_id"], ctx.caps),
                )
            }


@register
class LoadVae(BaseNode):
    """Load a VAE from a standalone safetensors file.

    This node loads a Variational Autoencoder component. In mock mode
    it returns a sentinel dict; in real mode it dispatches to the
    registered VAE architecture module (currently "zit_vae") via
    ``arch.vae.get_module()`` and caches the result via
    ``pipeline_cache.get_or_load()``.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: Single input slot named "model_id" with type "STRING".
        OUTPUT_SLOTS: Single output slot named "vae" with type "VAE".
    """
    NODE_TYPE = "LoadVae"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load VAE"
    DESCRIPTION = "Loads a VAE from a standalone safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
    OUTPUT_SLOTS = [SlotSpec("vae", "VAE")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_real_loads_zit_vae_fixture
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadVae node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        dispatches to the registered VAE architecture module ("zit_vae")
        via ``arch.vae.get_module()`` and caches the loaded VAE via
        ``pipeline_cache.get_or_load()``.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the VAE to load.

        Returns:
            In mock mode: Dict with key "vae" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Dict with key "vae" containing a
            ``torch.nn.Module`` (the loaded ZiTVaeModel).

        Raises:
            RuntimeError: If no VAE arch module is registered for
                key "zit_vae" (should not occur after P23-B2).
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"vae": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: dispatch to the registered VAE arch module.
            # The arch key "zit_vae" matches zit_vae.py's can_handle()
            # contract. get_or_load provides caching: if the same
            # model_id is loaded again, the cached ZiTVaeModel is returned
            # without re-loading. The cache is not modified on exception
            # per the PipelineCache contract.
            from worker.nodes.arch.vae import get_module

            module = get_module("zit_vae")
            if module is None:
                # Defensive guard — zit_vae is imported and appended to
                # _REGISTERED_MODULES in vae/__init__.py (P23-B2),
                # so this should never trigger in normal operation.
                raise RuntimeError(
                    f"no VAE arch module registered for 'zit_vae'; "
                    f"cannot load VAE '{inputs['model_id']}'"
                )

            logger.debug("LoadVae: requesting model_id=%s", inputs["model_id"])
            return {
                "vae": ctx.pipeline_cache.get_or_load(
                    f"vae:{inputs['model_id']}",
                    lambda: module.load(inputs["model_id"], ctx.caps),
                )
            }


@register
class LoadClip(BaseNode):
    """Load a CLIP text encoder from a safetensors file.

    This node loads a text encoder component for prompt conditioning.
    In mock mode it returns a sentinel dict; in real mode it dispatches
    to the registered CLIP architecture module (currently "qwen3") via
    ``arch.clip.get_module()`` and caches the result via
    ``pipeline_cache.get_or_load()``.

    Class Attributes:
        NODE_TYPE: The registry key for this node type.
        CATEGORY: The category this node belongs to.
        DISPLAY_NAME: Human-readable name shown in UI/tooling.
        DESCRIPTION: One-line description of the node's purpose.
        INPUT_SLOTS: "model_id" (STRING, required) and "clip_type"
            (STRING, optional). clip_type is a dispatch hint (e.g.
            "qwen3") for architecture-specific loading.
        OUTPUT_SLOTS: Single output slot named "clip" with type "CLIP".
    """
    NODE_TYPE = "LoadClip"
    CATEGORY = "Loaders"
    DISPLAY_NAME = "Load CLIP"
    DESCRIPTION = "Loads a CLIP text encoder from a safetensors file."
    INPUT_SLOTS = [SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]
    OUTPUT_SLOTS = [SlotSpec("clip", "CLIP")]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_real_loads_qwen3_fixture
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Execute the LoadClip node.

        Branches on ctx.mock at the top per §14.6 — the mock branch
        returns a sentinel dict with no real loading; the real branch
        dispatches to the registered CLIP architecture module (currently
        "qwen3") via ``arch.clip.get_module()`` and caches the loaded
        encoder via ``pipeline_cache.get_or_load()``.

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain a "model_id" key with the string
                identifier of the CLIP encoder to load. An optional
                "clip_type" key (e.g. "qwen3") may be provided as a
                dispatch hint for architecture-specific loading.

        Returns:
            In mock mode: Dict with key "clip" containing a sentinel
            dict {"mock": True, "model_id": <model_id>}.
            In real mode: Dict with key "clip" containing a
            ``torch.nn.Module`` (the loaded Qwen3TextEncoder).

        Raises:
            RuntimeError: If no CLIP arch module is registered for the
                requested clip_type (should not occur after qwen3 is
                registered in arch.clip.__init__).
        """
        if ctx.mock:
            # Mock branch: return a sentinel dict with no real loading,
            # no torch import, and no file I/O. The sentinel carries the
            # model_id so downstream tests can verify the correct value
            # was propagated through the node system.
            return {"clip": {"mock": True, "model_id": inputs["model_id"]}}
        else:
            # Real branch: dispatch to the registered CLIP arch module.
            # The clip_type key defaults to "qwen3" which matches the
            # dispatcher's registered module key in arch.clip.__init__.
            # get_or_load provides caching: if the same model_id is
            # loaded again, the cached Qwen3TextEncoder is returned
            # without re-loading. The cache is not modified on exception
            # per the PipelineCache contract.
            from worker.nodes.arch.clip import get_module

            module = get_module(inputs.get("clip_type", "qwen3"))
            if module is None:
                # Defensive guard — qwen3 is imported and appended to
                # _REGISTERED_MODULES in arch.clip.__init__ (P22-B3),
                # so this should never trigger in normal operation.
                clip_type = inputs.get("clip_type", "qwen3")
                raise RuntimeError(
                    f"no clip arch module registered for '{clip_type}'; "
                    f"cannot load clip '{inputs['model_id']}'"
                )

            logger.debug(
                "LoadClip: requesting model_id=%s, clip_type=%s",
                inputs["model_id"],
                inputs.get("clip_type", "qwen3"),
            )
            return {
                "clip": ctx.pipeline_cache.get_or_load(
                    f"clip:{inputs['model_id']}",
                    lambda: module.load(
                        inputs["model_id"], ctx.caps
                    ),
                )
            }
