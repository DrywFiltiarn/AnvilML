# Addendum: `AnvilError::ArtifactNotFound` variant

**Status:** Resolved this session. Recorded here because this session's tooling has
read-only access to `docs/ANVILML_DESIGN.md` via `project_knowledge_search` — the
actual repository file must be hand-edited by whoever next has write access, using
the exact diff below. Phase 15's task definitions in this delivery already assume
this change is in effect.

---

## Background

Phase 15's `GET /v1/artifacts/:hash` handler (task `P15-B2`) needs to return `404`
when the requested content hash has no matching artifact. `ANVILML_DESIGN.md §5.2`'s
original `AnvilError` enum, as specified back in Phase 2 (`P2-A1`), is:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    Db(#[from] sqlx::Error),
    Io(#[from] std::io::Error),
    Serde(String),
    Ipc(String),
    PayloadTooLarge(String),
    WorkerNotFound(String),
    JobNotFound(String),
    InvalidGraph(Vec<String>),
    CycleDetected(Vec<String>),
    ModelNotFound(String),
    WorkersUnavailable(String),
    Internal(String),
}
```

None of these variants is a correct semantic fit for "no artifact exists at this
content hash" — `P15-B2` used `Internal("artifact_not_found")` as a placeholder and
flagged it explicitly as a Deviation rather than silently treating it as settled,
per this project's convention for documenting genuine small spec gaps instead of
guessing past them.

## Resolution

Add an eighth `NotFound`-style variant, `ArtifactNotFound`, following the exact
existing precedent set by `WorkerNotFound`, `JobNotFound`, and `ModelNotFound` —
each already takes a single `String` identifier and maps to HTTP `404`:

```diff
 #[derive(Debug, thiserror::Error)]
 pub enum AnvilError {
     Db(#[from] sqlx::Error),
     Io(#[from] std::io::Error),
     Serde(String),
     Ipc(String),
     PayloadTooLarge(String),
     WorkerNotFound(String),
     JobNotFound(String),
     InvalidGraph(Vec<String>),
     CycleDetected(Vec<String>),
     ModelNotFound(String),
+    ArtifactNotFound(String),
     WorkersUnavailable(String),
     Internal(String),
 }
```

The `IntoResponse` impl gains one matching arm, identical in shape to
`ModelNotFound`'s:

```diff
+    AnvilError::ArtifactNotFound(hash) => (
+        StatusCode::NOT_FOUND,
+        Json(ErrorBody {
+            error: "artifact_not_found".into(),
+            message: format!("no artifact found for hash: {hash}"),
+            request_id: Uuid::new_v4(),
+        }),
+    ),
```

No other field on `AnvilError`, no other enum, and no migration/seed data is
affected — this is an additive, non-breaking change to one enum's variant list,
exactly analogous in shape and scope to the `EnumerationSource::Cpu` addendum from
earlier in this delivery (`docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md`).

## Where this is reflected in this delivery

- **`tasks/tasks_phase002.json` / `docs/TASKS_PHASE002.md`**, task `P2-A1`: now
  specifies `AnvilError` with `ArtifactNotFound(String)` included in the variant
  list and its `IntoResponse` mapping, at the point the enum is first defined.
- **`tasks/tasks_phase015.json` / `docs/TASKS_PHASE015.md`**, task `P15-B2`: now
  returns `AnvilError::ArtifactNotFound(hash)` directly instead of the
  `Internal("artifact_not_found")` placeholder, with the Deviation note removed
  since the gap it flagged is now closed.

## Action required by the repository maintainer

Apply the diff above to the live `docs/ANVILML_DESIGN.md §5.2` before or during
Phase 2 implementation, so the agent's `project_knowledge_search` reads reflect the
corrected enum when that phase is actually executed.
