# Plan Report: P18-D5

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P18-D5                                                      |
| Phase       | 018 — ZiT Generic Nodes                                     |
| Description | worker/nodes/loader.py: LoadVae real safetensors loading path |
| Depends on  | P18-D4                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-22T12:00:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Replace the `NotImplementedError` stub in `LoadVae.execute()`'s real path with actual
safetensors-based VAE loading using `diffusers.AutoencoderKL`, wired through
`ctx.pipeline_cache.get_or_load()`. The returned VAE object must be a real
`AutoencoderKL` instance that matches the component type `ZImagePipeline.__init__`
expects for its `vae=` argument. Existing mock-mode tests in
`worker/tests/test_nodes_loader.py` must continue to pass unchanged.

## Scope

### In Scope
- Modify `worker/nodes/loader.py`: replace `LoadVae.execute()`'s real-mode stub with
  a working safetensors loading path using `AutoencoderKL.from_pretrained()` (or the
  confirmed alternative at ACT time).
- The loader closure constructs an `AutoencoderKL` instance and returns it directly.
- Cache the result via `ctx.pipeline_cache.get_or_load(model_id, "bf16", loader_fn)`.
- Add an inline comment explaining the construction choice (`from_pretrained` vs
  `from_single_file`).
- No new source files, no new test files, no new public API surface.

### Out of Scope
- LoadModel real path (P18-D4, already implemented).
- LoadClip real path (P18-D6).
- VAE wrapper type — the real `AutoencoderKL` instance is returned directly, no
  `RealVae` wrapper is created.
- Any changes to `pipeline_cache.py` — it is already correct and tested.
- Any changes to `base.py` or `conftest.py`.

## Existing Codebase Assessment

The `LoadVae` class exists in `worker/nodes/loader.py` alongside `LoadModel` and
`LoadClip`. `LoadModel`'s real path has already been implemented (P18-D4) and follows
a clear pattern: lazy imports of `safetensors`/`diffusers`/`torch`, construction of a
`loader_fn` closure, and caching via `ctx.pipeline_cache.get_or_load()`. The `LoadVae`
stub mirrors this structure but raises `NotImplementedError`.

`pipeline_cache.py` implements a working LRU cache with `get_or_load(model_id, dtype,
loader_fn)` that handles OOM eviction. It is imported and used by the existing
`LoadModel` real path.

`NodeContext.pipeline_cache` is typed as `dict[str, Any]` in `base.py` but at runtime
(after P903-A2) is a `PipelineCache` instance with `get_or_load()` available.

The test file `worker/tests/test_nodes_loader.py` has three tests for `LoadVae`:
registry registration, mock execution, and metadata attribute verification. None of
these tests exercise the real path (mock mode short-circuits before reaching it).

Established patterns to follow:
- Lazy imports inside the non-mock branch (never at module top level).
- `loader_fn` closure captured with all parameters needed for construction.
- Cache key uses `(model_id, dtype_string)` format.
- Inline comments explaining non-obvious choices.
- Google-style docstrings on classes and non-trivial functions.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source  | Feature flags confirmed |
|--------|-----------|-----------------|-------------|------------------------|
| python | diffusers | 0.38.0          | pypi-query  | n/a (>=0.36.0 required) |
| python | torch     | (system-installed) | n/a      | n/a                     |

The `diffusers` package is already declared in `worker/requirements/base.txt` at
`>=0.36.0`. The current latest is `0.38.0`. The `AutoencoderKL` class and its
`from_pretrained()` classmethod have been present since early diffusers versions and
remain stable.

**Important note:** The exact loading call (`from_pretrained` vs `from_single_file`)
must be confirmed at ACT time by inspecting the Z-Image Turbo model's directory layout
and the diffusers source. The plan assumes `from_pretrained` with a `vae` subfolder,
matching the pattern established by `LoadModel`'s `from_pretrained(..., subfolder="unet")`.

## Approach

1. **Verify VAE directory layout at ACT time.** Before writing any code, inspect the
   Z-Image Turbo model files to confirm whether the VAE weights are in a `vae/`
   subfolder alongside a `config.json` (→ `from_pretrained`) or are a standalone
   `.safetensors` file (→ `from_single_file`). The plan's approach is contingent on
   this verification. This is the primary risk of this task — the ACT agent must confirm
   before writing.

2. **Replace the stub in `LoadVae.execute()`.** In the real-mode branch (after the
   mock-mode `return`), replace the `raise NotImplementedError(...)` with:
   - Lazy imports: `from diffusers import AutoencoderKL`, `import torch`.
   - A `loader_fn` closure that constructs `AutoencoderKL.from_pretrained(model_id,
     subfolder="vae", torch_dtype=torch.bfloat16)` (or the confirmed alternative).
     Inline comment explaining the construction choice.
   - Call `ctx.pipeline_cache.get_or_load(model_id, "bf16", loader_fn)` to cache the
     result.
   - Return `{"vae": result}`.

3. **Update the docstring's `Raises` section.** Remove the `NotImplementedError` from
   the `Raises:` paragraph in `LoadVae.execute()`'s docstring, since the stub no longer
   raises. If the method can now raise other exceptions (e.g., from diffusers loading),
   note those instead.

4. **Verify mock tests pass.** Run the existing test suite with mock mode. No new tests
   are needed because the real path is unreachable in mock mode (the environment check
   returns before reaching it).

## Public API Surface

None. This task modifies a private code path (the real-mode branch of `execute()`)
that is never directly called by external consumers. The public API (`NODE_TYPE`,
`INPUT_SLOTS`, `OUTPUT_SLOTS`, `execute()` signature) remains unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Replace `LoadVae.execute()`'s real path stub with actual AutoencoderKL loading via pipeline_cache |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadvae_registered_in_registry` | LoadVae is registered in NODE_REGISTRY | NODE_REGISTRY cleared, loader module reloaded | None | "LoadVae" in registry, NODE_TYPE matches | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_execute_returns_mock_vae` | execute() returns MockVae in mock mode | ANVILML_WORKER_MOCK=1 set by conftest | `model_id="test-vae"` | `{"vae": MockVae()}` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_metadata_attributes` | All six metadata attrs correct | Direct import of LoadVae | None | NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS all correct | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes -v` exits 0 |

Full suite: `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0.

## CI Impact

No CI changes required. The modification is confined to an existing Python source file
within the worker module. The `worker-linux` and `worker-windows` CI jobs automatically
pick up changes to `worker/*.py` and run `py_compile` followed by pytest. No new test
files or CI configuration changes are introduced.

## Platform Considerations

None identified. The diffusers `AutoencoderKL.from_pretrained()` call is platform-neutral
and works identically on Linux, Windows, and macOS. The model_id is a filesystem path
that `from_pretrained` resolves via standard Python path semantics. No `cfg` guards or
platform-specific code paths are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The VAE weights are a standalone `.safetensors` file without `config.json`, requiring `AutoencoderKL.from_single_file()` instead of `from_pretrained()`. The plan assumes `from_pretrained(..., subfolder="vae")` based on the LoadModel pattern. | Medium | High | ACT agent must inspect the Z-Image Turbo model directory layout at session start. If `from_single_file` is needed, adjust the loader_fn accordingly. Document the confirmed approach in the implementation report. |
| The `vae/` subfolder naming convention differs from the expected `subfolder="vae"` (e.g., it could be `vae_model/` or a flat structure). | Low | Medium | ACT agent must inspect actual model directory structure. The `subfolder` parameter in `from_pretrained` should match the actual directory containing both `config.json` and the VAE weight files. |
| `ctx.pipeline_cache` is typed as `dict[str, Any]` in `base.py` but is a `PipelineCache` at runtime. The `get_or_load()` call works because P903-A2 wires a real `PipelineCache` instance. If P903-A2 has not completed, this call will fail with `AttributeError`. | Low | High | P18-D5 depends on P18-D4 which depends on P18-D3b; P903-A2 is a prerequisite for the entire Phase 018 real-path work. Verify P903-A2 completion before proceeding. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0
