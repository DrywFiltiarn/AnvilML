# Tasks: Phase 906 — OpenAPI Spec Correctness Retrofit

| Field | Value |
|-------|-------|
| Phase | 906 |
| Name | OpenAPI Spec Correctness Retrofit |
| Milestone group | Retrofit |
| Project(s) | anvilml |
| Status | Draft |
| Depends on phases | 905 (via P905-A6 and P905-A7) |
| Task file | `.forge/tasks/tasks_phase906.json` |
| Tasks | 5 |

---

## Overview

Phase 906 is a retrofit phase that corrects three categories of defect in
`backend/openapi.json` before Phase 21 begins.

**Defect 1 — `ModelKind` missing from schema components (critical).**
`anvilml-openapi/src/main.rs` never registered `ModelKind` in the
`#[derive(OpenApi)]` schema list. Two `$ref` pointers dangle, causing every
strict OpenAPI viewer to fail with `EMISSINGPOINTER`.

**Defect 2 — `DType.BF16` serialises as `b_f16` instead of `bf16` (critical).**
`rename_all = "snake_case"` splits `BF16` as three words (`B`, `F`, `16`),
producing `b_f16`. The correct value is `bf16`. This breaks any client
deserialising a model's `dtype_hint` field and caused the CI openapi-diff
gate to fail after P905-A1 added F8 variants (the diff exposed the stale
committed `backend/openapi.json`).

**Defect 3 — CI diff gate silently skipped by agent (process gap).**
The generator task (P20-A2) ran `cargo run -p anvilml-openapi` but did not
enforce `git diff --exit-code backend/openapi.json`. The gate only runs in
CI. When P905-A1 changed `DType` and the agent did not regenerate, the
committed file went stale and CI failed. P906-A4 makes the regeneration
step explicit and its acceptance criterion is the diff gate itself.

**Three secondary defects** (missing `image/png` binary schema, missing
`ArtifactMeta` required fields, missing `InferenceCaps` required fields)
are corrected in P906-A2.

**What this phase does NOT do:** No handler behaviour changes. No domain
type changes except the `BF16` rename attribute in P906-A3.

---

## Prereq update required before The Forge runs this phase

Update `.forge/tasks/tasks_phase021.json`:
- Change `P21-A1.prereqs` from `["P20-A4"]` to `["P906-A4"]`

Note: the previous P906 draft listed P20-A2 and P905-A2 as anchors. The
correct anchor is the end of Phase 905 — both terminal tasks `P905-A6` and
`P905-A7` must be complete before Phase 906 begins.

This routes Phase 21 through the corrected and committed spec. Commit this
change manually before The Forge picks up Phase 906.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-openapi / anvilml-core / anvilml-registry | P906-A1 – P906-A5 | Schema registration fix; secondary spec fixes; BF16 rename fix; regeneration gate; Windows path normalisation |

---

## Prerequisites

- `P905-A6` complete (PATCH endpoint; tail of the A1→A6 chain)
- `P905-A7` complete (cancel CI fix; parallel leaf)
- Both branches of Phase 905 must be complete before Phase 906 begins
- `P21-A1.prereqs` updated to `["P906-A4"]` before The Forge starts this phase

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|---|---|---|
| `crates/anvilml-core/src/config.rs` | P906-A1 | `ModelKind` variants; serde values (`rename_all = "snake_case"`) |
| `crates/anvilml-core/src/types/model.rs` | P906-A3 | `DType` variants and serde serialisation strings |
| `backend/openapi.json` | P906-A4 | Generated output; must match `cargo run -p anvilml-openapi` exactly |

---

## Task Descriptions

### Group A — anvilml-openapi / anvilml-core

#### P906-A1: anvilml-openapi: add missing ModelKind schema to component registration

- **Prereqs:** P905-A6, P905-A7
- **Tags:** fix

**Problem:** `backend/openapi.json` references `#/components/schemas/ModelKind` in
two locations but the schema is absent, producing `EMISSINGPOINTER` in all strict
viewers. The type is `ToSchema`-derived; only its registration in the generator
binary is missing.

**Fix:** In `crates/anvilml-openapi/src/main.rs`, add `ModelKind` to the
`.schema(...)` call chain in the components builder. The import is
`use anvilml_core::ModelKind;` (crate-root re-export from `config.rs`).

Correct `ModelKind` serde values (`rename_all = "snake_case"` on `config.rs`):

| Variant | JSON string | Note |
|---|---|---|
| `Clip` | `"clip"` | |
| `Diffusion` | `"diffusion"` | |
| `Vae` | `"vae"` | |
| `Lora` | `"lora"` | |
| `ControlNet` | `"control_net"` | two words → underscore |
| `Unet` | `"unet"` | |
| `Upscale` | `"upscale"` | default |

**Do not regenerate `backend/openapi.json` in this task.** P906-A4 owns
the single authoritative regeneration after all fixes are in place.

**Acceptance criterion:** `cargo build -p anvilml-openapi && cargo clippy
-p anvilml-openapi -- -D warnings` exits 0.

---

#### P906-A2: anvilml-openapi: secondary spec fixes (png schema, required fields)

- **Prereqs:** P906-A1
- **Tags:** fix

Three lower-severity defects corrected in the same file:

**Fix 1 — `GET /v1/artifacts/{hash}` 200 response body schema missing.**
The `image/png` media type entry is empty (`{}`). Add binary schema to the
`#[utoipa::path]` annotation for `serve_artifact`:
```rust
responses(
    (status = 200, content_type = "image/png",
     body = inline(schema!({"type": "string", "format": "binary"})),
     description = "Artifact found")
)
```

**Fix 2 — `ArtifactMeta.id` and `.job_id` absent from `required`.**
Both are `Option<Uuid>` in Rust but present in every real artifact record.
Add via `#[schema(required)]` on the fields in `anvilml-core`, or via the
utoipa annotation on the schema registration.

**Fix 3 — `InferenceCaps` has no `required` fields.**
`fp32` is present on every device. Add `fp32` to the required list.

**Do not regenerate `backend/openapi.json` in this task.** P906-A4 owns
regeneration.

**Acceptance criterion:** `cargo clippy -p anvilml-openapi -- -D warnings`
exits 0.

---

#### P906-A3: anvilml-core: fix BF16 serde rename (b_f16 -> bf16)

- **Prereqs:** P905-A6, P905-A7
- **Tags:** fix

**Problem:** `rename_all = "snake_case"` treats `BF16` as three words
(`B`, `F`, `16`), serialising it as `b_f16`. The correct serde value for
bfloat16 is `bf16`. This caused the CI openapi-diff gate to fail because
`cargo run -p anvilml-openapi` generates `"bf16"` while the committed
`backend/openapi.json` (from P20-A2, against the old enum) contained
`"b_f16"`.

Note: `F8E4M3` and `F8E5M2` are **not affected** — they already carry
explicit `#[serde(rename = "f8_e4m3")]` / `#[serde(rename = "f8_e5m2")]`
attributes added by P905-A1.

**Fix:** In `crates/anvilml-core/src/types/model.rs`, add
`#[serde(rename = "bf16")]` to the `BF16` variant of `DType`. This is
the only change in `anvilml-core`.

Add test `dtype_bf16_serde_string`:
```rust
#[test]
fn dtype_bf16_serde_string() {
    assert_eq!(
        serde_json::to_string(&DType::BF16).expect("serialize BF16"),
        "\"bf16\""
    );
}
```

Bump `anvilml-core` patch version.

**Acceptance criterion:** `cargo test -p anvilml-core` exits 0; the new
test passes; `serde_json::to_string(&DType::BF16)` == `"\"bf16\""`.

---

#### P906-A4: anvilml-openapi: regenerate and commit corrected backend/openapi.json

- **Prereqs:** P906-A2, P906-A3
- **Tags:** fix

**This task has no source changes.** Its sole output is a regenerated and
staged `backend/openapi.json` that reflects all fixes from P906-A1 through
P906-A3 and P905-A1/A2.

**Steps:**
1. Run `cargo run -p anvilml-openapi` — this writes `backend/openapi.json`.
2. Run `git diff backend/openapi.json` — confirm the file changed (it must
   differ from the stale committed version).
3. Verify the following in the generated file:
   - `components/schemas/ModelKind` exists with `type: string` and
     `enum: [clip, diffusion, vae, lora, control_net, unet, upscale]`
   - `components/schemas/DType.enum` contains `bf16` (not `b_f16`),
     `f8_e4m3`, `f8_e5m2`
   - `paths["/v1/artifacts/{hash}"].get.responses.200.content["image/png"].schema`
     has `type: string, format: binary`
4. Run `git add backend/openapi.json` (stage only; The Forge commits).
5. Run `cargo test --workspace --features mock-hardware` — exits 0.

**Acceptance criterion:** `cargo run -p anvilml-openapi && git diff
--exit-code backend/openapi.json` exits 0 (idempotent — running the
generator twice produces no diff).

---

#### P906-A5: anvilml-registry: fix stale-model LIKE query fails on Windows paths

- **Prereqs:** P905-A6, P905-A7
- **Tags:** fix

**Problem:** `test_rescan_removes_stale_models` passes on Linux and fails on Windows CI
with `assertion failed: should remove 1 stale model (left: 0, right: 1)`.

Root cause: `store.rs` constructs the LIKE query with a hardcoded forward slash:
```sql
WHERE path LIKE ? || '/' || '%'
```
On Windows, `tempfile::tempdir()` produces backslash-separated paths
(e.g. `C:\Users\...\Temp\...`). The `/` separator never matches, `all_rows` is empty,
`stale_ids` is empty, and `removed` returns 0.

**Fix — two-part:**

1. **Normalise paths to forward slashes** at the point of DB write (in `upsert`) and
   at the point of query construction (in `rescan`):
   ```rust
   let path_str = meta.path.to_string_lossy().replace('\\', "/");
   ```
   Apply this to every `path` string that flows into or out of the `models` table.

2. **Replace the two-pass LIKE+exact SQL** with a single full-table fetch filtered in
   Rust:
   ```rust
   let all_rows: Vec<(String, String)> =
       sqlx::query_as("SELECT id, path FROM models")
           .fetch_all(&self.pool).await.map_err(sqlx_error)?;
   // Filter: path starts with any normalised dir prefix
   let stale_ids: Vec<String> = all_rows
       .into_iter()
       .filter(|(_, path)| {
           dirs.iter().any(|d| {
               let dir_norm = d.path.to_string_lossy().replace('\\', "/");
               path.starts_with(&dir_norm)
           }) && !fresh_paths.contains(path)
       })
       .map(|(id, _)| id)
       .collect();
   ```
   This eliminates the platform-specific separator dependency in SQL entirely and
   handles registries with multiple scanned directories correctly.

**No changes to test files.** The existing `rescan_stale.rs` test is correct — it
asserts `removed == 1` which is the expected behaviour. The fix makes the production
code match the test's expectation on all platforms.

Bump `anvilml-registry` patch version.

**Acceptance criterion:** `cargo test -p anvilml-registry` exits 0; the
`test_rescan_removes_stale_models` test passes; `cargo test --workspace
--features mock-hardware` exits 0.

---

## Phase Acceptance Criteria

```bash
# 1. All crates build and lint clean
cargo build --workspace --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings

# 2. BF16 serde value is correct
cargo test -p anvilml-core -- dtype_bf16_serde_string

# 3. Generator is idempotent against the committed file
cargo run -p anvilml-openapi
git diff --exit-code backend/openapi.json

# 4. Committed spec contains required schemas
grep -q '"ModelKind"' backend/openapi.json && echo "ModelKind OK"
python3 -c "
import json, sys
spec = json.load(open('backend/openapi.json'))
dt = spec['components']['schemas']['DType']['enum']
assert 'bf16' in dt and 'b_f16' not in dt, f'BF16 wrong: {dt}'
assert 'f8_e4m3' in dt and 'f8_e5m2' in dt, f'F8 missing: {dt}'
print('DType enum OK:', dt)
"

# 5. Stale-model removal works on all platforms
cargo test -p anvilml-registry -- rescan_stale

# 6. Full workspace test suite green
cargo test --workspace --features mock-hardware
```

---

## Known Constraints and Gotchas

- **Do not regenerate in A1/A2.** A1 and A2 only modify `main.rs`; they
  must not write `backend/openapi.json`. The stale file must remain until
  A4 regenerates it atomically with all fixes present. An intermediate
  regeneration would produce a partially-corrected spec that would then
  drift again when A3 lands.
- **`BF16` rename is additive.** The F8 variants already have explicit
  `#[serde(rename = ...)]` from P905-A1. A3 adds the same pattern to
  `BF16`. No other `DType` variants need explicit renames (`F32`→`f32`,
  `F16`→`f16`, `Q8`→`q8`, `Q4`→`q4`, `Unknown`→`unknown` are all correct
  under `rename_all = "snake_case"`).
- **`ControlNet` → `"control_net"`**: two-word PascalCase produces an
  underscore under `rename_all`. Verify the generated `ModelKind` enum
  in the spec uses `"control_net"`, not `"controlnet"`.
- **Windows path normalisation (A5):** All path strings stored in and read from the
  `models` table must use forward slashes. Apply `.replace('\\', "/")` consistently
  in both `upsert` (before insert) and `rescan` (before prefix comparison). Mixed
  separators in the DB (some rows written before the fix, some after) will cause false
  negatives in stale detection — this is acceptable for the retrofit since the DB is
  ephemeral in test environments and users are expected to trigger a rescan after
  upgrade.
- **`anvilml-core` version bump in A3.** Source files in `anvilml-core`
  are modified; the patch version must be bumped per `FORGE_AGENT_RULES.md
  §12.5`.
- **`anvilml-openapi` version bump in A1.** Source files in
  `anvilml-openapi` are modified in A1 (and A2); bump the patch version in
  A1, no additional bump needed for A2.
- **P21-A1 prereq must be updated manually** from `["P20-A4"]` to
  `["P906-A4"]` in `tasks_phase021.json` before The Forge runs Phase 906.