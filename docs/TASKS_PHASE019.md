# Tasks: Phase 19 — Model Loading Contract Groundwork

**Phase:** 19
**Name:** Model Loading Contract Groundwork
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 6, 9, 10, 14

---

## Overview

This phase builds every piece of infrastructure the eventual real architecture
modules (ZiT, Qwen3, the ZiT VAE — Phase 20 onward) need before any of them can be
written: the scheduler-side hash-to-path resolution that lets a job submitter
reference a model by its registry hash rather than a raw filesystem path,
`pipeline_cache.py`'s LRU component cache, the three generic loader nodes'
mock-mode behavior and skeleton structure, and the fixture-checkpoint convention
documentation future arch-module tasks must follow. **No concrete arch module is
written in this phase** — `LoadModel`/`LoadVae`/`LoadClip`'s real branches
deliberately raise `NotImplementedError` until Phase 20 registers the first real
diffusion arch module.

This phase exists as its own distinct unit, separate from "ZiT Diffusion + Qwen3
CLIP + ZiT VAE," for the same reason "Generic Node Groundwork" (Phase 10) was kept
separate from "Dynamic Node System" (Phase 11): `ANVILML_DESIGN.md`'s phasing
discipline is explicit that groundwork closes before any concrete architecture
work begins, and conflating the two risks building the shared infrastructure
*around* one architecture's specific needs rather than architecture-agnostically.
Every piece this phase builds — the cache, the hash resolution, the loader node
shape — must work identically regardless of which architecture eventually fills in
the real branch.

At the start of this phase, job graphs can only reference models by raw path (no
resolution layer exists), there is no model/pipeline cache in the worker, and no
loader nodes exist at all. At the end: a job graph can reference a model by its
SHA256 hash and have it correctly resolved before dispatch; `pipeline_cache.py`
provides LRU-cached component loading; `LoadModel`/`LoadVae`/`LoadClip` exist with
correct mock-mode behavior and a documented, intentional real-mode placeholder; and
the fixture-checkpoint convention is written down for Phase 20 to follow.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Hash resolution | P19-A1 | Scheduler rewrites `model_id` hashes to filesystem paths before dispatch |
| B | Pipeline cache | P19-B1 | `pipeline_cache.py`'s LRU `get_or_load()` |
| C | Loader nodes | P19-C1 … P19-C3 | `LoadModel` (mock, then real-placeholder), then `LoadVae`/`LoadClip` |
| D | Fixture convention | P19-D1 | Documentation for Phase 20's fixture-checkpoint requirements |
| E | Verification | P19-E1 | Confirms existing CI wiring already covers this phase's new tests |

---

## Prerequisites

`ModelStore::get()` must exist per Phase 6 (P6-A3). `JobScheduler`'s dispatch path
must exist per Phase 14 (P14-A5). `worker_main.py`'s real/mock startup sequences and
`NodeContext`/`@register` must exist per Phases 9–10.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md` Appendix B.2 | P19-A1 | The exact hash-to-path rewrite behavior and the "worker never sees a hash" guarantee |
| `ANVILML_DESIGN.md §11.6` | P19-B1 | `pipeline_cache.get_or_load()`'s exact role — components only, never an assembled pipeline |
| `ANVILML_DESIGN.md §10.3` | P19-C1, P19-C2, P19-C3 | Exact `LoadModel`/`LoadVae`/`LoadClip` slot shapes |
| `ANVILML_DESIGN.md §10.6` | P19-C2, P19-C3 | The marker convention applies even to a deliberately-raising real branch |
| `ANVILML_DESIGN.md §17.5` | P19-D1 | Fixture-checkpoint conventions, including the mandatory metadata-fallback regression case |

---

## Task Descriptions

### Group A — Hash resolution

#### P19-A1: anvilml-scheduler: resolve model_id hashes to filesystem paths at dispatch

**Goal:** Implement the rewrite step that lets a job submitter reference a model
by its registry hash, while guaranteeing the Python worker never has to perform its
own hash lookup.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — extends `dispatch_one()`.

**Key implementation notes:**
- Only the **dispatched copy** sent over IPC is rewritten — the persisted
  `Job.graph` in `job_store` keeps the original hash, since that's what the
  submitter actually provided and what should be displayed back via
  `GET /v1/jobs/:id`.
- An unknown hash fails the job **before** any `Execute` message is sent — never
  dispatch a graph with an unresolved reference and let the worker discover the
  problem.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test scheduler_tests
# -> >=5 tests, exits 0
```

---

### Group B — Pipeline cache

#### P19-B1: worker/pipeline_cache.py: get_or_load() LRU component cache

**Goal:** Implement the cache every loader node will use to avoid redundant
reloads of the same component within one worker process's lifetime.

**Files to create or modify:**
- `worker/pipeline_cache.py` — new; `PipelineCache`.

**Key implementation notes:**
- This caches raw **components** (a transformer, a VAE, a text encoder) keyed by
  `model_id` — assembling a runnable pipeline from cached components is the
  diffusion arch module's `sample()` function's own responsibility, cached under a
  separate `f"{model_id}:pipeline"` key, per `ANVILML_DESIGN.md §11.6`. This module
  itself does not know or care about pipeline classes.
- Eviction only removes the dict entry — it does not explicitly free GPU memory;
  that's left to Python's refcounting and torch's own memory management.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_pipeline_cache.py -v
# -> >=6 tests, exits 0
```

---

### Group C — Loader nodes

#### P19-C1: worker/nodes/loader.py: LoadModel node, mock branch only

**Goal:** Create the first generic loader node with its mock branch fully
working, before the real branch (which deliberately can't do anything real yet) is
added.

**Files to create or modify:**
- `worker/nodes/loader.py` — new; `LoadModel`, mock branch only.

**Key implementation notes:**
- The real branch is a bare placeholder `raise NotImplementedError` for this task
  only — the next task completes it with the actual `pipeline_cache` call and the
  required markers.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> >=2 tests, exits 0
```

#### P19-C2: worker/nodes/loader.py: LoadModel real branch, deferred-raise + markers

**Goal:** Complete `LoadModel`'s real branch with the actual cache call and the
mandatory marker pair — even though the real branch currently raises by design.

**Files to create or modify:**
- `worker/nodes/loader.py` — completes `LoadModel`'s real branch.

**Key implementation notes:**
- The real branch deliberately raises `NotImplementedError("no diffusion arch
  module registered yet")` — this is intentional groundwork-phase behavior to
  record as a finding, not a defect to silently work around.
- Both `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers are still required per
  `ANVILML_DESIGN.md §10.6`, even though the real path currently raises — a test
  that asserts the expected `NotImplementedError` is itself a valid, collectible
  real-mode test, satisfying the marker requirement honestly rather than papering
  over an absent real path with no test at all.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> >=4 tests total in the file, exits 0
```

#### P19-C3: worker/nodes/loader.py: LoadVae, LoadClip node skeletons (mock-mode only)

**Goal:** Complete the loader node trio with the same mock/real-placeholder
pattern `LoadModel` established.

**Files to create or modify:**
- `worker/nodes/loader.py` — adds `LoadVae`, `LoadClip`.

**Key implementation notes:**
- Identical structure to `LoadModel` — only the `SlotType` and cache-key namespace
  differ per loader. Both get the same deferred-raise real branch and both required
  markers.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> >=12 tests total in the file, exits 0
```

---

### Group D — Fixture convention

#### P19-D1: worker/tests/fixtures/: fixture-checkpoint builder conventions doc

**Goal:** Document the fixture-checkpoint rules Phase 20's real arch-module work
must follow, before that work begins.

**Files to create or modify:**
- `worker/tests/fixtures/README.md` — new; documentation only.

**Key implementation notes:**
- No actual fixture files are created in this task — only the convention itself,
  including the mandatory regression case: at least one fixture per
  diffusion/CLIP/VAE family must have a non-recognizable key prefix and no `arch`
  metadata key, exercising the metadata-fallback path.

**Acceptance criterion:**
```bash
test -s worker/tests/fixtures/README.md
# -> file exists and is non-empty
```

---

### Group E — Verification

#### P19-E1: CI: worker-test job collects loader.py + pipeline_cache.py tests

**Goal:** Confirm — without any CI file edit — that this phase's new test files
are already picked up by the existing worker-test CI wiring from Phase 9.

**Files to create or modify:**
- None. Verification only.

**Key implementation notes:**
- Phase 9's P9-F1 already wired the full `worker/tests` suite for both mock and
  `real_mode` markers — this task confirms that wiring genuinely requires no
  changes, rather than assuming so without checking.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode
# -> both exit 0, output includes test_pipeline_cache.py and test_nodes_loader.py
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof: not applicable — this phase builds groundwork infrastructure
# (hash resolution, pipeline cache, loader node skeletons) with no concrete arch
# module to exercise it end-to-end yet. LoadModel/LoadVae/LoadClip's real
# branches deliberately raise NotImplementedError, by design, until Phase 20.
# The full test suite (scheduler_tests, test_pipeline_cache.py,
# test_nodes_loader.py) is the complete and sufficient proof of this phase's
# deliverable, per the narrow exemption in FORGE_TASK_AUTHORING_SPEC.md §9.
```

---

## Known Constraints and Gotchas

- **No concrete arch module exists after this phase** — `LoadModel`/`LoadVae`/
  `LoadClip`'s real branches all raise `NotImplementedError` by design. This is
  groundwork, not a regression, and Phase 20 is what closes the gap.
- The Python worker never performs its own hash-to-path lookup — by the time a
  loader node's `execute()` runs, `inputs["model_id"]` is already a real filesystem
  path, rewritten by the scheduler before dispatch. A loader node attempting its own
  hash resolution would be duplicating logic that belongs entirely on the Rust side.
- `pipeline_cache.py` caches raw components only — it has no awareness of
  `diffusers`/`transformers` pipeline classes. Assembling a runnable pipeline from
  cached components is the diffusion arch module's own `sample()` function's job,
  not this module's.
- The marker convention applies even to a deliberately-raising real branch — a test
  asserting the expected exception is a legitimate, collectible real-mode test, not
  a workaround to skip writing one.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 19 — Model Loading Contract Groundwork

**Capability proved:** Not applicable — this phase builds groundwork
infrastructure (model-hash resolution, the pipeline cache, loader node skeletons)
with no concrete arch module yet to exercise it end-to-end. See
`TASKS_PHASE019.md`'s Phase Acceptance Criteria for the full test-suite proof. The
first end-to-end real model load is Phase 20's scope.
```
