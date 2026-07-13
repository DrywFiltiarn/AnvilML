# Plan Report: P19-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P19-E1                                      |
| Phase       | 19 — Model Loading Contract Groundwork      |
| Description | CI: worker-test job collects loader.py + pipeline_cache.py tests |
| Depends on  | P19-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Confirm — without any CI file edit — that this phase's new test files
(`worker/tests/test_pipeline_cache.py` and `worker/tests/test_nodes_loader.py`) are
already collected and executed by the existing `worker-test` CI job from Phase 9's
P9-F1 wiring. The acceptance criterion is that both mock-mode and real-mode pytest
invocations exit 0 and their output confirms the new test files are present.

## Scope

### In Scope
- Verify `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v` exits 0 and its output
  includes `test_pipeline_cache.py` and `test_nodes_loader.py` test collection/execution.
- Verify `python -m pytest worker/tests -v -m real_mode` exits 0 and its output includes
  the LoadModel/LoadVae/LoadClip `NotImplementedError` real-mode tests.
- Read and confirm the CI workflow (`.github/workflows/ci.yml`) already runs the exact
  pytest commands that cover these tests — no structural change needed.

### Out of Scope
None. This task defers no scope (`defers_to: []`). No CI file edits, no source changes,
no test additions. Verification only.

## Existing Codebase Assessment

This phase's groundwork tasks (P19-A1 through P19-D1) have already produced the following
deliverables that this task verifies are covered by CI:

1. **`worker/pipeline_cache.py`** — A `PipelineCache` class using `collections.OrderedDict`
   for O(1) LRU eviction. Provides `get_or_load(key, loader_fn)` with the contract that
   failed loader calls (exceptions) do not populate the cache. Tested by
   `worker/tests/test_pipeline_cache.py` with 6 tests covering caching, eviction,
   recency refresh, custom capacity, and post-eviction re-load.

2. **`worker/nodes/loader.py`** — Three node classes (`LoadModel`, `LoadVae`, `LoadClip`)
   each with a mock branch that returns a sentinel dict and a real branch that calls
   `pipeline_cache.get_or_load()` with a loader_fn raising
   `NotImplementedError("no diffusion arch module registered yet")`. Both branches have
   `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker pairs pointing to their respective
   tests in `test_nodes_loader.py`. Tested by
   `worker/tests/test_nodes_loader.py` with 15 tests (5 per node: mock sentinel,
   real NotImplementedError, registry registration, cache key format, canonical
   real-mode raise).

3. **`.github/workflows/ci.yml`** — The `worker-test` job already runs:
   - Mock mode: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"`
   - Real mode: `python -m pytest worker/tests -v -m real_mode`
   These commands use the glob `worker/tests` which automatically picks up any new
   `test_*.py` file in that directory — no configuration change needed.

The established patterns to follow: all tests use `pytest.mark.real_mode` for real-mode
tests, mock-mode tests have no marker (or `not real_mode`), and the CI matrix runs
both modes on ubuntu-latest and windows-latest.

No gap between the design doc and current source affects this verification task — the
design doc (§10.6) specifies the marker convention, and the source files implement it
correctly.

## Resolved Dependencies

None. This task performs verification only — no new dependencies are introduced or
referenced. The existing test infrastructure (pytest, msgpack, pyzmq, base.txt) is
already covered by the CI workflow and was verified in prior phases.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| (none) | (none)  | (N/A)           | (N/A)      | (N/A)                   |

## Approach

This task requires no code changes. The approach is purely verification via two
pytest invocations whose acceptance criteria are explicitly defined in the task
context and TASKS_PHASE019.md.

**Step 1 — Verify mock-mode coverage.** Run:
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
```
Confirm:
- Exit code is 0.
- Output includes `test_pipeline_cache.py` (6 tests: get_or_load_cached,
  get_or_load_different_keys, lru_eviction, access_refreshes_recency,
  custom_max_entries, evicted_entry_is_truly_removed).
- Output includes `test_nodes_loader.py` tests for the mock sentinel cases
  (`test_load_model_mock_returns_sentinel`, `test_load_vae_mock_returns_sentinel`,
  `test_load_clip_mock_returns_sentinel`, `test_load_model_in_registry`,
  `test_load_vae_in_registry`, `test_load_clip_in_registry`).

**Step 2 — Verify real-mode coverage.** Run:
```bash
python -m pytest worker/tests -v -m real_mode
```
Confirm:
- Exit code is 0.
- Output includes `test_nodes_loader.py` tests for the real-mode `NotImplementedError`
  cases (`test_load_model_real_raises_not_implemented`,
  `test_load_vae_real_raises_not_implemented`, `test_load_clip_real_raises_not_implemented`,
  `test_load_model_real_cache_key_format`, `test_load_vae_real_cache_key_format`,
  `test_load_clip_real_cache_key_format`, `test_load_model_real_raises_no_diffusion_arch`,
  `test_load_vae_real_raises_no_diffusion_arch`, `test_load_clip_real_raises_no_diffusion_arch`).

**Step 3 — Confirm CI wiring.** Read `.github/workflows/ci.yml` and confirm that the
`worker-test` job's mock-mode step runs `python -m pytest worker/tests -v -m "not real_mode"`
and the real-mode step runs `python -m pytest worker/tests -v -m real_mode`. Both commands
use the `worker/tests` glob, which automatically includes any new `test_*.py` file — no
config change is needed for this phase's new test files.

**Phase Deliverable Audit** (required for phase-closing task, §9a/FORGE_AGENT_RULES.md):

*§9a — defers_to procedure:*
- P19-C1 has `defers_to=['P19-C2']`. LoadModel's `NotImplementedError` at
  `worker/nodes/loader.py:78` carries the comment `// defers_to: P19-C2` at line 28.
  ✓ Accounted for.
- No other tasks in the phase have non-empty `defers_to`.

*§9a.1 — Unmarked-stub sweep:*
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" worker/nodes/loader.py worker/pipeline_cache.py
```
Results: 19 matches found in `worker/nodes/loader.py`. Analysis:
- LoadModel's `NotImplementedError` (line 78): Has `// defers_to: P19-C2` comment at
  line 28. P19-C1 has `defers_to=['P19-C2']`. ✓ Accounted for.
- LoadVae's `NotImplementedError` (line 149): No `defers_to:` comment present. P19-C3
  has `defers_to=[]`. **Finding** — the stub site has no defers_to comment and the
  originating task has empty defers_to. This is a task-authoring defect from P19-C3,
  not introduced by P19-E1. The real branch is intentional (deferred to P20) but
  the defers_to comment is missing from the source code.
- LoadClip's `NotImplementedError` (line 224): Same situation as LoadVae. **Finding**.

*§9a.2 — Dual-mode parity-marker sweep:*
```bash
grep -L "REAL_PATH_VERIFIED:" worker/nodes/loader.py
# Result: (empty — all files containing execute() have the marker)

grep -L "MOCK_PATH_VERIFIED:" worker/nodes/loader.py
# Result: (empty — all files containing execute() have the marker)
```
All three node classes (`LoadModel`, `LoadVae`, `LoadClip`) have both
`REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers present in `worker/nodes/loader.py`.
✓ No findings.

*Summary:* §9a passes. §9a.1 finds 2 unmarked stubs (LoadVae and LoadClip
`NotImplementedError` without `defers_to:` comments, from P19-C3). §9a.2 passes.
The §9a.1 findings are pre-existing task-authoring defects from P19-C3, not
introduced by this verification task. The phase cannot formally close until
P19-C3's stubs are either given `defers_to:` comments or P19-C3's scope is
corrected. P19-E1's verification scope is unaffected.

## Public API Surface

None. This task introduces no new public items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | `.github/workflows/ci.yml` | Confirm CI wiring for worker tests |
| Read | `worker/tests/test_pipeline_cache.py` | Verify test file exists and is collected |
| Read | `worker/tests/test_nodes_loader.py` | Verify test file exists and is collected |
| Read | `worker/pipeline_cache.py` | Confirm source under test |
| Read | `worker/nodes/loader.py` | Confirm source under test |

## Tests

No new tests are written by this task. The acceptance criterion is the exit code and
output of the two existing pytest invocations, which are documented in the Approach
section above.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| (verification) | mock_mode_collects_new_tests | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v` exits 0 and output includes `test_pipeline_cache.py` and `test_nodes_loader.py` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v` exits 0 with both test files in output |
| (verification) | real_mode_collects_new_tests | `python -m pytest worker/tests -v -m real_mode` exits 0 and output includes the LoadModel/LoadVae/LoadClip NotImplementedError real-mode tests | `python -m pytest worker/tests -v -m real_mode` exits 0 with NotImplementedError tests in output |

## CI Impact

No CI changes required. The existing `.github/workflows/ci.yml` `worker-test` job
already runs:
- Mock mode: `python -m pytest worker/tests -v -m "not real_mode"` (auto-collects new test files)
- Real mode: `python -m pytest worker/tests -v -m real_mode` (auto-collects new test files)

The `worker/tests` glob pattern automatically includes any `test_*.py` file in the
directory, so no CI file modification is needed for this phase's new test files.

## Platform Considerations

None identified. The verification commands are platform-neutral:
- `worker/tests` glob works identically on Linux and Windows.
- `ANVILML_WORKER_MOCK=1` environment variable is set identically on both platforms.
- The existing CI matrix already covers both `ubuntu-latest` and `windows-latest` for
  mock and real modes. No additional platform-specific handling is needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The mock-mode pytest run may fail due to an unrelated pre-existing test failure (not from this phase's files). | Low | Medium | If the run fails, check whether the failing tests are from this phase's files (`test_pipeline_cache.py`, `test_nodes_loader.py`) or from prior phases. If from prior phases, document the failure in the report but note that P19-E1's scope is limited to confirming collection/execution of the new test files. |
| The real-mode pytest run may fail because `torch` is not installed on the local environment. | Medium | Medium | The acceptance criterion uses `python -m pytest` which may use a system Python rather than the venv interpreter. If torch is missing, the real-mode tests will fail to import. In that case, the plan notes this as a known limitation — the CI job installs `cpu-runner-reqs.txt` before running real-mode tests, so CI will pass even if the local environment is not fully provisioned. |
| §9a.1 findings from P19-C3 (unmarked stubs) may block phase closure. | High | High | Documented in the Phase Deliverable Audit. The findings are pre-existing task-authoring defects from P19-C3. P19-E1 cannot fix them (no source changes in scope). The ACT agent for this task should surface the findings in the report and note that phase closure requires P19-C3's stubs to be given `defers_to:` comments. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v` exits 0 and output
      includes `test_pipeline_cache.py` and `test_nodes_loader.py`
- [ ] `python -m pytest worker/tests -v -m real_mode` exits 0 and output includes
      the LoadModel/LoadVae/LoadClip `NotImplementedError` real-mode tests
- [ ] `.github/workflows/ci.yml` confirmed to use `worker/tests` glob for both mock
      and real-mode pytest steps (no config change needed)
