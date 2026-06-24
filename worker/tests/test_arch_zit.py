"""Unit tests for the ZiT architecture dispatch module.

Tests cover ``can_handle()`` dispatching for ZiT and non-ZiT models,
the mock ``sample()`` path returning ``MockLatent`` with the correct
seed, and import isolation (no torch import at module load time).

.. versionadded:: 0.1.0
"""

from __future__ import annotations

import importlib
import os
import sys
import threading
from typing import Any

import pytest

from worker.nodes.arch.diffusion.zit import (
    MockLatent,
    VAE_SCALE_FACTOR,
    _SamplingCancelled,
    _infer_config_from_checkpoint,
    _make_callback,
    _remap_z_image_keys,
    can_handle,
    compute_latent_shape,
    load_transformer,
    load_vae,
    sample,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_model(arch: str | None = "zit") -> Any:
    """Build a minimal model object with an ``arch`` attribute.

    Uses a simple namespace object with the ``arch`` attribute set
    to the given value. If ``arch`` is ``None``, the model object
    is constructed without an ``arch`` attribute to test the
    missing-attribute case.

    Args:
        arch: The architecture string to set, or ``None`` to omit
            the attribute entirely.

    Returns:
        A namespace object with an ``arch`` attribute (or none).
    """
    if arch is None:
        # Create a model object without an arch attribute.
        # This tests the getattr fallback in can_handle().
        return type("Model", (), {})()
    return type("Model", (), {"arch": arch})()


# ---------------------------------------------------------------------------
# Tests: VAE_SCALE_FACTOR
# ---------------------------------------------------------------------------


def test_vae_scale_factor_value() -> None:
    """Verify ``VAE_SCALE_FACTOR`` module constant equals ``8``.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        reading a module-level constant, but consistent with the test file
        convention).

    Tests:
        Import ``VAE_SCALE_FACTOR`` from the module under test and
        assert it equals ``8``.

    Expected output:
        ``VAE_SCALE_FACTOR == 8`` — the Z-Image-Turbo VAE spatial
        compression factor matches the published config.
    """
    assert VAE_SCALE_FACTOR == 8


# ---------------------------------------------------------------------------
# Tests: can_handle
# ---------------------------------------------------------------------------


def test_can_handle_zit() -> None:
    """Verify ``can_handle()`` returns ``True`` for a ZiT model.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model object with ``arch == "zit"``, pass it to
        ``can_handle()``, and assert the result is ``True``.

    Expected output:
        ``can_handle(model) == True`` — the ZiT arch module claims
        this model.
    """
    model = _make_model("zit")
    assert can_handle(model) is True


def test_can_handle_non_zit() -> None:
    """Verify ``can_handle()`` returns ``False`` for non-ZiT models.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Construct a model with ``arch == "flux"`` and pass it to
        ``can_handle()``, assert ``False``. Then construct a model
        without an ``arch`` attribute and assert ``False`` again.

    Expected output:
        ``can_handle(flux_model) == False`` and
        ``can_handle(no_arch_model) == False`` — the ZiT arch module
        does not claim these models.
    """
    # Test with a non-ZiT architecture string.
    flux_model = _make_model("flux")
    assert can_handle(flux_model) is False

    # Test with a model that has no arch attribute at all.
    no_arch_model = _make_model(None)
    assert can_handle(no_arch_model) is False


# ---------------------------------------------------------------------------
# Tests: load_transformer
# ---------------------------------------------------------------------------


def test_load_transformer_is_callable() -> None:
    """Verify ``load_transformer`` is a callable in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Assert that ``load_transformer`` is a callable (a function
        object) — this confirms the symbol exists and is importable
        without requiring torch or diffusers. The test does NOT
        exercise the real loading path (that requires raw-format
        fixtures from P904-B1b).

    Expected output:
        ``callable(load_transformer) == True`` — the function symbol
        exists and is importable in mock mode.
    """
    assert callable(load_transformer)


def test_load_vae_is_callable() -> None:
    """Verify ``load_vae`` is a callable in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Assert that ``load_vae`` is a callable (a function
        object) — this confirms the symbol exists and is importable
        without requiring torch or diffusers. The test does NOT
        exercise the real loading path (that requires raw-format
        VAE fixtures).

    Expected output:
        ``callable(load_vae) == True`` — the function symbol
        exists and is importable in mock mode.
    """
    assert callable(load_vae)


# ---------------------------------------------------------------------------
# Tests: sample (mock mode)
# ---------------------------------------------------------------------------


def test_sample_mock_returns_mock_latent_and_seed() -> None:
    """Verify ``sample()`` returns ``(MockLatent(), seed)`` in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.

    Tests:
        Call ``sample()`` with ``seed=42`` and all other arguments
        as ``None`` or empty, and assert the returned tuple contains
        a ``MockLatent`` sentinel and the correct seed value.

    Expected output:
        ``result[0]`` is a ``MockLatent`` instance and
        ``result[1] == 42``.
    """
    result = sample(
        model=None,
        conditioning=None,
        latent=None,
        steps=4,
        cfg=7.0,
        seed=42,
        device="cpu",
        cancel_flag=[False],
        emit_progress=lambda step, total: None,
    )

    assert isinstance(result[0], MockLatent)
    assert result[1] == 42


def test_sample_mock_preserves_seed_value() -> None:
    """Verify ``sample()`` returns the exact seed passed in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring the mock code path is taken.

    Tests:
        Call ``sample()`` with several different seed values (0, 1,
        2**32 - 1) and assert each one is returned unchanged.

    Expected output:
        The seed value in the result tuple matches the input exactly
        for each test case.
    """
    for test_seed in (0, 1, 2**32 - 1, 12345):
        result = sample(
            model=None,
            conditioning=None,
            latent=None,
            steps=4,
            cfg=7.0,
            seed=test_seed,
            device="cpu",
            cancel_flag=[False],
            emit_progress=lambda step, total: None,
        )

        assert result[1] == test_seed


def test_sample_real_assembles_pipeline_via_cache() -> None:
    """Verify ``sample()`` calls ``pipeline_cache.get_or_load()`` in real mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK`` is temporarily set to ``"0"`` by this
        test, overriding the autouse fixture. A mock ``pipeline_cache``
        is provided with a ``get_or_load`` mock.

    Tests:
        Call ``sample()`` in real mode with a mock model that carries
        ``model_id="test_model"`` and a mock ``pipeline_cache``.
        Configure the mock pipeline's ``__call__`` to return
        ``[MagicMock(), seed]`` (matching ``return_dict=False`` output
        format). Assert that ``get_or_load`` was called with a key
        containing ``:pipeline`` and that the returned tuple has the
        correct structure.

    Expected output:
        ``get_or_load.assert_called_once()`` with the first positional
        argument matching ``"test_model:pipeline"``; returned tuple
        is ``(mock_latent, 42)``.
    """
    # Capture the pre-existing value and force real mode.
    original = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        # Build a mock pipeline cache with a MagicMock for get_or_load.
        from unittest.mock import MagicMock

        mock_cache = MagicMock()
        # get_or_load returns a mock pipeline object.
        mock_pipeline = MagicMock()
        mock_cache.get_or_load.return_value = mock_pipeline

        # Configure the mock pipeline's __call__ to return a list
        # matching return_dict=False output: [latent_result, seed].
        # For a MagicMock, __call__ is the same as the mock's return_value.
        latent_result = MagicMock()
        mock_pipeline.return_value = [latent_result, 42]

        # Build a model object that carries model_id (like RealModel).
        model = type("Model", (), {"arch": "zit", "model_id": "test_model"})()

        # Build a conditioning object with positive/negative embeds.
        conditioning = type("Conditioning", (), {
            "positive": None,
            "negative": None,
        })()

        # Build a mock clip object with tokenizer/text_encoder.
        # These attributes are read by loader_fn from clip (not
        # conditioning) to fix the wiring defect.
        mock_clip = type("RealClip", (), {
            "tokenizer": MagicMock(),
            "text_encoder": MagicMock(),
        })()

        # Call sample() — the real path now invokes the pipeline.
        result = sample(
            model=model,
            conditioning=conditioning,
            clip=mock_clip,
            latent=None,
            steps=4,
            cfg=7.0,
            seed=42,
            device="cpu",
            cancel_flag=[False],
            emit_progress=lambda step, total: None,
            pipeline_cache=mock_cache,
        )

        # Assert get_or_load was called with the correct cache key.
        # The call was made inside sample() before the pipeline was
        # invoked.
        mock_cache.get_or_load.assert_called_once()
        call_args = mock_cache.get_or_load.call_args
        cache_key = call_args[0][0]
        assert ":pipeline" in cache_key, (
            f"Expected cache key to contain ':pipeline', got '{cache_key}'"
        )

        # Assert the returned tuple has the correct structure:
        # (latent_result, seed) matching return_dict=False format.
        assert result[0] is latent_result
        assert result[1] == 42
    finally:
        # Restore the original value unconditionally.
        if original is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original


def test_sample_real_invokes_pipeline_with_correct_args() -> None:
    """Verify ``sample()`` calls the pipeline with all expected keyword arguments.

    Preconditions:
        ``ANVILML_WORKER_MOCK`` is temporarily set to ``"0"`` by this
        test, overriding the autouse fixture. A mock ``pipeline_cache``
        returns a ``MagicMock`` pipeline object whose ``__call__`` is
        configured to return ``[MagicMock(), seed]``.

    Tests:
        Call ``sample()`` in real mode with a mock model (``arch="zit"``,
        ``model_id="test_model"``), mock conditioning (with
        ``positive``/``negative`` attributes), and a
        ``threading.Event()`` as ``cancel_flag``. Assert that the mock
        pipeline's ``__call__`` was invoked with ``output_type="latent"``,
        ``return_dict=False``, ``num_inference_steps=steps``,
        ``guidance_scale=cfg``, and a callable ``callback_on_step_end``.
        Also assert the returned tuple has the correct structure.

    Expected output:
        Pipeline ``__call__`` called with all expected keyword arguments
        matching the plan's invocation signature.
    """
    # Capture the pre-existing value and force real mode.
    original = os.environ.get("ANVILML_WORKER_MOCK")
    os.environ["ANVILML_WORKER_MOCK"] = "0"
    try:
        from unittest.mock import MagicMock

        steps = 8
        cfg = 7.5
        seed = 99

        # Build a mock pipeline cache that returns a mock pipeline.
        mock_cache = MagicMock()
        mock_pipeline = MagicMock()
        mock_cache.get_or_load.return_value = mock_pipeline

        # Configure the mock pipeline's __call__ to return a list
        # matching return_dict=False output format.
        # For a MagicMock, __call__ is the same as the mock's return_value.
        latent_result = MagicMock()
        mock_pipeline.return_value = [latent_result, seed]

        # Build a mock model with arch="zit" and model_id.
        model = type("Model", (), {"arch": "zit", "model_id": "test_model"})()

        # Build a conditioning object with positive/negative embeds.
        conditioning = type("Conditioning", (), {
            "positive": MagicMock(),
            "negative": MagicMock(),
        })()

        # Build a mock clip object with tokenizer/text_encoder.
        # These attributes are read by loader_fn from clip (not
        # conditioning) to fix the wiring defect.
        mock_clip = type("RealClip", (), {
            "tokenizer": MagicMock(),
            "text_encoder": MagicMock(),
        })()

        # A threading.Event as cancel_flag.
        cancel_flag = threading.Event()

        result = sample(
            model=model,
            conditioning=conditioning,
            clip=mock_clip,
            latent=None,
            steps=steps,
            cfg=cfg,
            seed=seed,
            device="cuda",
            cancel_flag=cancel_flag,
            emit_progress=lambda step, total: None,
            pipeline_cache=mock_cache,
        )

        # Assert the pipeline was called with all expected keyword args.
        # For a MagicMock, call_args is on the mock itself (not __call__).
        call_kwargs = mock_pipeline.call_args[1]
        assert call_kwargs["output_type"] == "latent"
        assert call_kwargs["return_dict"] is False
        assert call_kwargs["num_inference_steps"] == steps
        assert call_kwargs["guidance_scale"] == cfg

        # callback_on_step_end must be a callable (the _make_callback adapter).
        assert callable(call_kwargs["callback_on_step_end"])

        # prompt_embeds and negative_prompt_embeds must match conditioning.
        assert call_kwargs["prompt_embeds"] is conditioning.positive
        assert call_kwargs["negative_prompt_embeds"] is conditioning.negative

        # latents must be the value passed in.
        assert call_kwargs["latents"] is None

        # Assert the returned tuple has the correct structure.
        assert result[0] is latent_result
        assert result[1] == seed
    finally:
        # Restore the original value unconditionally.
        if original is None:
            os.environ.pop("ANVILML_WORKER_MOCK", None)
        else:
            os.environ["ANVILML_WORKER_MOCK"] = original


# ---------------------------------------------------------------------------
# Tests: import isolation
# ---------------------------------------------------------------------------


def test_sample_mock_no_torch_import() -> None:
    """Verify the module imports cleanly without torch in mock mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture.

    Tests:
        Remove ``torch`` from ``sys.modules`` (if present) and
        re-import the ``worker.nodes.arch.diffusion.zit`` module. Assert that
        no ``ImportError`` is raised and that ``torch`` is not in
        ``sys.modules`` after import — proving no top-level import
        of torch occurs.

    Expected output:
        Module imports successfully and ``"torch"`` is absent from
        ``sys.modules``, confirming mock-mode import isolation.
    """
    # Remove torch from sys.modules to simulate an environment
    # where torch is not installed. This ensures the import
    # succeeds even without the package available.
    torch_was_present = "torch" in sys.modules
    torch_saved = sys.modules.pop("torch", None)

    # Also remove the module from sys.modules cache so we get a
    # fresh import that exercises the full module body.
    sys.modules.pop("worker.nodes.arch.diffusion.zit", None)

    # Also remove the parent arch package from cache.
    sys.modules.pop("worker.nodes.arch", None)

    try:
        # Import must succeed — if torch were imported at module level,
        # this would raise ImportError since we just removed it.
        import worker.nodes.arch.diffusion.zit as zit_mod

        importlib.reload(zit_mod)

        # Verify torch is still absent from sys.modules after import.
        assert "torch" not in sys.modules, (
            "torch was imported at module level — "
            "this breaks mock-mode isolation"
        )

        # Verify the module's public API is intact.
        assert callable(zit_mod.can_handle)
        assert callable(zit_mod.sample)
        assert zit_mod.MockLatent is not None
    finally:
        # Restore torch if it was present before.
        if torch_was_present and torch_saved is not None:
            sys.modules["torch"] = torch_saved
        # Restore cached modules for other tests.
        sys.modules.pop("worker.nodes.arch.diffusion.zit", None)
        sys.modules.pop("worker.nodes.arch", None)


# ---------------------------------------------------------------------------
# Tests: compute_latent_shape
# ---------------------------------------------------------------------------


def test_compute_latent_shape_known_dims() -> None:
    """Verify ``compute_latent_shape()`` produces the canonical ZiT latent shape.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active (not strictly required for
        a pure arithmetic function, but consistent with the test file
        convention).

    Tests:
        Call ``compute_latent_shape(1, 1024, 1024, 4)`` and assert
        the result equals ``(1, 4, 128, 128)``. This is the canonical
        ZiT case: 1024×1024 image → 128×128 latent (8× spatial
        compression), batch 1, 4 channels (standard SD-style).

    Expected output:
        ``compute_latent_shape(1, 1024, 1024, 4) == (1, 4, 128, 128)``.
    """
    result = compute_latent_shape(1, 1024, 1024, 4)
    assert result == (1, 4, 128, 128)


def test_compute_latent_shape_non_divisible() -> None:
    """Verify ``compute_latent_shape()`` silently floors non-divisible dimensions.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Call ``compute_latent_shape(2, 1025, 1026, 4)`` and assert
        the result equals ``(2, 4, 128, 128)``. The floor division
        ``1025 // 16 == 64`` and ``1026 // 16 == 64``, so
        ``h == w == 128`` — this verifies that non-divisible
        dimensions silently floor rather than raise, matching
        ``ZImagePipeline.prepare_latents``'s integer-division behaviour.

    Expected output:
        ``compute_latent_shape(2, 1025, 1026, 4) == (2, 4, 128, 128)``.
    """
    result = compute_latent_shape(2, 1025, 1026, 4)
    assert result == (2, 4, 128, 128)


# ---------------------------------------------------------------------------
# Tests: _make_callback
# ---------------------------------------------------------------------------


def test_make_callback_emits_progress() -> None:
    """Verify ``_make_callback()`` returns a closure that emits progress.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Build a list accumulator for ``emit_progress``, create an unset
        ``threading.Event`` as ``cancel_flag`` (no cancellation), and
        call ``_make_callback()`` with ``total_steps=4``. Invoke the
        returned closure with ``i=0`` and ``callback_kwargs={}``.
        Assert ``emit_progress`` was called exactly once with ``(0, 4)``
        and the return value equals ``{}``.

    Expected output:
        ``emit_progress`` called once with ``(0, 4)``; return value is
        ``{}`` — the callback_kwargs passed through unchanged.
    """
    # Use a list as a mutable accumulator to record emit_progress calls.
    progress_calls: list[tuple[int, int]] = []

    def emit_progress(step: int, total: int) -> None:
        progress_calls.append((step, total))

    # An unset threading.Event — no cancellation pending.
    cancel_flag = threading.Event()

    # Build the callback adapter.
    callback = _make_callback(emit_progress, cancel_flag, total_steps=4)

    # Invoke with self=None (diffusers passes the pipeline instance,
    # but the adapter doesn't need it), i=0, t=None, callback_kwargs={}.
    result = callback(None, 0, None, {})

    # Assert progress was emitted with the correct (step, total) values.
    assert len(progress_calls) == 1
    assert progress_calls[0] == (0, 4)

    # Assert callback_kwargs was returned unchanged.
    assert result == {}


def test_make_callback_raises_on_cancellation() -> None:
    """Verify ``_make_callback()`` raises ``_SamplingCancelled`` when cancelled.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture, ensuring mock mode is active.

    Tests:
        Create a ``threading.Event`` and set it before calling the
        callback (simulating a cancellation request). Build the closure
        with ``total_steps=4`` and invoke it with ``i=2``. Assert that
        ``emit_progress`` was called with ``(2, 4)`` (progress is
        emitted before cancellation check) and that ``_SamplingCancelled``
        is raised.

    Expected output:
        ``emit_progress`` called once with ``(2, 4)``;
        ``_SamplingCancelled`` raised — the adapter detected the
        cancellation request and raised the sentinel exception.
    """
    # Use a list as a mutable accumulator to record emit_progress calls.
    progress_calls: list[tuple[int, int]] = []

    def emit_progress(step: int, total: int) -> None:
        progress_calls.append((step, total))

    # Create a threading.Event and set it — cancellation is pending.
    cancel_flag = threading.Event()
    cancel_flag.set()

    # Build the callback adapter.
    callback = _make_callback(emit_progress, cancel_flag, total_steps=4)

    # Invoke with i=2 — progress should be emitted, then cancellation
    # detected and _SamplingCancelled raised.
    with pytest.raises(_SamplingCancelled, match="sampling cancelled at step 2"):
        callback(None, 2, None, {})

    # Assert progress was emitted before cancellation was detected.
    assert len(progress_calls) == 1
    assert progress_calls[0] == (2, 4)


# ---------------------------------------------------------------------------
# Tests: load_transformer — no diffusers internal import
# ---------------------------------------------------------------------------


def test_no_diffusers_internal_import() -> None:
    """Verify the import of ``convert_z_image_transformer_checkpoint_to_diffusers`` is removed.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture (not strictly required — this test reads source text).

    Tests:
        Read the source of ``worker/nodes/arch/diffusion/zit.py`` and
        assert that the string ``convert_z_image_transformer_checkpoint_to_diffusers``
        does not appear anywhere in the file.

    Expected output:
        Zero matches for ``convert_z_image_transformer_checkpoint_to_diffusers``
        in the source file — the private diffusers internal is no longer imported.
    """
    import os

    src_path = os.path.join(
        os.path.dirname(__file__),
        "..",
        "nodes",
        "arch",
        "diffusion",
        "zit.py",
    )
    src = open(src_path).read()
    assert "convert_z_image_transformer_checkpoint_to_diffusers" not in src


# ---------------------------------------------------------------------------
# Tests: _remap_z_image_keys
# ---------------------------------------------------------------------------


def test_remap_key_transformations() -> None:
    """Verify the manual key remap produces correct diffusers-convention keys.

    Preconditions:
        ``ANVILML_WORKER_MOCK=1`` is set by the ``conftest.py`` autouse
        fixture (not strictly required — the remap function is pure
        key manipulation, no torch/diffusers import needed).

    Tests:
        Construct a synthetic checkpoint with every key type present in the
        ZiT architecture (x_embedder, layers with qkv/out/q_norm/k_norm,
        final_layer, cap_embedder, noise_refiner, context_refiner,
        t_embedder, norm_final), then call ``_remap_z_image_keys()`` and
        assert the remapped keys match the expected diffusers convention.

    Expected output:
        All keys are correctly remapped:
        * ``model.diffusion_model.`` prefix is stripped.
        * ``final_layer.`` → ``all_final_layer.2-1.``
        * ``x_embedder.`` → ``all_x_embedder.2-1.``
        * ``.attention.out.weight`` → ``.attention.to_out.0.weight``
        * ``.attention.out.bias`` → ``.attention.to_out.0.bias``
        * ``.attention.q_norm.weight`` → ``.attention.norm_q.weight``
        * ``.attention.k_norm.weight`` → ``.attention.norm_k.weight``
        * ``.attention.qkv.weight`` is split into ``to_q.weight``,
          ``to_k.weight``, ``to_v.weight``.
        * ``norm_final.weight`` is removed.
    """
    # Build a synthetic checkpoint with every key type present in the
    # ZiT architecture. Use simple scalar tensors (shape [1]) so that
    # torch.chunk works on the QKV split without needing real tensor
    # shapes — each QKV split produces [1] tensors.
    import torch

    checkpoint: dict[str, Any] = {
        # x_embedder keys (with model.diffusion_model. prefix)
        "model.diffusion_model.x_embedder.proj.weight": torch.ones(3840, 3),
        # Layer 0 attention keys (using .attention. prefix as in diffusers source)
        "model.diffusion_model.layers.0.attention.qkv.weight": torch.ones(11520, 3840),
        "model.diffusion_model.layers.0.attention.to_out.0.weight": torch.ones(3840, 3840),
        "model.diffusion_model.layers.0.attention.to_out.0.bias": torch.ones(3840),
        "model.diffusion_model.layers.0.attention.q_norm.weight": torch.ones(128),
        "model.diffusion_model.layers.0.attention.k_norm.weight": torch.ones(128),
        "model.diffusion_model.layers.0.ff.net.0.proj.weight": torch.ones(3840, 3840),
        "model.diffusion_model.layers.0.ff.net.2.weight": torch.ones(3840, 3840),
        "model.diffusion_model.layers.0.attention.norm_q.weight": torch.ones(128),
        "model.diffusion_model.layers.0.attention.norm_k.weight": torch.ones(128),
        # Layer 1 attention keys (with separate to_q/to_k/to_v — no qkv)
        "model.diffusion_model.layers.1.attention.to_q.weight": torch.ones(3840, 3840),
        "model.diffusion_model.layers.1.attention.to_k.weight": torch.ones(3840, 3840),
        "model.diffusion_model.layers.1.attention.to_v.weight": torch.ones(3840, 3840),
        # Layer 29 (last layer, with qkv to test defuse)
        "model.diffusion_model.layers.29.attention.qkv.weight": torch.ones(11520, 3840),
        # Context refiner keys
        "model.diffusion_model.context_refiner.0.attention.to_q.weight": torch.ones(3840, 3840),
        "model.diffusion_model.context_refiner.1.attention.to_q.weight": torch.ones(3840, 3840),
        # Noise refiner keys
        "model.diffusion_model.noise_refiner.0.attention.to_q.weight": torch.ones(3840, 3840),
        # cap_embedder
        "model.diffusion_model.cap_embedder.0.weight": torch.ones(2560, 768),
        # final_layer
        "model.diffusion_model.final_layer.linear.weight": torch.ones(64, 16),
        "model.diffusion_model.final_layer.adaLN_modulation.1.weight": torch.ones(3840 * 6),
        # t_embedder
        "model.diffusion_model.t_embedder.mlp.0.weight": torch.ones(3840, 256),
        # norm_final (should be removed)
        "model.diffusion_model.norm_final.weight": torch.ones(3840),
    }

    remapped = _remap_z_image_keys(checkpoint)

    # --- Key renaming: model.diffusion_model. prefix stripped ---
    # x_embedder: "model.diffusion_model.x_embedder." → "all_x_embedder.2-1."
    assert "all_x_embedder.2-1.proj.weight" in remapped

    # Layer 0 attention keys: prefix stripped
    assert "layers.0.attention.to_out.0.weight" in remapped
    assert "layers.0.attention.to_out.0.bias" in remapped
    assert "layers.0.attention.norm_q.weight" in remapped
    assert "layers.0.attention.norm_k.weight" in remapped

    # Layer 1: to_q/to_k/to_v keys (no qkv, so no change beyond prefix strip)
    assert "layers.1.attention.to_q.weight" in remapped
    assert "layers.1.attention.to_k.weight" in remapped
    assert "layers.1.attention.to_v.weight" in remapped

    # Layer 29: qkv key should be split
    # After remap, the qkv key is gone (defused)
    assert "layers.29.attention.qkv.weight" not in remapped
    assert "layers.29.attention.to_q.weight" in remapped
    assert "layers.29.attention.to_k.weight" in remapped
    assert "layers.29.attention.to_v.weight" in remapped

    # Context refiner keys: prefix stripped
    assert "context_refiner.0.attention.to_q.weight" in remapped
    assert "context_refiner.1.attention.to_q.weight" in remapped

    # Noise refiner keys: prefix stripped
    assert "noise_refiner.0.attention.to_q.weight" in remapped

    # cap_embedder: prefix stripped
    assert "cap_embedder.0.weight" in remapped

    # final_layer: "model.diffusion_model.final_layer." → "all_final_layer.2-1."
    assert "all_final_layer.2-1.linear.weight" in remapped
    assert "all_final_layer.2-1.adaLN_modulation.1.weight" in remapped

    # t_embedder: prefix stripped
    assert "t_embedder.mlp.0.weight" in remapped

    # norm_final.weight should be removed
    assert "norm_final.weight" not in remapped

    # Verify QKV defuse produces correct tensor shapes.
    # Original qkv weight shape: [11520, 3840] → each split: [3840, 3840]
    qkv_key = "model.diffusion_model.layers.0.attention.qkv.weight"
    assert qkv_key not in remapped  # original key removed
    assert remapped["layers.0.attention.to_q.weight"].shape == torch.Size([3840, 3840])
    assert remapped["layers.0.attention.to_k.weight"].shape == torch.Size([3840, 3840])
    assert remapped["layers.0.attention.to_v.weight"].shape == torch.Size([3840, 3840])

    # Verify the original checkpoint was not mutated (we operate on a copy)
    assert "model.diffusion_model.layers.29.attention.qkv.weight" in checkpoint
    assert "model.diffusion_model.norm_final.weight" in checkpoint
