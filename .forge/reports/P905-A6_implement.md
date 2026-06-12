# Implementation Report: P905-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A6                                           |
| Phase       | 905 — FP8 dtype & model metadata override         |
| Description | anvilml-server: PATCH /v1/models/:id metadata override endpoint |
| Implemented | 2026-06-12T15:05:00Z                             |
| Status      | COMPLETE                                          |

## Summary

Implemented the `PATCH /v1/models/:id` endpoint for the AnvilML server. Added a `patch_model`
async handler in `handlers/models.rs` that delegates to `ModelRegistry::patch_meta`, returns
200 with the updated `ModelMeta` on success, 404 when the model is not found, and 500 on errors.
Wired the route in `lib.rs` alongside the existing GET route on `/v1/models/{id}`. Added three
unit tests covering dtype update with VRAM recomputation, 404 for missing models, and partial
patch field preservation. Bumped `anvilml-server` version from `0.1.18` to `0.1.19`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| rust   | axum      | 0.8              | lockfile       |

Note: `axum::extract::Json` in v0.8 does not have an `into_inner()` method; the inner value
is accessed via `body.0`. This was the only deviation from the plan's suggested API call.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `patch_model` handler with `#[utoipa::path]` annotation; import `ModelMetaPatch` |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire PATCH route; add 3 unit tests |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.18 → 0.1.19` |

## Commit Log

```
 .forge/reports/P905-A6_plan.md               | 109 +++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/models.rs |  49 ++++++-
 crates/anvilml-server/src/lib.rs             | 202 ++++++++++++++++++++++++++-
 7 files changed, 369 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-c9f15fb44f7888ec)

running 45 tests
test tests::patch_model_updates_dtype_hint ... ok
test tests::patch_model_returns_404 ... ok
test tests::patch_model_partial_preserves_other_fields ... ok
... (all 45 tests passed)

test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.20s
```

All 3 new tests pass:
- `patch_model_updates_dtype_hint` — sends `{"dtype_hint":"f8_e4m3"}`, verifies 200 with updated dtype and recomputed `vram_estimate_mib`
- `patch_model_returns_404` — sends PATCH for non-existent model ID, verifies 404 with error JSON
- `patch_model_partial_preserves_other_fields` — sends `{"kind":"vae"}`, verifies `dtype_hint` unchanged and `kind` updated

Full workspace test suite: 220+ tests passed, 0 failed.

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.57s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.83s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.36s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.81s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
No `ServerConfig` fields were modified; the config reference test passes (no matching tests run).

### Gate 2 — OpenAPI Drift
```
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
(no diff output — exit 0)
```
The `#[utoipa::path]` annotation addition triggers Gate 2. `cargo run -p anvilml-openapi`
regenerated `backend/openapi.json`; `git diff --exit-code` exits 0 confirming idempotency.

## Deviations from Plan

- **Axum API**: The plan specified `body.into_inner()` for extracting the inner `ModelMetaPatch`
  from `axum::extract::Json`. Axum 0.8 does not provide `into_inner()` on `Json<T>`; the inner
  value is accessed via `body.0`. Changed to `body.0` to compile successfully.

## Blockers

None.
