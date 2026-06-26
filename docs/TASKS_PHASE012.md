# Tasks: Phase 12 — Graph Validation

**Phase:** 12
**Name:** Graph Validation
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 11

---

## Overview

This phase implements `anvilml-scheduler`'s graph validator: the `ValidatedGraph`
newtype, the `GraphError` enum, and `validate_graph()`'s full six-check,
collect-all-errors validation pipeline — checked against the dynamic
`NodeTypeRegistry` Phase 11 wired up. No job queue, no dispatch loop, and no HTTP
handler exist yet; this phase is purely the validation logic in isolation, callable
and testable on its own before the scheduler phases that follow build job submission
on top of it.

This phase exists right after the Dynamic Node System (Phase 11) and before the Job
Queue (Phase 13) because validation needs a real, queryable node registry to check
node types and slot compatibility against — Phase 11 is what makes that registry
real rather than permanently empty. Validation also has to exist before job
submission can use it: `POST /v1/jobs` (a later phase) will call `validate_graph()`
before ever touching the queue, since only a `ValidatedGraph` may be enqueued, per
`ANVILML_DESIGN.md §12.3`.

At the start of this phase, `anvilml-scheduler` is an empty stub crate (Phase 1's
P1-B5). At the end, `validate_graph()` runs all six checks from
`ANVILML_DESIGN.md §12.3` — structural shape, duplicate IDs, unknown node types,
dangling edges, slot type compatibility, and cycle detection — collecting every
violation rather than stopping at the first, and the only way to construct a
`ValidatedGraph` outside the crate is a successful call into this function.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Validator | P12-A1 … P12-A6 | `ValidatedGraph`, `GraphError`, then `validate_graph()`'s six checks built up incrementally |
| B | Closeout | P12-B1 | `lib.rs` re-export pass, 80-line check |

---

## Prerequisites

`NodeTypeRegistry` must exist and be populated correctly per Phase 11 (P11-A1).
`anvilml-core` must export `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType`
exactly as defined in Phase 3 (P3-A7). `anvilml-scheduler` must exist as a buildable
stub crate per Phase 1's P1-B5.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §12.1` | P12-B1 | Module layout — `types.rs`, `dag.rs` |
| `ANVILML_DESIGN.md §12.3` | P12-A1 … P12-A6 | The exact six checks, in order, in collect-all-errors mode — never fail-fast |
| `ANVILML_DESIGN.md §5.6` | P12-A5 | `SlotType`'s `Any` variant disables type checking for that slot |

---

## Task Descriptions

### Group A — Validator

#### P12-A1: anvilml-scheduler: ValidatedGraph newtype (construction-gated)

**Goal:** Define the newtype that represents "a graph that has passed every
validation check," with no way to construct one except through a successful
validation call.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/types.rs` — `ValidatedGraph`.

**Key implementation notes:**
- The wrapped `serde_json::Value` field is `pub(crate)`, not `pub` — and no
  `From<serde_json::Value>` or other bypass exists. The only way code outside this
  crate obtains a `ValidatedGraph` is a successful `validate_graph()` call, which
  doesn't exist until P12-A3.
- `GraphError` is explicitly out of scope for this task — it's the very next one.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=2 tests, exits 0
```

#### P12-A2: anvilml-scheduler: GraphError enum, all 7 variants

**Goal:** Define every error this validator can produce, before any check that
produces one is implemented.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/types.rs` — adds `GraphError`.
- `crates/anvilml-scheduler/Cargo.toml` — adds `thiserror` if not already present.

**Key implementation notes:**
- Seven variants exactly: `NotAnObject`, `MissingNodesArray`, `DuplicateNodeId`,
  `UnknownNodeType`, `DanglingEdge`, `SlotTypeMismatch`, `CycleDetected` — each with
  a `#[error("...")]` `Display` message via `thiserror`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=6 tests total in the file, exits 0
```

#### P12-A3: anvilml-scheduler: validate_graph structural checks (1-2)

**Goal:** Implement the entry point function and its first two checks —
confirming the graph is even shaped like a graph before checking anything deeper
about its contents.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — `validate_graph()`, checks 1–2 only.

**Key implementation notes:**
- Collect-all-errors mode from the very first check onward: this function never
  short-circuits on the first violation found, except where a violation makes
  further checking structurally meaningless (a non-object root has no `"nodes"`
  array to check duplicates within, so that one case does return early).
- Checks 3–6 are explicitly deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=5 new tests, exits 0
```

#### P12-A4: anvilml-scheduler: validate_graph node-type + edge checks (3-4)

**Goal:** Extend the validator with the checks that actually consult the dynamic
node registry — the first checks in this phase that depend on Phase 11's wiring
being correct.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — adds checks 3–4.

**Key implementation notes:**
- Check 3 (`UnknownNodeType`) queries `registry.get(type_name)` synchronously — the
  registry is assumed already populated by the time validation runs; this function
  doesn't wait for or trigger population itself.
- Check 4 (`DanglingEdge`) requires **both** that the referenced node exists **and**
  that it declares the referenced output slot — a node that exists but doesn't have
  that output is still a dangling edge.
- This receives exactly the scope P12-A3 deferred.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=11 tests total in the file, exits 0
```

#### P12-A5: anvilml-scheduler: validate_graph slot-type-compat check (5)

**Goal:** Extend the validator with slot-type compatibility checking, the check
that actually uses `SlotType`'s semantic meaning rather than just structural
presence.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — adds check 5.

**Key implementation notes:**
- Compatibility is exact `SlotType` match, **or** either side is `SlotType::Any` —
  `Any` disables type checking for that slot entirely, per `ANVILML_DESIGN.md §5.6`.
- This check only runs on edges that already passed check 4 — an edge already
  flagged `DanglingEdge` has no resolved slot to compare types against, and must
  not also be reported here as a separate `SlotTypeMismatch`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=16 tests total in the file, exits 0
```

#### P12-A6: anvilml-scheduler: validate_graph cycle detection (6), Kahn's algorithm

**Goal:** Complete the validator with cycle detection, the final check, and close
the loop on `ValidatedGraph` construction — this is the one and only place in the
crate that ever constructs one.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — adds check 6 and the final
  `Ok(ValidatedGraph(...))` construction.

**Key implementation notes:**
- Uses Kahn's algorithm: compute in-degree per node, repeatedly remove zero-in-degree
  nodes, anything still remaining when the process terminates is part of a cycle.
  `CycleDetected` names **every** remaining node, not just one representative node.
- A graph that passes all six checks with zero collected errors is the only
  condition under which `Ok(ValidatedGraph(graph))` is returned.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test dag_tests
# -> >=21 tests total in the file, exits 0
```

---

### Group B — Closeout

#### P12-B1: anvilml-scheduler: lib.rs re-export pass, 80-line check

**Goal:** Finalize `anvilml-scheduler`'s public surface for this phase's work and
confirm `lib.rs` stays within the 80-line hard cap.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/lib.rs` — re-exports only.

**Key implementation notes:**
- Same pattern as every prior crate's closing `lib.rs` task — no implementation
  logic, re-export and line-count verification only.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-scheduler/src/lib.rs
# -> <=80
cargo test -p anvilml-scheduler
# -> exits 0, full crate suite
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof: not applicable — this phase implements a pure validation
# function with no HTTP handler, CLI subcommand, or other externally observable
# surface yet wired up. POST /v1/jobs (a later phase) is the eventual real
# consumer of validate_graph(). The full dag_tests.rs suite (21+ tests covering
# all six checks individually and in combination) is the complete and sufficient
# proof of this phase's deliverable, per the narrow exemption in
# FORGE_TASK_AUTHORING_SPEC.md §9.
```

---

## Known Constraints and Gotchas

- `validate_graph()` is collect-all-errors, never fail-fast, except for the one
  structural case (non-object root) where further checking is meaningless — don't
  generalize that one early-return into a pattern for other checks.
- `ValidatedGraph` has exactly one legitimate construction path: a successful
  `validate_graph()` call. No bypass constructor may ever be added, even for
  testing convenience — tests construct one the same way production code does.
- A `DanglingEdge` (check 4) and a `SlotTypeMismatch` (check 5) are mutually
  exclusive per edge — an edge already flagged as dangling is skipped by check 5,
  never double-reported.
- Cycle detection (check 6) must name every node remaining in the cycle, not a
  single representative node — this matters for an operator trying to actually fix
  a submitted graph.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 12 — Graph Validation

**Capability proved:** Not applicable — this phase implements `validate_graph()`
as a pure function with no HTTP handler or other externally observable surface
wired up yet. See `TASKS_PHASE012.md`'s Phase Acceptance Criteria for the full
test-suite proof. `POST /v1/jobs` (a later phase) is the eventual real consumer.
```
