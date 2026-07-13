# Plan Report: P19-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P19-D1                                      |
| Phase       | 019 — Model Loading Contract Groundwork     |
| Description | worker/tests/fixtures/: fixture-checkpoint builder conventions doc |
| Depends on  | P19-C3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T11:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/tests/fixtures/README.md` — a documentation file that codifies the
fixture-checkpoint convention which every arch-module task in Phase 20+ must follow
when building real-mode test fixtures. The convention specifies that fixtures must be
tiny synthetic `.safetensors` files (never real downloaded weights), with tensor shapes
chosen to be structurally valid for the architecture's shape-inference formula while
staying well under 1 GB peak RAM, and that at least one fixture per diffusion/CLIP/VAE
family must exercise the metadata-fallback path by having a non-recognizable key prefix
and no `arch` metadata key. No actual fixture files are created — only the convention
document itself.

## Scope

### In Scope
- Create `worker/tests/fixtures/README.md` documenting:
  - The purpose of the fixtures directory (tiny synthetic `.safetensors` for real-mode tests)
  - The sizing rule: shapes structurally valid for shape-inference, not miniaturized real-model copies, well under 1 GB peak RAM
  - The three arch families covered (diffusion, CLIP, VAE) and their naming conventions
  - The mandatory metadata-fallback regression case: one fixture per family with non-recognizable key prefix and no `arch` metadata key, exercising the fallback path that v3's `st.metadata` vs `st.metadata()` bug exploited
  - The file naming convention for fixtures
  - The builder script convention (a Python script that creates fixtures deterministically)
  - The relationship between fixtures and the loader nodes (LoadModel, LoadVae, LoadClip)
  - How to create a new fixture (step-by-step for Phase 20+ authors)
- The `worker/tests/fixtures/` directory itself does not need to be created by this task
  (it is referenced as a path; the ACT agent will create it as a prerequisite step when
  the first real fixture is needed in Phase 20)

### Out of Scope
- No actual `.safetensors` fixture files are created (per task specification)
- No builder script (`worker/tests/fixtures/build_fixtures.py` or similar) is authored
- No changes to existing test files, loader nodes, or arch module dispatch files
- No changes to CI configuration
- No changes to `docs/TESTS.md` (no new tests are introduced; this is documentation only)

## Existing Codebase Assessment

No prior source exists in `worker/tests/fixtures/` — the directory does not yet exist on
disk. This task establishes the baseline documentation that Phase 20+ arch-module tasks
will follow when building real-mode fixtures.

The existing codebase provides the context this convention must document:
- **Loader nodes** (`worker/nodes/loader.py`): `LoadModel`, `LoadVae`, `LoadClip` each
  have a mock branch (returning sentinel dicts) and a real branch that raises
  `NotImplementedError("no diffusion arch module registered yet")`. The real branches
  delegate to `pipeline_cache.get_or_load()` — the infrastructure is in place; the
  actual safetensors reading and arch dispatch is deferred to Phase 20.
- **Arch module dispatch** (`worker/nodes/arch/{diffusion,clip,vae}/__init__.py`): Each
  family has a `get_module(key)` dispatcher that scans `_REGISTERED_MODULES` for the
  first module whose `can_handle(key)` returns `True`. With zero registered modules,
  `get_module` returns `None` silently.
- **Pipeline cache** (`worker/pipeline_cache.py`): An LRU cache keyed by model/component
  identifier. The cache stores raw components only — it does not manage assembled
  pipelines. Failed loader calls do not populate the cache.
- **Dual-mode parity markers**: Every node's `execute()` method carries
  `REAL_PATH_VERIFIED:` and `MOCK_PATH_VERIFIED:` comment markers pointing at specific
  test functions in `worker/tests/test_nodes_loader.py`. This convention applies to
  arch module `load()`/`sample()`/`decode()` methods as well, which is why fixtures
  are needed — real-mode tests must load actual safetensors files to exercise the
  `load()` path.
- **The metadata-fallback bug class**: v3 shipped a bug where `st.metadata` (a property
  reference) was called as `st.metadata()` (a method call), producing incorrect results
  for checkpoints without a recognizable `arch` metadata key. This bug was never caught
  because every real fixture used so far had a recognizable prefix and never hit the
  fallback path. The convention mandates that at least one fixture per family exercises
  this path.

## Resolved Dependencies

None. This task is documentation-only and introduces no external dependencies.

## Approach

1. **Verify prerequisite state.** Confirm that `worker/tests/fixtures/` directory exists
   (or will be created by the ACT agent as a first step). The directory is referenced
   by the design doc (§17.5) and the architecture doc (§2, worker/tests/fixtures/) but
   does not yet exist on disk. The ACT agent must `mkdir -p worker/tests/fixtures/`
   before creating `README.md`.

2. **Write `worker/tests/fixtures/README.md`.** The document must cover the following
   sections in order:

   a. **Purpose** — One paragraph explaining that this directory holds tiny synthetic
      `.safetensors` checkpoints for real-mode tests, never real downloaded weights.
      Reference `ANVILML_DESIGN.md §17.5`.

   b. **Sizing rules** — Three bullet points:
      - Tensor shapes must be structurally valid for the architecture's shape-inference
        formula (the loader node's dispatch path will attempt to load the file; shapes
        that fail shape inference are not useful fixtures)
      - Shapes must NOT be a miniaturized copy of real model shapes verbatim
        (the purpose is structural validity, not dimensional accuracy)
      - Peak RAM to construct a fixture must stay well under 1 GB (the 10GB CI/agent
        VM constraint from §17.5)

   c. **Arch families and naming** — A table listing the three families, their dispatch
      module paths, and the fixture naming convention:
      - `diffusion` → `worker/nodes/arch/diffusion/` → `fixture_<family>.safetensors`
      - `clip` → `worker/nodes/arch/clip/` → `fixture_<family>.safetensors`
      - `vae` → `worker/nodes/arch/vae/` → `fixture_<family>.safetensors`
      Use `<family>` as a placeholder (e.g., `fixture_zit.safetensors`,
      `fixture_flux2klein.safetensors`).

   d. **Metadata-fallback regression case** — This is the most critical section.
      Explain that:
      - v3 shipped a `st.metadata` vs `st.metadata()` bug (`worker/nodes/loader.py:702`
        in that codebase) that was never caught because every real fixture had a
        recognizable prefix
      - At least one fixture per family MUST have a non-recognizable key prefix (e.g.,
        `xyz_random_tensor_data` instead of `diffusion_model.*`) AND no `arch` metadata
        key, forcing the loader to use the metadata-fallback path
      - Name this the "metadata-fallback regression case" and explain why it exists
        (the exact bug class this rule was created to catch)

   e. **Builder script convention** — Describe that when Phase 20+ authors need to
      create a fixture, they should write a small Python script (e.g.,
      `worker/tests/fixtures/build_<family>.py`) that uses the `safetensors` library
      to create a tiny `.safetensors` file deterministically. The script should:
      - Accept no arguments (runs end-to-end)
      - Write the fixture file to `worker/tests/fixtures/`
      - Be idempotent (safe to re-run)
      - Use shapes that are structurally valid but small (e.g., `(1, 4, 8, 8)` for a
        latent tensor, `(768, 768)` for a diffusion model weight)

   f. **How to add a new fixture** — A step-by-step checklist:
      1. Identify the arch family and the key the loader will use
      2. Choose tensor shapes that are structurally valid for that family's shape-inference
      3. Write the builder script
      4. Create at least one "fallback" fixture with non-recognizable keys
      5. Run the builder script
      6. Verify the fixture loads correctly in mock mode (no torch needed)
      7. Verify the fixture loads correctly in real mode (torch CPU, exercises `load()`)

   g. **What this directory does NOT contain** — Clarify:
      - No real model weights (even scaled-down versions)
      - No checkpoints larger than what's needed for shape-inference validation
      - No fixture files from v3 or earlier versions of the codebase

3. **Verify the file.** Run `test -s worker/tests/fixtures/README.md` to confirm the
   file exists and is non-empty (the acceptance criterion).

## Public API Surface

None. This task creates a documentation file only — no Python functions, Rust items,
or other public API surface.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/README.md` | Fixture-checkpoint builder conventions documentation |

Note: The `worker/tests/fixtures/` directory itself will be created by the ACT agent as
a prerequisite step (the directory does not yet exist on disk).

## Tests

None. This task is documentation-only and introduces no tests. The acceptance criterion
is a shell test (`test -s worker/tests/fixtures/README.md`) that confirms the file
exists and is non-empty.

## CI Impact

No CI changes required. The README.md file is documentation only and is not executed
or tested by any CI job. The existing `worker-linux-mock` and `worker-linux-real` CI
jobs will continue to pick up all test files under `worker/tests/` as they already do.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The
README.md file uses standard Markdown formatting with no platform-specific content.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The README omits a detail that Phase 20+ authors need, causing them to guess or create inconsistent fixtures | Medium | Medium | The ACT agent should draft the README and then re-read the task context and ANVILML_DESIGN.md §17.5 to verify all mandated points are covered before writing. |
| The metadata-fallback section is unclear about what constitutes a "non-recognizable key prefix" | Low | Medium | Use concrete examples: `xyz_random_tensor_data` vs `diffusion_model.output`. Reference the exact v3 bug location (`worker/nodes/loader.py:702`) so authors can look up the code path. |
| The sizing guidance is too vague, leading Phase 20 authors to create oversized fixtures | Medium | High | Include specific shape examples (e.g., `(1, 4, 8, 8)` latent, `(768, 768)` weight) and explicitly state the "well under 1 GB" constraint with a concrete upper bound (e.g., "each fixture file should be under 10 MB"). |

## Acceptance Criteria

- [ ] `test -s worker/tests/fixtures/README.md` exits 0
