# Tasks: Phase 3 — Core Domain Types: Data Model

**Phase:** 3
**Name:** Core Domain Types: Data Model
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2

---

## Overview

This phase completes `anvilml-core` by adding every remaining domain type the rest of
the system will pass around: jobs, models, artifacts, hardware, workers, node
descriptors, and WebSocket events, plus the dynamic `NodeTypeRegistry` that holds node
type definitions reported by the Python worker at runtime. None of these types do any
I/O — `anvilml-core` remains a pure-data crate through the end of this phase, exactly
as `ANVILML_DESIGN.md §5` requires.

This phase exists immediately after config/errors because every later crate's public
API is expressed in terms of these types: `anvilml-hardware`'s detectors return
`GpuDevice`/`HardwareInfo`; `anvilml-registry`'s scanner returns `ModelMeta`;
`anvilml-ipc`'s message enums embed `JobSettings` and reference `WorkerInfo`;
`anvilml-server`'s handlers serialize `Job`, `ArtifactMeta`, and `WsEvent` directly
into HTTP/WebSocket responses. Getting these types' field names, enum variants, and
serde representations exactly right now means no later phase needs to redefine or
rename anything `anvilml-core` already committed to.

At the start of this phase, `anvilml-core` has only `AnvilError` and `ServerConfig`
(Phase 2). At the end, it exports the complete type surface from
`ANVILML_DESIGN.md §5.3`–§5.6 and the events table, plus `NodeTypeRegistry`. Phase 4
(`anvilml-hardware`'s detectors) depends directly on this phase's `GpuDevice`,
`HardwareInfo`, `DeviceType`, `InferenceCaps`, `EnumerationSource`, and
`CapabilitySource` types existing exactly as specified here.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Domain types & registry | P3-A1 … P3-A11 | All of `anvilml-core/src/types/*`, plus `NodeTypeRegistry`, plus a final `lib.rs` cleanup pass |

A single group is used because every task in this phase operates on the same crate's
`types/` module in a strict, naturally-ordered build-up — splitting into multiple
group letters would not aid navigation for a phase this narrowly scoped to one
directory.

---

## Prerequisites

`anvilml-core` must have `AnvilError` (P2-A1) and the complete `ServerConfig` +
`config_load::load()` (P2-A7) already merged, since this phase's `lib.rs` re-export
ordering task (P3-A11) assumes both already exist alongside the new `types` module.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §5.3` | P3-A1 | `Job`, `JobStatus`, `JobSettings` field names and types |
| `ANVILML_DESIGN.md §5.4` | P3-A2 | `ModelMeta`, `ModelKind`/`ModelDtype`/`ModelFormat` variants and snake_case serde |
| `ANVILML_DESIGN.md §5.5` | P3-A4, P3-A5 | `HardwareInfo`, `GpuDevice`, `InferenceCaps`, `EnumerationSource`, `CapabilitySource` — exact fields and the hint-vs-authoritative distinction in doc comments |
| `ANVILML_DESIGN.md §5.6` | P3-A7 | `NodeTypeDescriptor`, `SlotDescriptor`, `SlotType` |
| `ANVILML_DESIGN.md` events table | P3-A8, P3-A9 | `WsEvent` variant list and field shapes |
| `ANVILML_DESIGN.md §10` | P3-A10 | `NodeTypeRegistry` is dynamic, populated only from worker `Ready` events — never a hardcoded list |

---

## Task Descriptions

### Group A — Domain types & registry

#### P3-A1: anvilml-core: Job, JobStatus, JobSettings types

**Goal:** Define the job submission and lifecycle types that the scheduler, the
worker IPC layer, and the HTTP API all reference as their shared vocabulary for "a
generation request and its current state."

**Files to create or modify:**
- `crates/anvilml-core/src/types/mod.rs` — new; declares the `types` submodule tree.
- `crates/anvilml-core/src/types/job.rs` — `Job`, `JobStatus`, `JobSettings`.
- `crates/anvilml-core/src/lib.rs` — adds `mod types; pub use types::*;`.

**Key implementation notes:**
- `Job.graph` is `serde_json::Value` — the submitted node graph is opaque to Rust;
  only the Python worker interprets its contents.
- `JobStatus` needs `Copy, PartialEq, Eq` in addition to the common derive set, since
  it's compared and matched on frequently in scheduler logic added in a later phase.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test job_tests
# -> >=4 tests, exits 0
```

#### P3-A2: anvilml-core: ModelMeta, ModelKind, ModelDtype, ModelFormat

**Goal:** Define the model-file metadata types the registry scanner will populate
and the scheduler/loader will read to decide which architecture module handles a
given file.

**Files to create or modify:**
- `crates/anvilml-core/src/types/model.rs` — `ModelMeta` and its three enums.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod model;`.

**Key implementation notes:**
- All three enums use `#[serde(rename_all = "snake_case")]` — e.g.
  `ModelKind::TextEncoder` serializes as `"text_encoder"`, matching what the HTTP API
  and the Python worker's own dict keys will expect.
- `ModelMeta` itself is not `Copy` (it owns a `PathBuf` and `String`s); only the three
  enums are `Copy`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test model_tests
# -> >=4 tests, exits 0
```

#### P3-A3: anvilml-core: ArtifactMeta type

**Goal:** Define the metadata record for a generated, content-addressed PNG
artifact, which both the scheduler and the server will read from
`anvilml-artifacts`' future store.

**Files to create or modify:**
- `crates/anvilml-core/src/types/artifact.rs` — `ArtifactMeta`.
- `crates/anvilml-core/Cargo.toml` — adds `utoipa`.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod artifact;`.

**Key implementation notes:**
- `hash: String` is the SHA-256 hex content address — this is the same identifier
  scheme `anvilml-artifacts`' future `ArtifactStore` will use as its primary key.
- `ToSchema` (from `utoipa`) is introduced in this task since `ArtifactMeta` is the
  first type that will appear directly in an OpenAPI-annotated HTTP response, once
  the server phase adds the `/v1/artifacts` handlers.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test artifact_tests
# -> >=3 tests, exits 0
```

#### P3-A4: anvilml-core: HardwareInfo, GpuDevice, DeviceType types

**Goal:** Define the first half of the hardware snapshot types — the structural
pieces (`HardwareInfo`, `GpuDevice`, `DeviceType`) that Phase 4's detectors will
construct and return.

**Files to create or modify:**
- `crates/anvilml-core/src/types/hardware.rs` — `HostInfo`, `HardwareInfo`,
  `GpuDevice`, `DeviceType`.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod hardware;`.

**Key implementation notes:**
- `HostInfo` is intentionally minimal (`hostname`, `os`) at this point — it exists
  so `HardwareInfo` has a complete shape now; no task in this phase populates it from
  a real host probe, that's Phase 4/5's concern.
- `GpuDevice.caps: InferenceCaps` and the two `*Source` enum fields are referenced
  here by name but defined in the very next task (P3-A5) — write this task assuming
  those names will exist by the time the crate is built as a whole (both land before
  any `cargo build` is required to succeed, since they're sequential tasks in the
  same session-by-session build-up).

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test hardware_tests
# -> >=4 tests, exits 0
```

#### P3-A5: anvilml-core: InferenceCaps, EnumerationSource, CapabilitySource

**Goal:** Complete the hardware type module with the capability and provenance
types that encode the design's central hint-vs-authoritative distinction
(`ANVILML_DESIGN.md §6.1`).

**Files to create or modify:**
- `crates/anvilml-core/src/types/hardware.rs` — adds `InferenceCaps`,
  `EnumerationSource`, `CapabilitySource`.

**Key implementation notes:**
- `InferenceCaps::default()` is all-`false` — a freshly constructed value claims no
  capability until something actually probes for it.
- `EnumerationSource` has **seven** variants per the amended `ANVILML_DESIGN.md
  §5.5`: `Vulkan, Dxgi, Sysfs, Nvml, Mock, Override, Cpu`. The `Cpu` variant is an
  addition (see `docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md`) marking a device that was
  *synthesized* by the unconditional CPU fallback rather than actually enumerated by
  any detection mechanism — distinct from all six other variants, which describe a
  real (or mocked) enumeration result.
- The doc comment on `CapabilitySource` must state explicitly that `PyTorch` is the
  only source an arch module's loader may use for a runtime dtype decision —
  `DeviceTable` and `Fallback` are pre-spawn hints for scheduling estimates only.
  This isn't decorative; it's the textual anchor a much later phase's code review
  will point back to.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test hardware_tests
# -> >=9 tests total in the file, exits 0
```

#### P3-A6: anvilml-core: WorkerInfo, WorkerStatus, EnvReport, ProvisioningState

**Goal:** Define the worker-process status types the scheduler's dispatch logic and
the `/v1/workers` and `/v1/system` HTTP handlers will both read.

**Files to create or modify:**
- `crates/anvilml-core/src/types/worker.rs` — `WorkerStatus`, `WorkerInfo`,
  `EnvReport`, `ProvisioningState`.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod worker;`.

**Key implementation notes:**
- `WorkerStatus` variants (`Spawning`, `Idle`, `Busy`, `Dying`, `Dead`) are the
  states a `ManagedWorker` cycles through across its lifecycle — defining them here,
  ahead of the worker-pool implementation phase, lets the dispatch-selection logic in
  the scheduler phase reference a stable enum from day one.
- `EnvReport` exists for the `/v1/system/env` endpoint's eventual response shape —
  it does not get populated by any code in this phase.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test worker_tests
# -> >=4 tests, exits 0
```

#### P3-A7: anvilml-core: NodeTypeDescriptor, SlotDescriptor, SlotType

**Goal:** Define the type describing a node's shape (its name, category, and typed
input/output slots) as reported by the Python worker — the building block
`NodeTypeRegistry` (P3-A10) will store.

**Files to create or modify:**
- `crates/anvilml-core/src/types/node.rs` — `NodeTypeDescriptor`, `SlotDescriptor`,
  `SlotType`.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod node;`.

**Key implementation notes:**
- `SlotType` is a **fixed, closed** enum per `ANVILML_DESIGN.md §5.6` — exactly
  eleven variants (`Model`, `Clip`, `Vae`, `Conditioning`, `Latent`, `Image`,
  `String`, `Int`, `Float`, `Bool`, `Any`), with `#[serde(rename_all =
  "SCREAMING_SNAKE_CASE")]`. There is **no** `Other`/fallback variant — the design
  doc treats this list as complete for the MVP's needs; a future slot kind is added
  by extending this enum directly in a dedicated task, not by routing through a
  catch-all.
- `Any` disables type checking for that slot entirely — this is what the graph
  validator (a later phase) checks for when deciding whether two connected slots
  are compatible.
- `SlotDescriptor.optional: bool` is what lets a node input be omitted in favor of a
  node-internal default, per the generic node system design.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test node_tests
# -> >=4 tests, exits 0
```

#### P3-A8: anvilml-core: WsEvent job-lifecycle variants

**Goal:** Define the job-related half of the `WsEvent` enum — the events a client
subscribed to `/v1/events` receives as a submitted job moves through the queue,
executes, and completes.

**Files to create or modify:**
- `crates/anvilml-core/src/types/events.rs` — new; `WsEvent` enum with only the
  seven job-lifecycle variants.
- `crates/anvilml-core/src/types/mod.rs` — adds `mod events;`.

**Key implementation notes:**
- `#[serde(tag = "type", rename_all = "snake_case")]` on the enum — every variant
  serializes with a `"type"` discriminator key, e.g. `JobQueued` → `{"type":
  "job_queued", ...}`.
- The remaining three variants (`WorkerStatusChanged`, `SystemStats`,
  `ProvisioningProgress`) are explicitly out of scope for this task — they are
  P3-A9's deferred scope, since they depend on `WorkerInfo`/`WorkerStatus` from
  P3-A6 and keeping all ten variants in one task would mix two distinct concerns
  (job events vs. system/worker events) in a single change.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test events_tests
# -> >=7 tests, exits 0
```

#### P3-A9: anvilml-core: WsEvent worker/system/provisioning variants

**Goal:** Complete the `WsEvent` enum with the system-level event variants,
finishing the full event vocabulary the WebSocket broadcaster will emit.

**Files to create or modify:**
- `crates/anvilml-core/src/types/events.rs` — adds `WorkerStatusChanged`,
  `SystemStats`, `ProvisioningProgress`.

**Key implementation notes:**
- `SystemStats.workers: Vec<WorkerInfo>` and `WorkerStatusChanged.status:
  WorkerStatus` both import from `types::worker` (P3-A6) — do not redefine either
  type locally in `events.rs`.
- This task receives exactly the scope P3-A8 deferred; confirm P3-A8's `WsEvent` has
  precisely the seven job variants, no more, before extending it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test events_tests
# -> >=10 tests total in the file, exits 0
```

#### P3-A10: anvilml-core: NodeTypeRegistry dynamic registry

**Goal:** Implement the in-memory registry that holds the set of node types
currently known to the system, populated entirely at runtime from a worker's
`Ready` event — never from a compiled-in list.

**Files to create or modify:**
- `crates/anvilml-core/src/node_registry.rs` — `NodeTypeRegistry`.
- `crates/anvilml-core/src/lib.rs` — adds `mod node_registry; pub use
  node_registry::NodeTypeRegistry;`.

**Key implementation notes:**
- `register_all()` **replaces** the entire contents on each call, it does not merge
  — a worker's `Ready` event reports its complete current node set every time, and
  the registry must reflect exactly that, not an accumulation across multiple
  workers or restarts.
- Internally an `RwLock<HashMap<String, NodeTypeDescriptor>>` keyed by `type_name`;
  `get()`/`list()` take only the read lock, so concurrent reads never block each
  other, and a concurrent read during a `register_all()` write must not deadlock —
  this is one of the mandatory test cases.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test node_registry_tests
# -> >=5 tests, exits 0
```

#### P3-A11: anvilml-core: lib.rs final re-export pass, 80-line check

**Goal:** Tidy `anvilml-core/src/lib.rs` into a clean, alphabetically-ordered
re-export list now that all of Phase 2 and Phase 3's modules exist, and confirm it
still respects the 80-line hard cap before this phase closes.

**Files to create or modify:**
- `crates/anvilml-core/src/lib.rs` — reorder only; no new declarations.

**Key implementation notes:**
- This is a pure reordering/tidying pass — no new types, no new logic. Tagged
  `refactor` since it makes zero observable behaviour change; per
  `FORGE_AGENT_RULES.md §4.6`, confirm no `pub` signature changed before writing the
  implementation report.
- If the file is anywhere near 80 lines, that is itself a signal something
  drifted into `lib.rs` that shouldn't have (implementation code, accidentally) —
  investigate rather than assuming the cap needs an exception.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-core/src/lib.rs
# -> <=80
cargo test -p anvilml-core
# -> exits 0, no regressions
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof: not applicable — this phase adds only pure data types and an
# in-memory registry to anvilml-core, with no new HTTP endpoint, WebSocket event,
# CLI flag, or file-on-disk artifact a human or script could observe from outside
# the test process. The full anvilml-core test suite (job_tests, model_tests,
# artifact_tests, hardware_tests, worker_tests, node_tests, events_tests,
# node_registry_tests) is the complete and sufficient proof of this phase's
# deliverable, per the narrow exemption in FORGE_TASK_AUTHORING_SPEC.md §9.
```

---

## Known Constraints and Gotchas

- `types/mod.rs` accumulates one `mod <name>;` declaration per task in this phase —
  each task adds exactly one line there, never restructuring the file as a whole.
- `GpuDevice` (P3-A4) references `InferenceCaps`, `EnumerationSource`, and
  `CapabilitySource` by name before they're defined in the same file by the very next
  task (P3-A5) — this is expected and intentional sequencing, not a forward-reference
  error, since both tasks land in the same `hardware.rs` file before any build is
  required to succeed independently between them.
- `WsEvent`'s ten variants are deliberately split across two tasks (P3-A8/P3-A9) to
  keep each task's `context` field under the 1000-character cap and each task
  focused on one coherent sub-concern (job events vs. system events).

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 3 — Core Domain Types: Data Model

**Capability proved:** Not applicable — this phase adds only pure data types and an
in-memory registry to `anvilml-core`, with no new externally observable behaviour.
See `TASKS_PHASE003.md`'s Phase Acceptance Criteria for the full test-suite proof.
```
