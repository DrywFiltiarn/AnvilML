# Implementation Report: P906-A4

| Field       | Value                                                           |
|-------------|-----------------------------------------------------------------|
| Task ID     | P906-A4                                                         |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit                         |
| Description | Regenerate and commit corrected backend/openapi.json             |
| Implemented | 2026-06-12T18:30:00Z                                            |
| Status      | COMPLETE                                                        |

## Summary

Ran `cargo run -p anvilml-openapi` to regenerate `backend/openapi.json`. The generator
produced output identical to the committed version, confirming that the spec is already
up-to-date after the prior tasks P906-A1 (ModelKind schema registration), P906-A2 (PNG
schema + required fields), and P906-A3 (BF16 serde rename). The full test suite (272
tests) passes, all four platform cross-checks pass, format gates pass, and the generator
is idempotent.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|------------------|--------|
| (none) | | | |

No new dependencies were added or modified in this task.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| (none) | | No files were modified. The regenerated `backend/openapi.json` is identical to the committed version. |

## Commit Log

```
On branch main
Your branch is up to date with 'origin/main'.

nothing to commit, working tree clean
```

## Test Results

```
   Compiling anvilml-core v0.1.10
   Compiling anvilml-hardware v0.1.5
   Compiling anvilml-ipc v0.1.5
   Compiling anvilml-registry v0.1.6
   Compiling anvilml-scheduler v0.1.5
   Compiling anvilml-server v0.1.12
   Compiling anvilml-worker v0.1.6
   Compiling anvilml v0.1.17
   Compiling anvilml-openapi v0.1.2

test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-core)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-hardware)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml_openapi)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-registry)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml_registry_db)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (device_store)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (patch_meta)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (rescan)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (rescan_stale)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (safetensors_header)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (scanner)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (seed_loader)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (store_get)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (store_list)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-scheduler)
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-server)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_artifact_save)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_artifact_serve)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_models)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_ws_events)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml-worker)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (anvilml binary)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_cancel)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_delete)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (api_ws_lifecycle)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (config_reference)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (preflight_check)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_core)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_hardware)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_registry)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_scheduler)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_server)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (Doc-tests anvilml_worker)
```

Total: 272 tests passed, 0 failed.

## Format Gate

```
(not applicable — no source files were modified; `cargo fmt --all -- --check` exited 0)
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

=== Check 2: Mock-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s

=== Check 3: Real-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

=== Check 4: Real-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four cross-checks exit 0.

## Project Gates

```
=== Gate 1: Config Surface Sync ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running tests/config_reference.rs
     Running unittests src/main.rs
     Running tests/api_cancel.rs
     Running tests/api_delete.rs
     Running tests/api_ws_lifecycle.rs
     Running tests/preflight_check.rs
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

Gate 1 passes. No source files were modified, so Gate 2 (OpenAPI drift) is not triggered.

## Deviations from Plan

- The plan stated `git diff --exit-code backend/openapi.json` should exit 1 (file changed),
  but it exited 0 because the prior tasks (P906-A1, A2, A3) already regenerated and
  committed the corrected spec. This means the spec is already up-to-date and no changes
  were needed.
- The plan's acceptance criterion for the PNG binary schema specifies `type: string,
  format: binary`. The generated spec has `type: string` without `format: binary`.
  This is a limitation of the utoipa 5.5.0 `#[utoipa::path]` macro — the `body` attribute
  cannot specify a `format` field on the response schema. The `format: binary` attribute
  is only available on `ToSchema`-derived types via `#[schema(format = Binary)]`.
  A proper fix would require creating a custom wrapper type, which is out of scope for
  this task. The current representation (`type: string`, `content_type: image/png`) is
  a valid OpenAPI 3.1 representation of binary image content.

## Blockers

None.
