# Tasks: Phase 10 — Generic Node Groundwork

**Phase:** 10
**Name:** Generic Node Groundwork
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9

---

## Overview

This phase builds the generic node system's scaffolding — `BaseNode`, the
`@register` decorator, `SlotSpec`, `NodeContext`, and the three architecture-family
dispatch packages (`arch/diffusion/`, `arch/clip/`, `arch/vae/`) with their shared
`can_handle()`/`get_module()` scan logic — with **no concrete node types and no
concrete arch modules yet**. There is no `LoadModel`, no `Sampler`, no `zit.py`. This
phase proves the dispatch mechanism works correctly with zero registered modules
before any real node or architecture is added on top of it.

This phase exists, scoped exactly this narrowly, because `ANVILML_DESIGN.md §20`'s
roadmap explicitly separates "Generic Node Groundwork" (this phase) from "Dynamic
Node System" (where the worker actually reports real node types and `/v1/nodes` goes
live) and from the architecture-specific phases much later (ZiT, Flux 2 Klein). The
roadmap's own description of this phase states its tests cover "dispatch-with-zero-
modules-registered and dispatch-with-one-stub-module — not yet a real checkpoint,
since no arch module exists yet to load one." Building concrete nodes or arch modules
here would be scope creep into phases that haven't been authored yet.

At the start of this phase, `worker/nodes/` does not exist. At the end:
`worker/nodes/base.py` has a complete, tested `BaseNode`/`@register`/`SlotSpec`/
`NodeContext` contract; all three `arch/*/`'s dispatch packages exist with correctly
empty registries; `worker/nodes/__init__.py` can auto-import the (currently empty)
set of node files without error; and `worker_main.py`'s `_import_nodes()` is wired to
this real (if still empty) machinery instead of Phase 9's placeholder stub. Later
phases add concrete node files and concrete arch modules on top of this groundwork
without needing to touch any of it.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Base contract | P10-A1 … P10-A4 | `SlotSpec`, `NODE_REGISTRY`, `@register`, `NodeContext`, `BaseNode` |
| B | Arch dispatch | P10-B1 … P10-B2 | The shared `get_module()` scan logic for `diffusion/`, `clip/`, `vae/` |
| C | Auto-import | P10-C1 | `worker/nodes/__init__.py`'s import-triggers-registration wiring |
| D | Wiring | P10-D1 | Connects `worker_main.py`'s `_import_nodes()` to the real (empty) machinery |
| E | Documentation | P10-E1 | A short pointer doc for the marker convention future node-authoring phases will use |

---

## Prerequisites

`worker/worker_main.py` must have both its real-mode and mock-mode startup
sequences complete and passing per Phase 9 (P9-D2, P9-D3), including the
`_import_nodes()` stub this phase replaces.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §14.5` | P10-A1 … P10-A4 | `SlotSpec`, `NODE_REGISTRY`, `register()`, `NodeContext`'s docstring and signature — normative, copy verbatim |
| `ANVILML_DESIGN.md §10.4` | P10-B1, P10-B2 | `get_module()` is the **one** shared dispatcher per family — never three divergent implementations |
| `ANVILML_DESIGN.md §10.2` | P10-C1 | Node registration happens via import side-effect, triggered by the worker's auto-import |
| `ANVILML_DESIGN.md §14.3` | P10-D1 | Node import is identical code in both real and mock modes |
| `ANVILML_DESIGN.md §10.6` | P10-E1 | The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker convention — documented here for future use, not yet exercised |

---

## Task Descriptions

### Group A — Base contract

#### P10-A1: worker/nodes/base.py: SlotSpec dataclass + NODE_REGISTRY dict

**Goal:** Create the node package and its two most basic pieces — the slot
description dataclass and the global registry dict every node will eventually
populate.

**Files to create or modify:**
- `worker/nodes/__init__.py` — new, empty for now.
- `worker/nodes/base.py` — new; `NODE_REGISTRY`, `SlotSpec`.

**Key implementation notes:**
- `ANVILML_DESIGN.md §14.5`'s module docstring and `SlotSpec` definition are
  normative — copy them verbatim rather than paraphrasing.
- No `@register` decorator or `NodeContext` yet — both are separate, later tasks in
  this same phase.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_base.py -v
# -> >=3 tests, exits 0
```

#### P10-A2: worker/nodes/base.py: @register decorator with required-attr validation

**Goal:** Implement the decorator that turns a class definition into a registry
entry, with validation that catches a malformed node class at definition time
rather than at first use.

**Files to create or modify:**
- `worker/nodes/base.py` — adds `register()`.

**Key implementation notes:**
- Checks all six required attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`,
  `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`) via `hasattr`, raising `TypeError`
  naming the specific missing attribute — not a generic failure message.
- Returns the decorated class unchanged; this is registration only, never a wrapper
  or proxy around the class.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_base.py -v
# -> >=8 tests total in the file, exits 0
```

#### P10-A3: worker/nodes/base.py: NodeContext runtime context class

**Goal:** Implement the runtime context object every future node's `execute()`
method will receive, carrying everything a node needs without requiring it to reach
into global state.

**Files to create or modify:**
- `worker/nodes/base.py` — adds `NodeContext`.

**Key implementation notes:**
- All seven attributes (`job_id`, `device`, `caps`, `cancel_flag`, `emit`,
  `pipeline_cache`, `mock`) are assigned directly with no validation or
  transformation — `ANVILML_DESIGN.md §14.5`'s docstring is normative here too.
- `caps` is documented as the source every future arch module's dtype decision must
  read from — never a Rust-side hint — though this is purely documentation at this
  phase, since no arch module exists yet to actually enforce it against.
- `mock: bool` is the flag every future node's `execute()` will branch on exactly
  once, at the top of the method — this phase doesn't yet have a node to demonstrate
  that pattern with, but the field exists now so later phases don't need to revisit
  this class's shape.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_base.py -v
# -> >=12 tests total in the file, exits 0
```

#### P10-A4: worker/nodes/base.py: BaseNode ABC abstract execute()

**Goal:** Complete the base contract with the abstract base class every concrete
node will subclass, closing out Group A.

**Files to create or modify:**
- `worker/nodes/base.py` — adds `BaseNode(ABC)`.

**Key implementation notes:**
- `BaseNode` itself carries no `NODE_TYPE`/`CATEGORY`/etc. class attributes — those
  come from each concrete subclass and are validated by `@register` (P10-A2) at
  class-definition time, not by `BaseNode` itself.
- Standard Python `ABC` semantics already prevent direct instantiation and enforce
  that a subclass implements `execute()` — no additional enforcement code is needed
  in this task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_base.py -v
# -> >=15 tests total in the file, exits 0
```

---

### Group B — Arch dispatch

#### P10-B1: worker/nodes/arch/diffusion/__init__.py: can_handle/get_module dispatch

**Goal:** Implement the shared dispatch mechanism for the diffusion architecture
family — the single `get_module()` function every concrete diffusion arch module
(none of which exist yet) will eventually be discovered through.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/__init__.py` — `get_module()`.

**Key implementation notes:**
- `get_module()` is the **one shared dispatcher** for this family — `can_handle()`
  is never defined at the package level itself; each concrete module (added in a
  much later phase) defines its own.
- With an empty `_REGISTERED_MODULES` list, `get_module()` must return `None` for
  any key, and must never raise — this is the "zero modules registered" case the
  roadmap's own phase description calls out as a required test.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_dispatch.py -v
# -> >=3 tests, exits 0
```

#### P10-B2: worker/nodes/arch/clip/__init__.py and arch/vae/__init__.py: same dispatch

**Goal:** Replicate the exact same shared-scan-logic pattern for the two remaining
architecture families, rather than writing three independently-evolving
implementations of the same dispatch logic.

**Files to create or modify:**
- `worker/nodes/arch/clip/__init__.py`, `worker/nodes/arch/vae/__init__.py` — both
  get the same `get_module()` shape as `diffusion/__init__.py`.

**Key implementation notes:**
- `ANVILML_DESIGN.md §10.4` is explicit: "do not write three separate iteration
  implementations." Copy P10-B1's structure into both files rather than
  reimplementing the scan loop from scratch.
- Both start with an empty `_REGISTERED_MODULES` list — `qwen3.py`, `zit_vae.py`,
  `flux2_vae.py` and so on are all out of this phase's scope entirely.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_dispatch.py -v
# -> >=9 tests total in the file, exits 0
```

---

### Group C — Auto-import

#### P10-C1: worker/nodes/__init__.py: auto-import wiring for nodes/ submodules

**Goal:** Implement the import-triggers-registration mechanism the worker will use
at startup to populate `NODE_REGISTRY`, even though there's nothing to register yet.

**Files to create or modify:**
- `worker/nodes/__init__.py` — adds the auto-import loop.

**Key implementation notes:**
- Iterates `.py` files directly under `worker/nodes/` only — **not** recursively
  into `arch/`, since arch modules are imported by their own family's dispatcher
  (Group B), not by this top-level loop.
- At this phase, no concrete node files exist, so `NODE_REGISTRY` is correctly
  **empty** immediately after import — this is the expected state, not a bug to
  chase down.
- Re-importing must be idempotent — no duplicate-registration error on a second
  import.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_init.py -v
# -> >=3 tests, exits 0
```

---

### Group D — Wiring

#### P10-D1: worker_main.py: wire real _import_nodes() to worker.nodes auto-import

**Goal:** Replace Phase 9's `_import_nodes()` stub (which returned a hardcoded empty
list) with a real call into this phase's auto-import machinery — closing the loop
between `worker_main.py` and the node system, even though the practical result
(an empty list) doesn't change yet.

**Files to create or modify:**
- `worker/worker_main.py` — `_import_nodes()` now calls the real auto-import.

**Key implementation notes:**
- Both the real-mode (Phase 9's P9-D2) and mock-mode (P9-D3) call sites are updated
  identically — node import is the same code in both modes, per
  `ANVILML_DESIGN.md §14.3`.
- The observable result (`node_types` is an empty list) doesn't change from Phase 9
  — what changes is that it's now derived from a real registry rather than a
  hardcoded literal, which matters once concrete node files start appearing in
  later phases.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_worker_main.py -v -m real_mode
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> both exit 0, all pre-existing tests from Phase 9 still pass
```

---

### Group E — Documentation

#### P10-E1: FORGE_TASK_AUTHORING_SPEC.md-style marker convention doc note

**Goal:** Leave a short, pointer-style reference for the mock/real parity marker
convention inside `worker/nodes/`, so the next phase's node-authoring tasks have a
concrete example close at hand without duplicating the full rule.

**Files to create or modify:**
- `worker/nodes/MARKER_CONVENTION.md` — new; documentation only.

**Key implementation notes:**
- This is a **pointer**, not a duplicate of `ANVILML_DESIGN.md §10.6`'s full rule —
  keeping the two documents from drifting apart means this file states the marker
  pair's exact comment format and a one-line note that it's mechanically checked by
  a CI gate, and nothing more.
- No code changes in this task.

**Acceptance criterion:**
```bash
test -s worker/nodes/MARKER_CONVENTION.md
# -> file exists and is non-empty
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
python -m py_compile $(git ls-files 'worker/*.py')
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof: not applicable — this phase builds the node system's base
# contract and dispatch scaffolding with zero concrete node types and zero
# concrete arch modules. There is nothing for an external client to observe
# beyond what Phase 9's existing real_startup_tests.rs already demonstrates
# (a Ready event with an empty node_types list, now derived from a real registry
# instead of a hardcoded literal — not an externally distinguishable change). The
# full test suite (test_base.py, test_arch_dispatch.py, test_nodes_init.py, and
# the updated test_worker_main.py assertions) is the complete and sufficient
# proof of this phase's deliverable, per the narrow exemption in
# FORGE_TASK_AUTHORING_SPEC.md §9. The "Dynamic Node System" phase (later in the
# roadmap) is where node_types first becomes non-empty and observably different.
```

---

## Known Constraints and Gotchas

- **No concrete node types and no concrete arch modules belong in this phase** —
  `LoadModel`, `Sampler`, `zit.py`, `qwen3.py`, and every other concrete file named
  in `ANVILML_DESIGN.md §10.3`/§14 is explicitly out of scope here, reserved for
  later, separately-authored phases.
- `get_module()` must be the **one** shared implementation per architecture family —
  `diffusion/`, `clip/`, and `vae/`'s `__init__.py` files must not diverge into three
  independently-written scan loops, even though each lives in its own file.
- The auto-import loop in `worker/nodes/__init__.py` scans only the top-level
  `nodes/` directory, never recursing into `arch/` — arch modules are discovered by
  their family's own dispatcher, a structurally different mechanism.
- `worker/nodes/MARKER_CONVENTION.md` is a pointer, not a second copy of
  `ANVILML_DESIGN.md §10.6`'s full rule — keep it short and let the design doc
  remain the single source of truth for the rule itself.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 10 — Generic Node Groundwork

**Capability proved:** Not applicable — this phase builds the node system's base
contract (`BaseNode`, `@register`, `SlotSpec`, `NodeContext`) and the three
architecture-family dispatch packages with zero concrete nodes or arch modules
registered. See `TASKS_PHASE010.md`'s Phase Acceptance Criteria for the full
test-suite proof. The first externally observable change to `node_types` (moving
from empty to populated) occurs in the later "Dynamic Node System" phase.
```
