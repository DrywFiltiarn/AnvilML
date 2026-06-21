# Tasks: Phase 903 — Pipeline Cache & Model Path Resolution Retrofit

| Field | Value |
|-------|-------|
| Phase | 903 |
| Name | Pipeline Cache & Model Path Resolution Retrofit |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 18 (after P18-D1, before any P18-D2+ real-path task) |

## Overview

Phase 903 is a two-task retrofit correcting two independent wiring gaps that
block every real (non-mock) node implementation in Phase 18 from Phase 18
group D onward.

**Gap 1 (Rust-side, newly discovered):** `model_id` values submitted in job
graphs are SHA256 hex digests of the first 1 MiB of a model file
(`anvilml-registry`'s `ModelScanner` convention, `ANVILML_DESIGN.md §7.2`).
No mechanism exists today to resolve that hash back to a filesystem path
inside the Python worker process. `WorkerMessage::Execute` carries only
`job_id`, `graph` (opaque JSON), `settings`, `device_index` — no model
directory information, and `build_worker_env()` forwards no model-path env
var. Without this, `LoadModel`/`LoadVae`/`LoadClip`'s real paths cannot open
any file regardless of their own implementation correctness.

**Gap 2 (Python-side, predates Phase 18):** `worker/worker_main.py`
(introduced in P14-A2, before `worker/pipeline_cache.py` existed) constructs
`NodeContext(..., pipeline_cache={})` — a bare empty dict. P18-C1 later
implemented the real `PipelineCache` class (LRU eviction, OOM retry, fully
tested) but no task ever revisited `worker_main.py` to instantiate it. Every
real loader/arch path that calls `ctx.pipeline_cache.get_or_load(...)` per
the design doc will fail with `AttributeError: 'dict' object has no
attribute 'get_or_load'` until this is fixed.

Both defects predate the current execution frontier (P18-D1, complete) and
are corrected here as new leaf tasks per the retrofit-leaf pattern — neither
requires editing any already-executed task's files beyond the two specific
wiring points described below.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | retrofit | P903-A1 … P903-A2 | model_id resolution (Rust) + pipeline_cache wiring (Python) |

## Prerequisites

Phase 18 group C complete (P18-C1: `pipeline_cache.py` exists and is
tested). Phase 18 group D1 complete (`worker/nodes/arch/zit.py` exists with
mock path only). `anvilml-registry::ModelStore` exists with a `get(id) ->
Option<ModelMeta>`-shaped lookup (Phase 6/7).

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §7.2` | P903-A1 | Model ID is lower-case hex SHA256 of first 1 MiB |
| `crates/anvilml-ipc/src/messages.rs` | P903-A1 | `WorkerMessage::Execute` field shape is NOT changed — graph JSON is rewritten in place before encoding, no new IPC fields |
| `worker/nodes/base.py` | P903-A2 | `NodeContext.pipeline_cache` type becomes `PipelineCache` in practice; the declared type hint (`dict[str, Any]`) is intentionally left unchanged since `PipelineCache` is duck-type compatible for the one method nodes call (`get_or_load`) — no signature change needed |

## Task Descriptions

### Group A — Retrofit

#### P903-A1: anvilml-scheduler: resolve model_id hash to filesystem path at dispatch time

**Goal:** Before any `WorkerMessage::Execute` is sent, rewrite every
`LoadModel`/`LoadVae`/`LoadClip` node's `inputs.model_id` in the job graph
from its submitted SHA256 hash to the resolved absolute filesystem path,
looked up via `ModelStore`. If a hash cannot be resolved, the job is marked
`Failed` with a clear, actionable error and `Execute` is never sent for that
job — the Python worker never sees an unresolved hash.

**Rationale for dispatch-time (not submit-time) resolution:** resolving at
dispatch keeps the lookup against current `ModelStore` state at the moment
of execution, avoiding a stale-resolution race if a model is rescanned or
removed between job submission and dispatch.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features
mock-hardware` exits 0 with all existing tests passing plus ≥ 3 new tests
in `tests/model_resolve_tests.rs`.

#### P903-A2: worker/worker_main.py: wire real PipelineCache into NodeContext

**Goal:** Replace the `pipeline_cache={}` placeholder with a single real
`PipelineCache` instance, created once per worker process and reused across
every job's `NodeContext`, so that cache entries (loaded model components)
persist across jobs on the same worker rather than being discarded and
reloaded from disk on every single job.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_worker_main.py -v` exits 0, same test count as
before plus one new test verifying instance reuse across two sequential
Execute messages.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `model_store` field; rewrite graph `model_id` values before dispatch; fail job on unresolved hash |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `anvilml-registry` path dependency if not already present |
| CREATE | `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | ≥ 3 tests: resolves known id, fails unknown id without sending Execute, non-loader node inputs untouched |
| MODIFY | `worker/worker_main.py` | Instantiate one real `PipelineCache`; pass into every `NodeContext` |
| MODIFY | `worker/tests/test_worker_main.py` | Add instance-reuse test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_resolves_known_model_id` | A `LoadModel` node's `model_id` is rewritten from hash to path before dispatch | `ModelStore` seeded with one known `ModelMeta` | Job graph with `LoadModel{model_id: "<known-hash>"}` | Dispatched `Execute.graph` contains the resolved path, not the hash | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_unknown_model_id_fails_job_without_dispatch` | An unresolvable hash fails the job and never sends Execute | Empty `ModelStore` | Job graph with `LoadModel{model_id: "<unknown-hash>"}` | Job status becomes `Failed`; no `Execute` sent to worker | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_non_loader_node_inputs_untouched` | Nodes other than LoadModel/LoadVae/LoadClip are not rewritten | Job graph with a `Sampler` node carrying a `seed` input that happens to look hash-like | Job graph with mixed node types | `Sampler.inputs` unchanged after rewrite pass | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |
| `worker/tests/test_worker_main.py` | `test_pipeline_cache_reused_across_jobs` | The same `PipelineCache` instance is passed to `NodeContext` for two sequential jobs | Worker process handling two Execute messages in sequence | Two Execute messages, same worker process | `id(ctx1.pipeline_cache) == id(ctx2.pipeline_cache)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 |

## CI Impact

No new CI jobs. `rust-linux`/`rust-windows` matrix runs the new
`model_resolve_tests.rs` automatically (standard `cargo test --workspace`
discovery). `worker-linux`/`worker-windows` run the updated
`test_worker_main.py` automatically (standard `pytest worker/tests/`
discovery).

## Platform Considerations

None identified. Both changes use only existing cross-platform mechanisms
(`SqlitePool` query already used elsewhere in the scheduler; Python
`PipelineCache` already proven mock-safe without torch).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Graph rewrite must not mutate node `inputs` for node types it doesn't recognise (forward compatibility with future loader node types) | Low | Medium | Rewrite pass matches only the three known `type` strings (`LoadModel`, `LoadVae`, `LoadClip`) exactly; all other node types pass through the JSON walk untouched. |
| `ModelStore::get` signature may differ slightly from the assumed `Option<ModelMeta>` return shape | Low | Low | At ACT time, the acting agent must confirm the exact method name and return type via `project_knowledge_search`/source inspection before writing the lookup call — do not assume the signature from this task description alone. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0
- [ ] `grep -n "pipeline_cache={}" worker/worker_main.py` returns no hits
