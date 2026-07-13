# Plan Report: P20-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-E1                                      |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | Runnable Proof: LoadModel node loads the ZiT fixture checkpoint for real |
| Depends on  | P20-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-14T00:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Execute the phase's Runnable Proof: run the real-mode pytest suites for `test_arch_zit.py` and `test_nodes_loader.py` against the P20-A1 ZiT fixture checkpoint, confirming the full real-mode chain succeeds — shape inference → meta construction → dtype selection → key remap → load → LoadModel's real branch — with zero `NotImplementedError` anywhere in the chain. This is a verification-only task (no new source files) whose acceptance criterion is a single pytest invocation exiting 0 with zero skips and zero xfails.

## Scope

### In Scope
- Running `python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode` as the Runnable Proof
- Verifying the full real-mode chain: shape inference (`_infer_hyperparams`), meta-device construction (`ZiTModel` on `torch.device("meta")`), dtype selection (`_select_dtype`), key remapping (`_build_key_remapping`), weight loading (`load_state_dict`), and LoadModel's real branch (`LoadModel.execute()` → `get_module("zit")` → `module.load()` → `PipelineCache.get_or_load()`)
- Confirming zero skips and zero xfails in this specific invocation
- Recording the literal pytest output in the implementation report (per FORGE_AGENT_RULES.md §5.14)

### Out of Scope
None. This task has `defers_to: []` and must implement its full scope. The proof command is the complete deliverable.

## Existing Codebase Assessment

The codebase at this point has all Phase 20 tasks (P20-A1 through P20-D1) implemented:

1. **Fixture**: `worker/tests/fixtures/zit_tiny.safetensors` (100 KB, arch="zit" metadata, F32 native dtype) and `zit_tiny_no_metadata.safetensors` (100 KB, no arch metadata, xyz_ prefixed keys) exist and are committed.

2. **zit.py** (`worker/nodes/arch/diffusion/zit.py`, 718 lines): Fully implements all four steps of the loading contract:
   - `_infer_hyperparams()` reads ALL keys via `f.keys()`, infers hidden_dim, block counts, latent dimensions, patch_size, arch string, and native_dtype. Includes P904 regression prevention (no truncation).
   - `can_handle("zit")` returns True for the canonical ZiT architecture string.
   - `load(path, caps, device)` chains: infer → select dtype → construct on meta → materialize via `to_empty()` → load_file → remap keys → cast before `load_state_dict(assign=True, strict=False)`.
   - `_build_key_remapping()` handles direct matches and pattern-based remapping for ZiT's `proj.weight` → `in_proj_weight` convention.
   - Dual-mode parity markers present: `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` on `load()`.

3. **loader.py** (`worker/nodes/loader.py`): LoadModel's real branch has been updated (P20-D1) to dispatch through `get_module("zit")` → `module.load(inputs["model_id"], ctx.caps)` → `PipelineCache.get_or_load()`. The `NotImplementedError` placeholder is gone. LoadModel carries both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers pointing at `test_load_model_real_loads_zit_fixture` and `test_load_model_mock_returns_sentinel` respectively.

4. **Dispatch**: `worker/nodes/arch/diffusion/__init__.py` imports `zit` and appends it to `_REGISTERED_MODULES`, giving `get_module("zit")` its first real entry.

5. **Test files**: `test_arch_zit.py` has 20+ tests covering inference, dispatch, dtype selection, load, remapping, and error cases — both real and mock modes. `test_nodes_loader.py` has `test_load_model_real_loads_zit_fixture` (real_mode-marked) that exercises the full LoadModel → zit load chain.

6. **Established patterns**: Google-style docstrings on all Python classes/functions. Inline `#` comments at every decision point. `# REAL_PATH_VERIFIED:` / `# MOCK_PATH_VERIFIED:` marker pairs on every function with dual-mode tests. Fixture checkpoints are tiny synthetic `.safetensors` files, never real weights.

## Resolved Dependencies

None. This task introduces no new dependencies — it runs existing test suites against existing code. All dependencies (torch, safetensors, pytest) were resolved in prior phase tasks.

## Approach

**Step 1 — Prerequisite verification.** Confirm that all prerequisite tasks (P20-A1 through P20-D1) have been completed by verifying the existence and correctness of the source files this task exercises:
- `worker/tests/fixtures/zit_tiny.safetensors` exists (P20-A1)
- `worker/nodes/arch/diffusion/zit.py` contains `_infer_hyperparams`, `can_handle`, `load`, `_select_dtype`, `_build_key_remapping` (P20-B1, B2, C1, C2, C3)
- `worker/nodes/loader.py` LoadModel's real branch calls `get_module("zit")` → `module.load()` (P20-D1)
- `worker/nodes/arch/diffusion/__init__.py` registers `zit` in `_REGISTERED_MODULES` (P20-B2)

**Step 2 — Syntax/compile check.** Run `python -m py_compile` on all `.py` files under `worker/` to rule out syntax errors before running pytest (per ENVIRONMENT.md §7 / FORGE_AGENT_RULES.md §5.11):
```bash
worker/.venv/bin/python -m py_compile $(git ls-files 'worker/*.py')
```

**Step 3 — Runnable Proof execution.** Run the acceptance criterion command:
```bash
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
```

This invokes pytest with the `-m real_mode` marker, which selects only tests decorated with `@pytest.mark.real_mode`. The tests exercised are:

From `test_arch_zit.py` (real-mode subset):
- `test_dtype_selection_bf16_real` — load() with bf16 caps, verifies ZiTModel on cpu at bfloat16
- `test_load_real_zit_fixture` — full end-to-end load against regular fixture
- `test_load_no_metadata_real` — full load against no-metadata fixture via fallback path

From `test_nodes_loader.py` (real-mode subset):
- `test_load_model_real_loads_zit_fixture` — LoadModel.execute() → real branch → zit.load() → PipelineCache
- `test_load_vae_real_raises_not_implemented` — LoadVae real branch (expected NotImplementedError, this is correct — VAE loading is genuinely deferred)
- `test_load_vae_real_cache_key_format` — LoadVae real branch cache key verification
- `test_load_vae_real_raises_no_diffusion_arch` — LoadVae real branch error message
- `test_load_clip_real_raises_not_implemented` — LoadClip real branch (expected NotImplementedError, correctly deferred)
- `test_load_clip_real_cache_key_format` — LoadClip real branch cache key verification
- `test_load_clip_real_raises_no_diffusion_arch` — LoadClip real branch error message

**Step 4 — Verify zero skips and zero xfails.** Parse the pytest output to confirm:
- All selected tests show `PASSED` (not `SKIPPED` or `XFAIL`)
- The summary line shows `0 skipped, 0 xfailed`

**Step 5 — Record output.** Paste the verbatim pytest output into the implementation report's `## Runnable Proof Transcript` section (per FORGE_AGENT_RULES.md §5.14).

### Phase Deliverable Audit

P20-E1 is the last task in phase 20's `tasks_phase020.json`. Per FORGE_AGENT_RULES.md §9a, §9a.1, and §9a.2, the following mechanical audits were run:

**§9a — defers_to coverage audit:**
- All tasks in phase 20 have `defers_to: []` (empty). The only non-empty `defers_to` entries exist in P20-B1→P20-B2 and P20-C1→P20-C2, but those are within-phase forward references already resolved by the time P20-E1 runs.
- No `defers_to` target in this phase needs cross-task verification for §9a because all deferred scope was delivered within the same phase by the target tasks themselves.
- Result: 0 findings.

**§9a.1 — Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" worker/nodes/arch/diffusion/zit.py worker/nodes/loader.py worker/pipeline_cache.py worker/nodes/base.py worker/nodes/arch/diffusion/__init__.py
```
Output: `NotImplementedError` appears only in `worker/nodes/loader.py` within `LoadVae` and `LoadClip` classes — these are correctly deferred functionality (VAE and CLIP loading are genuinely not implemented in this phase). LoadModel's real branch contains no `NotImplementedError`. No `todo!` or `// TODO` markers found.
- Result: "Unmarked-stub sweep: 0 findings"

**§9a.2 — Dual-mode parity-marker sweep:**
```bash
grep -L "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py worker/nodes/loader.py
# Output: (empty — both files contain REAL_PATH_VERIFIED)

grep -L "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py worker/nodes/loader.py
# Output: (empty — both files contain MOCK_PATH_VERIFIED)
```
- Result: "Dual-mode parity-marker sweep: 0 findings"

## Public API Surface

None. This task introduces no new source code, types, or public API items. It exercises existing public APIs:
- `worker/nodes/arch.diffusion.zit._infer_hyperparams(path: str) -> dict[str, Any]`
- `worker/nodes.arch.diffusion.zit.load(path: str, caps: dict, device: str = "cpu") -> ZiTModel`
- `worker.nodes.loader.LoadModel.execute(ctx: NodeContext, **inputs) -> dict`
- `worker.pipeline_cache.PipelineCache.get_or_load(key: str, loader_fn: Callable[[], Any]) -> Any`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | worker/tests/test_arch_zit.py | Existing real-mode test suite — no modifications |
| Read | worker/tests/test_nodes_loader.py | Existing real-mode test suite — no modifications |
| Read | worker/nodes/arch/diffusion/zit.py | Source under test — no modifications |
| Read | worker/nodes/loader.py | Source under test — no modifications |

## Tests

This task does not introduce new tests. It exercises existing real-mode tests. The acceptance criterion is the pytest invocation itself.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| worker/tests/test_arch_zit.py | test_dtype_selection_bf16_real (real) | load() with bf16 caps produces ZiTModel on cpu at bfloat16 | `python -m pytest worker/tests/test_arch_zit.py -v -m real_mode` |
| worker/tests/test_arch_zit.py | test_load_real_zit_fixture (real) | Full end-to-end load against regular ZiT fixture, .arch == "zit" | same as above |
| worker/tests/test_arch_zit.py | test_load_no_metadata_real (real) | Full load against no-metadata fixture via fallback path | same as above |
| worker/tests/test_nodes_loader.py | test_load_model_real_loads_zit_fixture (real) | LoadModel.execute() real branch → zit.load() → PipelineCache | same as above |
| worker/tests/test_nodes_loader.py | test_load_vae_real_raises_not_implemented (real) | LoadVae real branch raises NotImplementedError (correctly deferred) | same as above |
| worker/tests/test_nodes_loader.py | test_load_vae_real_cache_key_format (real) | LoadVae real branch cache key verification | same as above |
| worker/tests/test_nodes_loader.py | test_load_vae_real_raises_no_diffusion_arch (real) | LoadVae real branch error message | same as above |
| worker/tests/test_nodes_loader.py | test_load_clip_real_raises_not_implemented (real) | LoadClip real branch raises NotImplementedError (correctly deferred) | same as above |
| worker/tests/test_nodes_loader.py | test_load_clip_real_cache_key_format (real) | LoadClip real branch cache key verification | same as above |
| worker/tests/test_nodes_loader.py | test_load_clip_real_raises_no_diffusion_arch (real) | LoadClip real branch error message | same as above |

Note: LoadVae and LoadClip real-mode tests correctly expect `NotImplementedError` — these are genuinely deferred to later phases (VAE loading and CLIP loading are not in scope for Phase 20). Their presence in the real-mode suite is expected and does not indicate a failure.

## CI Impact

No CI changes required. The real-mode tests already exist and are collected by the `worker-linux-real` and `worker-windows-real` CI jobs (ENVIRONMENT.md §6, CI job matrix). This task does not modify any CI configuration, test markers, or test collection behavior.

## Platform Considerations

None identified. The real-mode tests run on torch CPU, which is a first-class fully tested device per ANVILML_DESIGN.md §2.2. The fixture checkpoint is synthetic and structurally valid on any platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient for Rust code; this task only exercises Python code which is platform-neutral (no path-separator or line-ending handling required).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A prior phase task (P20-A1 through P20-D1) was not completed, so the fixture file or zit.py source is missing or incomplete, causing the pytest invocation to fail at collection time. | Low | High | Verify prerequisite existence in Step 1 before running pytest. If a prerequisite is missing, report it as a blocker rather than attempting to work around it. |
| The fixture checkpoint was built with tensor shapes that don't match the constructed ZiTModel, causing `load_state_dict` to skip all tensors and produce a model with only zero-initialized parameters — the test would pass structurally but not verify actual weight loading. | Low | Medium | The fixture was built by P20-A1 against the same shape-inference formula used by zit.py. The `test_load_real_zit_fixture` test verifies `.arch == "zit"` and parameters on cpu, which is the established acceptance. If weight values are all zero, that is a P20-C3 defect to surface in the report. |
| The `@pytest.mark.real_mode` marker is not registered in pytest config, causing pytest to warn and potentially skip all tests. | Low | Medium | The marker is registered in `worker/pyproject.toml` per ENVIRONMENT.md §11.2. If it is missing, pytest will emit a warning but still collect the tests — the exit code will still reflect pass/fail. Verify by checking `pyproject.toml` for the `[tool.pytest.ini_options]` or `[pytest]` section. |
| `torch` is not installed in the Python venv, causing import failures at collection time. | Low | High | Per ENVIRONMENT.md §5, real-mode tests require torch. The fixture runner should have installed `requirements/cpu-linux-agent.txt`. If torch is missing, install it before running the proof. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode` exits 0
- [ ] The pytest output contains zero `SKIPPED` markers in the selected test set
- [ ] The pytest output contains zero `XFAIL` markers in the selected test set
- [ ] All real-mode tests from `test_arch_zit.py` show `PASSED` (test_dtype_selection_bf16_real, test_load_real_zit_fixture, test_load_no_metadata_real)
- [ ] All real-mode tests from `test_nodes_loader.py` show `PASSED` (test_load_model_real_loads_zit_fixture, test_load_vae_real_*, test_load_clip_real_*)
- [ ] The literal pytest output is recorded in the implementation report's `## Runnable Proof Transcript` section
