# Tasks: Phase 902 — ArtifactStore Relocation Retrofit

| Field | Value |
|-------|-------|
| Phase | 902 |
| Name | ArtifactStore Relocation Retrofit |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 15 (after P15-A3, before P16-A1) |

## Overview

Phase 902 is a five-task retrofit correcting a layering defect introduced in P15-A2.

P15-A1 (approved plan) placed `ArtifactStore` in `crates/anvilml-server/src/artifact/store.rs`.
P15-A2's task context only asked for `Arc<ArtifactStore>` to be added as a field on
`JobScheduler` and `AppState` — i.e. constructor injection, with `anvilml-server`
remaining the owner. The implementing agent instead relocated the entire module into
`crates/anvilml-ipc/src/artifact_store.rs` and justified this in both the implementation
report and the module's own doc comment by claiming a dependency cycle: "the scheduler
cannot depend on the server."

**This claim is false.** The actual workspace dependency graph (verified directly from
`Cargo.toml` in every crate, June 2026) is:

```
anvilml-server  -->  anvilml-scheduler
anvilml-scheduler  (no dependency on anvilml-server, direct or transitive)
```

`anvilml-scheduler/Cargo.toml` has never listed `anvilml-server` as a dependency, at any
point in the project's history. There was no cycle to break. Had P15-A2 been implemented
as specified — `anvilml-server` constructs `ArtifactStore` and passes `Arc<ArtifactStore>`
into the `JobScheduler` constructor — no relocation would have been necessary at all.

Despite the rationale being wrong, the *symptom* it was reacting to deserves a real fix
rather than a pure revert: `anvilml-scheduler` does genuinely need read/write access to
artifact storage (to persist `ImageReady` payloads), and constructor injection from
`anvilml-server` down into `anvilml-scheduler` works today but is architecturally fragile
long-term — it means `anvilml-scheduler` cannot be tested or used standalone without first
constructing an `anvilml-server`-owned object. The correct destination is a dedicated
`anvilml-artifacts` crate, depended on independently by both `anvilml-scheduler` and
`anvilml-server`, with neither depending on the other for it. This is the same pattern
already used for `anvilml-registry` (model metadata persistence, depended on by both
`anvilml-worker` and `anvilml-scheduler` without either owning the other's copy).

`anvilml-ipc`'s `ws` module (`EventBroadcaster`) is explicitly **not** in scope for this
retrofit. Its own cycle rationale — `anvilml-worker` needs it, but `anvilml-server`
transitively depends on `anvilml-worker` — is real and verified independently; placing
`EventBroadcaster` in `anvilml-ipc` is the correct fix for that actual cycle. Only
`artifact_store.rs` is being moved.

**P902-A1** creates `anvilml-artifacts` and moves `store.rs` into it unchanged, fixing
only its module-doc rationale (see "Doc correction text" below).

**P902-A2** removes `artifact_store.rs` and its now-dead dependencies (`chrono`, `sha2`,
`sqlx`, `base64` — `base64` was added during P15-A2 but is in fact only used by
`anvilml-scheduler`'s `event_loop.rs`, which already declares it independently) from
`anvilml-ipc`, restoring the crate's doc-comment claim that it contains no business logic.

**P902-A3** and **P902-A4** repoint every call site (15 files total) from
`anvilml_ipc::ArtifactStore` to `anvilml_artifacts::ArtifactStore`, split by crate
(`anvilml-scheduler` vs. `anvilml-server`/`backend`) to keep each task atomic and
independently revertable. Both retain their `anvilml-ipc` dependency for `EventBroadcaster`
and the `WorkerEvent`/`WorkerMessage` types — only the `ArtifactStore` import moves.

**P902-A5** fixes the one remaining false claim that lives in source code rather than
documentation: `crates/anvilml-server/src/lib.rs`'s crate-level doc comment still states
that `anvilml-server` "owns... the artifact store." This is a code-comment correction,
not a docs correction — see "Manual documentation correction" below for why the original
A5/A6 split collapsed into this single task.

## Manual documentation correction (completed outside this task list)

`docs/ARCHITECTURE.md`, `docs/ANVILML_DESIGN.md`, and `README.md` were corrected manually
by the project owner on 2026-06-20, ahead of and independently from this task list's
execution — commit `5cb6a8b`, "docs: update documents in order to align with P902 retrofit
phase." That commit already applies everything the original draft of P902-A5 and half of
P902-A6 would have done: the `anvilml-artifacts` crate-tree entries in both documents'
dependency-graph diagrams and repository-layout trees, both documents' crate-responsibility
tables (`ARCHITECTURE.md §4`, `ANVILML_DESIGN.md §3.3`), removal of the stale
`anvilml-server/src/artifact/` module-layout block in `ANVILML_DESIGN.md §12.1`, and
removal of `ArtifactStore` from `anvilml-ipc`'s entry in both trees.

Because of this, the task originally numbered P902-A5 (a pure-docs task) is **deleted** —
running it would find nothing left to change. The task originally numbered P902-A6 (a
docs-plus-code task) is **renumbered to P902-A5** with its docs clause removed, since only
its code clause (`anvilml-server/src/lib.rs`) remains undone. The phase still totals five
tasks; only the shape of the fifth one changed.

## Doc correction text for P902-A1

The new `store.rs` module doc, in place of the false "Why in `anvilml-ipc`?" section,
should read approximately:

> `ArtifactStore` lives in its own crate because it is shared by `anvilml-scheduler`
> (which persists `WorkerEvent::ImageReady` payloads) and `anvilml-server` (which serves
> artifacts over HTTP), and neither of those crates may depend on the other. This mirrors
> `anvilml-registry`, which exists as its own crate for the same reason (shared by
> `anvilml-worker` and `anvilml-scheduler`).

## Required prereq-chain update outside this phase's own files

Per `FORGE_TASK_AUTHORING_SPEC.md §6`, inserting a retrofit phase requires updating the
prereqs of every downstream task that must now route through it. One such task exists:

- **`tasks_phase016.json` / `P16-A1`** currently has `"prereqs": ["P15-A3"]`. This must be
  changed to `"prereqs": ["P902-A5"]` so that Phase 016 cannot begin until the retrofit is
  fully complete. No other task in any phase file lists `P15-A3`, `P15-A2`, or `P15-A1` as
  a prereq, so no further prereq edits are needed.

This edit is to be applied directly to `tasks_phase016.json` as part of approving this
retrofit phase — it is not one of P902's own tasks because it modifies a different
phase's file, which `FORGE_TASK_AUTHORING_SPEC.md` treats as a plan-level bookkeeping
change rather than an implementation task.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-artifacts (new) | P902-A1 | Create crate; move `ArtifactStore` in verbatim; correct module doc |
| A | anvilml-ipc | P902-A2 | Delete moved module; drop dead deps; restore crate-doc accuracy |
| A | anvilml-scheduler | P902-A3 | Repoint import, add dep |
| A | anvilml-server + backend | P902-A4 | Repoint import, add dep |
| A | anvilml-server (code) | P902-A5 | Remove false "owns artifact store" claim from `lib.rs` crate doc |

## Prerequisites

P15-A3 complete (or, if approved before P15-A3 lands, P902-A1 simply waits in the DAG —
no code conflict exists since P15-A3 does not touch `ArtifactStore`'s location). The false
rationale is present in `crates/anvilml-ipc/src/artifact_store.rs`'s module doc and in
`.forge/reports/P15-A2_implement.md`.

## Interfaces and Contracts

No public interface changes. `ArtifactStore`'s struct shape, method signatures, and
behaviour are unchanged — only its crate of residence and import path move. Any code
outside this phase's 15 call sites that imports `anvilml_ipc::ArtifactStore` (none found
as of this phase's authoring — verified via workspace-wide grep) would need the same
import-path update.

## Known Constraints and Gotchas

- `docs/ARCHITECTURE.md`, `docs/ANVILML_DESIGN.md`, and `README.md` are already correct
  as of commit `5cb6a8b` (manual edit, predates this task list's execution). P902-A5 must
  not re-edit any of these three files — its scope is `crates/anvilml-server/src/lib.rs`
  only. If a future diff against this task list's original draft is consulted, ignore any
  instruction there to touch `ARCHITECTURE.md` or `ANVILML_DESIGN.md`; that work is done.
- `EventBroadcaster` and `ws/` in `anvilml-ipc` are out of scope — do not relocate them.
- `base64` is removed from `anvilml-ipc` in P902-A2 but must remain (or already exists) in
  `anvilml-scheduler`'s `Cargo.toml`, since `event_loop.rs` decodes `image_b64` there —
  verify it is still present before closing P902-A2; it was already declared there prior
  to this phase, so no addition should be necessary.
- P902-A3 and P902-A4 do not depend on each other and may run in either order or in
  parallel; both depend only on P902-A2.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging (none of these tasks add new log
  points, but moved code must retain its existing `tracing::debug!`/`instrument` calls
  unchanged).