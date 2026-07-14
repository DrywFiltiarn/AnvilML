# Plan Report: P21-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P21-C1                                            |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/sampler.py: Sampler generic node, mock branch only |
| Depends on  | P21-B2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-14T16:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/nodes/sampler.py` with the `Sampler` generic node class, fully implementing its mock branch per `ANVILML_DESIGN.md §10.3` and `§14.6`, and a bare `NotImplementedError` placeholder for the real branch (deferred to P21-C2). The node registers itself in `NODE_REGISTRY` via `@register`, defines all seven input slots and two output slots per the spec, and branches on `ctx.mock` at the top of `execute()`. Mock-mode seed resolution is deterministic (`-1` → `0`). Accompanying test file `worker/tests/test_nodes_sampler.py` contains ≥3 tests verifying mock output shape, deterministic seed resolution, and registry presence.

## Scope

### In Scope
- Create `worker/nodes/sampler.py` with `Sampler` class:
  - All six required class attributes: `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS` — matching the exact slot specs from the task.
  - `execute(ctx, **inputs)` method branching on `ctx.mock` at the top.
  - Mock branch: returns `{"latent": {"mock": True, "shape": inputs["latent"].get("shape")}, "seed": inputs["seed"] if inputs["seed"] != -1 else 0}`.
  - Real branch: bare `raise NotImplementedError(...)` with `# defers_to: P21-C2` comment.
  - `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` marker comments per `ANVILML_DESIGN.md §10.6`.
- Create `worker/tests/test_nodes_sampler.py` with ≥3 tests:
  - Mock returns expected shape.
  - Mock seed=-1 resolves to 0 deterministically.
  - Node is in `NODE_REGISTRY`.
  - Real branch raises `NotImplementedError` (required for REAL_PATH_VERIFIED marker).

### Out of Scope
- Real branch actual sampling logic — deferred to P21-C2 (`worker/nodes/sampler.py: Sampler real branch dispatches to arch module`). P21-C2 will replace the placeholder `raise NotImplementedError` with `arch.diffusion.get_module(inputs["model"].arch).sample(...)`.
- No new external dependencies.
- No changes to `worker/nodes/__init__.py` — auto-import handles `sampler.py` registration automatically (the `@register` decorator in `sampler.py` populates `NODE_REGISTRY` at module load time, and `__init__.py`'s `_import_nodes()` skips only `__init__` and `base` modules, not `sampler`).

## Existing Codebase Assessment

**What already exists:** The node system is fully scaffolded. `worker/nodes/base.py` defines the `BaseNode` ABC with an `execute()` abstract method, the `NodeContext` dataclass-like class (with `mock` bool attribute), the `SlotSpec` dataclass, and the `@register` decorator that populates `NODE_REGISTRY`. `worker/nodes/__init__.py` auto-imports all `.py` files under `nodes/` (skipping `__init__`, `base`, and packages) to trigger `@register` side effects.

**Established patterns:**
- **Class attributes:** Every node defines `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS` as class-level constants. `INPUT_SLOTS` and `OUTPUT_SLOTS` are lists of `SlotSpec` tuples.
- **execute() branching:** Every node's `execute()` method branches on `ctx.mock` at the very top — the mock branch returns a sentinel dict, the real branch either dispatches to arch modules or raises `NotImplementedError`.
- **Mock sentinel shape:** Mock returns `{"key": {"mock": True, ...}}` with relevant context values propagated.
- **Test patterns:** Tests use a `_make_ctx()` helper that constructs minimal `NodeContext` objects. Registry tests use subprocess isolation (spawn a fresh Python process, import the module, check `NODE_REGISTRY`). Real-mode tests are marked with `@pytest.mark.real_mode`.
- **Dual-mode markers:** Every `execute()` method carries `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` comments pointing to specific test function paths.

**Gap between design doc and source:** The design doc (`ANVILML_DESIGN.md §10.3`) describes the Sampler node's slots in a table. The current source has no `sampler.py` yet — only `loader.py` (LoadModel, LoadVae, LoadClip), `passthrough.py`, and `base.py` exist. The slot names in the design doc use CamelCase (`MODEL`, `CONDITIONING`) while `SlotSpec`'s `slot_type` field is a string that matches these values. The `load()` method on arch modules takes `(model_id, caps, device)` — but `Sampler` dispatches to `.sample()` which has a different signature (per `ANVILML_DESIGN.md §10.4`'s dispatch table, `sample(...)` is per-§11.6). This gap is handled by P21-C2, not P21-C1.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | msgpack | 1.0.8           | pypi-query MCP | n/a                    |
| python | pyzmq   | 26.2.0          | pypi-query MCP | n/a                    |

None of these are new dependencies — they are already in `worker/requirements/base.txt` and imported by existing nodes. The `Sampler` node itself imports only from `worker.nodes.base` (BaseNode, NodeContext, SlotSpec, register), all of which already exist. No external crates or packages are introduced.

## Approach

1. **Create `worker/nodes/sampler.py`.** Write the file with a module-level docstring describing the Sampler node's purpose, its mock/real branches, and its slot configuration. Import `BaseNode`, `NodeContext`, `SlotSpec`, and `register` from `worker.nodes.base`.

2. **Define the `Sampler` class.** Inside the class, set the six required class attributes:
   - `NODE_TYPE = "Sampler"`
   - `CATEGORY = "Sampling"`
   - `DISPLAY_NAME = "Sampler"`
   - `DESCRIPTION = "Runs a denoising diffusion step to produce a latent from a model, conditioning, and latent input."`
   - `INPUT_SLOTS = [SlotSpec("model","MODEL"), SlotSpec("conditioning","CONDITIONING"), SlotSpec("clip","CLIP"), SlotSpec("latent","LATENT"), SlotSpec("steps","INT"), SlotSpec("cfg","FLOAT"), SlotSpec("seed","INT")]`
   - `OUTPUT_SLOTS = [SlotSpec("latent","LATENT"), SlotSpec("seed","INT")]`

3. **Add dual-mode parity markers.** Place `# REAL_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented` and `# MOCK_PATH_VERIFIED: worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape` as module-level comments immediately before the `execute()` method, following the exact format from `ANVILML_DESIGN.md §10.6`.

4. **Implement `execute(self, ctx: NodeContext, **inputs) -> dict`.** The method signature uses the same typing as existing nodes. The method body:
   - **First line:** `if ctx.mock:` — branch on mock flag at the top, per §14.6.
   - **Mock branch:** Return `{"latent": {"mock": True, "shape": inputs["latent"].get("shape")}, "seed": inputs["seed"] if inputs["seed"] != -1 else 0}`. The seed resolution is deterministic: `-1` maps to `0`, any other value passes through unchanged. This ensures reproducible mock-mode test output.
   - **Real branch:** `raise NotImplementedError("Sampler real branch deferred to P21-C2 — dispatches to arch.diffusion.get_module(model.arch).sample()")`. Add `# defers_to: P21-C2` comment above the raise, per `FORGE_AGENT_RULES.md §9.7`.
   - Include Google-style docstring with `Args:`, `Returns:`, and `Raises:` sections.

5. **Apply `@register` decorator.** Place `@register` above the class definition, before the class body. This populates `NODE_REGISTRY["Sampler"] = Sampler` at module load time.

6. **Create `worker/tests/test_nodes_sampler.py`.** Write the test file with:
   - A `_make_ctx()` helper (same pattern as `test_passthrough.py` and `test_nodes_loader.py`).
   - `test_sampler_class_attributes()`: verify all six class attributes match expected values.
   - `test_sampler_mock_returns_expected_shape()`: mock mode returns correct sentinel with shape propagated from inputs.
   - `test_sampler_mock_seed_zero()`: seed=-1 resolves to 0 deterministically.
   - `test_sampler_real_raises_not_implemented()`: real mode raises NotImplementedError (required for REAL_PATH_VERIFIED marker). Marked with `@pytest.mark.real_mode`.
   - `test_sampler_in_registry()`: subprocess isolation test confirming `Sampler` appears in `NODE_REGISTRY`.

7. **Add inline documentation.** Per `ANVILML_DESIGN.md §4.5` and `ENVIRONMENT.md §10`:
   - Module-level docstring explaining the node's purpose and mock/real branches.
   - Class docstring with Google-style format (summary, class attributes, Args/Returns/Raises in execute()).
   - Inline comment at the mock/real branch explaining why each path exists.
   - `# defers_to: P21-C2` comment on the real branch raise per §9.7.

## Public API Surface

| Item | Path | Signature / Description |
|------|------|------------------------|
| Class | `worker.nodes.sampler.Sampler` | Extends `BaseNode`. Class attributes: `NODE_TYPE="Sampler"`, `CATEGORY="Sampling"`, `INPUT_SLOTS` (7 SlotSpecs), `OUTPUT_SLOTS` (2 SlotSpecs). |
| Method | `Sampler.execute` | `def execute(self, ctx: NodeContext, **inputs) -> dict` — branches on `ctx.mock`. Mock returns sentinel dict; real raises `NotImplementedError`. |
| Decorator side-effect | `NODE_REGISTRY` | `NODE_REGISTRY["Sampler"] = Sampler` (populated at module load via `@register`). |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/sampler.py` | New file: `Sampler` node class with mock branch + stub real branch. |
| CREATE | `worker/tests/test_nodes_sampler.py` | New file: ≥4 tests for `Sampler` (mock shape, mock seed, registry, real raises). |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_sampler.py` | `test_sampler_class_attributes` | All six class attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`) match expected values exactly. | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_class_attributes -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_mock_returns_expected_shape` (mock) | Mock-mode `execute()` returns `{"latent": {"mock": True, "shape": <input_shape>}, "seed": <input_seed>}` with shape propagated from `inputs["latent"]`. Satisfies `MOCK_PATH_VERIFIED` marker. | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_mock_seed_zero` (mock) | When `seed=-1`, mock returns `{"seed": 0}` (deterministic resolution). When `seed=42`, returns `{"seed": 42}`. | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_seed_zero -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_real_raises_not_implemented` (real) | Real-mode `execute()` raises `NotImplementedError` with message referencing P21-C2. Satisfies `REAL_PATH_VERIFIED` marker. | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_in_registry` | Subprocess isolation test: fresh Python process imports `worker.nodes.sampler`, confirms `NODE_REGISTRY["Sampler"]` exists and is the `Sampler` class. | `python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_in_registry -v` exits 0 |

## CI Impact

No CI changes required. The new test file is automatically picked up by the existing pytest invocation in ENVIRONMENT.md §6 Step 8 (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v -m "not real_mode"`). The real-mode test (`@pytest.mark.real_mode`) is automatically picked up by Step 9 (`python -m pytest worker/tests/ -v -m real_mode`). No new CI jobs, markers, or configuration entries are needed — the `real_mode` marker is already registered in `pyproject.toml`/`pytest.ini`.

## Platform Considerations

None identified. The `Sampler` node is a pure Python data transformation with no file I/O, no platform-specific paths, no line-ending handling, and no subprocess spawning. The mock branch uses only dictionary operations and the `get()` method on dict. The real branch raises an exception. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `inputs["latent"]` may not be a dict in edge cases — the mock branch calls `inputs["latent"].get("shape")` which assumes `inputs["latent"]` is a dict (matching the sentinel shape from `EmptyLatent`). If a caller passes a non-dict latent, `AttributeError` will be raised instead of a clean mock output. | Low | Medium | The latent input always comes from a previous node's output in the graph. In mock mode, `EmptyLatent` returns a dict with `"shape"`. In real mode, the latent is a `torch.Tensor` but the real branch raises before reaching the mock code. Document this assumption in the mock branch inline comment. The ACT agent will verify the actual `EmptyLatent` output shape from Phase 19's implementation. |
| `defers_to: P21-C2` comment on the real branch raise may be missed by the ACT agent if it focuses only on the mock branch. | Low | Low | The plan explicitly lists the `# defers_to: P21-C2` comment as a required inline comment in step 7. The FORGE_AGENT_RULES.md §9.7 rule makes this mandatory when `defers_to` is non-empty. |
| Test file naming convention mismatch — if `test_nodes_sampler.py` does not follow the established pattern of `test_nodes_<module>.py`, pytest may not discover it. | Very Low | Low | The plan uses `test_nodes_sampler.py` which matches the established pattern (`test_nodes_loader.py`, `test_nodes_image.py` listed in ARCHITECTURE.md). The ACT agent should verify the naming against existing files. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/sampler.py` exits 0 (syntax check before test run)
- [ ] `python -m py_compile worker/tests/test_nodes_sampler.py` exits 0 (syntax check before test run)
- [ ] `python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 with ≥3 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_nodes_sampler.py -v -m "not real_mode"` exits 0 (mock-mode subset)
- [ ] `python -m pytest worker/tests/test_nodes_sampler.py -v -m real_mode` exits 0 (real-mode subset — verifies NotImplementedError is raised)
